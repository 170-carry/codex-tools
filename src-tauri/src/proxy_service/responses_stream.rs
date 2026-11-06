//! Responses termination policy shared by HTTP SSE and WebSocket forwarding.
use super::*;

pub(super) fn events(
    mut upstream: UpstreamByteStream,
) -> Pin<Box<dyn Stream<Item = SseEvent> + Send>> {
    Box::pin(stream! {
        let mut decoder = SseDecoder::default();
        let mut state = Termination::default();
        while let Some(chunk) = upstream.next().await {
            match chunk {
                Ok(chunk) => {
                    for event in decoder.push(&chunk) {
                        let event = state.observe(event);
                        yield event;
                        if state.terminal { return; }
                    }
                }
                Err(_) => {
                    // Transport errors can contain URLs/credentials. Never reflect them.
                    yield state.failure("upstream_stream_interrupted");
                    return;
                }
            }
        }
        for event in decoder.finish() {
            let event = state.observe(event);
            yield event;
            if state.terminal { return; }
        }
        yield state.failure("upstream_stream_interrupted");
    })
}

#[derive(Default)]
struct Termination {
    terminal: bool,
    response_id: Option<String>,
    sequence: u64,
}

fn identifier(value: Option<&Value>) -> Option<&str> {
    value.and_then(Value::as_str).filter(|text| {
        !text.is_empty()
            && text.len() <= 128
            && text
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"_-".contains(&byte))
    })
}

impl Termination {
    fn observe(&mut self, event: SseEvent) -> SseEvent {
        let data = match serde_json::from_str::<Value>(&event.data) {
            Ok(data) if data.is_object() => data,
            _ => {
                self.terminal = true;
                return self.failure("upstream_stream_interrupted");
            }
        };
        if let Some(id) = identifier(data.pointer("/response/id")) {
            self.response_id = Some(id.to_string());
        }
        if let Some(sequence) = data.get("sequence_number").and_then(Value::as_u64) {
            self.sequence = self.sequence.max(sequence.saturating_add(1));
        }
        if data.get("type").and_then(Value::as_str) == Some("error")
            || event.event.as_deref() == Some("error")
        {
            // Retain bounded machine-readable diagnostics, not arbitrary upstream
            // messages, nested payloads, tool schemas or request text.
            let code = identifier(data.pointer("/error/code"))
                .or_else(|| identifier(data.get("code")))
                .unwrap_or("upstream_error");
            self.terminal = true;
            return self.failure(code);
        }
        self.terminal = is_responses_terminal_event(&event);
        event
    }

    fn failure(&self, code: &str) -> SseEvent {
        let id = self
            .response_id
            .clone()
            .unwrap_or_else(|| format!("resp_proxy_{}", uuid::Uuid::new_v4().simple()));
        let data = json!({
            "type": "response.failed",
            "sequence_number": self.sequence,
            "response": {
                "id": id, "object": "response", "status": "failed",
                "error": {
                    "code": code,
                    "message": "The upstream response failed or ended before completion."
                }
            }
        });
        SseEvent {
            event: Some("response.failed".into()),
            data: data.to_string(),
        }
    }
}

pub(super) fn build_passthrough_sse_response(
    upstream: CodexUpstreamResponse,
    usage_storage: ProxyStorageContext,
    usage_metadata: ApiProxyUsageMetadata,
) -> Response<Body> {
    let (headers, upstream) = upstream.into_stream();
    let output = stream! {
        let mut events = events(upstream);
        let mut recorded_usage = false;
        while let Some(event) = events.next().await {
            maybe_record_stream_usage_tokens(
                &usage_storage, &usage_metadata, &event, &mut recorded_usage,
            ).await;
            yield Ok::<Bytes, Infallible>(serialize_sse_event(
                event.event.as_deref(), &rewrite_sse_event_data_models_for_client(&event.data),
            ));
        }
    };
    let mut response = Response::builder().status(StatusCode::OK);
    for (name, value) in &headers {
        if should_forward_response_header(name.as_str()) {
            response = response.header(name, value);
        }
    }
    response
        .header("content-type", "text/event-stream; charset=utf-8")
        .header("cache-control", "no-cache")
        .body(Body::from_stream(output))
        .unwrap_or_else(|_| json_error_response(StatusCode::BAD_GATEWAY, "构建流式代理响应失败"))
}

pub(super) async fn relay_responses_sse_to_websocket(
    socket: &mut AxumWebSocket,
    upstream: CodexUpstreamResponse,
    usage_storage: ProxyStorageContext,
    usage_metadata: ApiProxyUsageMetadata,
) -> Result<(), String> {
    let (_, upstream) = upstream.into_stream();
    let mut events = events(upstream);
    let mut recorded_usage = false;
    while let Some(event) = events.next().await {
        maybe_record_stream_usage_tokens(
            &usage_storage,
            &usage_metadata,
            &event,
            &mut recorded_usage,
        )
        .await;
        send_responses_websocket_event(socket, &event).await?;
    }
    Ok(())
}
