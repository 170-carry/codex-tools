use super::*;

fn storage() -> ProxyStorageContext {
    // Fixtures have no usage: the writer never opens this path or any database.
    ProxyStorageContext {
        data_dir: std::env::temp_dir().join(format!("responses-test-{}", uuid::Uuid::new_v4())),
        store_lock: Arc::new(tokio::sync::Mutex::new(())),
        auth_refresh_lock: Arc::new(tokio::sync::Mutex::new(())),
        usage_writer: Arc::new(tokio::sync::Mutex::new(None)),
        sync_active_auth_on_refresh: false,
    }
}

fn metadata() -> ApiProxyUsageMetadata {
    ApiProxyUsageMetadata {
        key_id: None,
        key_label: None,
        model: "gpt-6.1-sol".into(),
        route: "/v1/responses".into(),
        reasoning_effort: None,
        service_tier: None,
    }
}

fn upstream(chunks: Vec<Result<Bytes, String>>) -> CodexUpstreamResponse {
    CodexUpstreamResponse::WebSocket {
        headers: HeaderMap::new(),
        stream: Box::pin(futures_util::stream::iter(chunks)),
    }
}

fn frame(value: Value) -> Result<Bytes, String> {
    Ok(Bytes::from(format!("data: {value}\n\n")))
}

fn delta() -> Result<Bytes, String> {
    frame(json!({"type":"response.output_text.delta", "delta":"partial", "sequence_number":7}))
}

fn cases() -> Vec<(Vec<Result<Bytes, String>>, &'static str)> {
    let mut cases = vec![
        (
            vec![delta(), Err("https://secret.invalid/?key=private".into())],
            "response.failed",
        ),
        (vec![delta()], "response.failed"),
        (vec![], "response.failed"),
        (
            vec![frame(
                json!({"type":"error", "error":{"code":"rate_limit_exceeded", "message":"secret", "request":{"tools":"secret"}}}),
            )],
            "response.failed",
        ),
        (
            vec![Ok(Bytes::from_static(
                b"event: error\ndata: {\"code\":\"overloaded\",\"message\":\"secret\"}\n\n",
            ))],
            "response.failed",
        ),
        (
            vec![Ok(Bytes::from_static(b"data: {\"type\":\"response.compl"))],
            "response.failed",
        ),
    ];
    for terminal in [
        "response.completed",
        "response.failed",
        "response.incomplete",
        "response.cancelled",
        "response.canceled",
        "response.done",
    ] {
        cases.push((
            vec![
                delta(),
                frame(
                    json!({"type":terminal, "response":{"id":"resp_test", "status":"incomplete"}}),
                ),
                Err("late secret".into()),
            ],
            terminal,
        ));
    }
    // Finish may flush a valid final event without the last SSE delimiter.
    cases.push((
        vec![Ok(Bytes::from_static(
            b"data: {\"type\":\"response.completed\"}",
        ))],
        "response.completed",
    ));
    cases
}

fn check(events: &[Value], terminal: &str) {
    assert_eq!(events.last().unwrap()["type"], terminal);
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(
                event["type"].as_str(),
                Some(
                    "response.completed"
                        | "response.failed"
                        | "response.incomplete"
                        | "response.cancelled"
                        | "response.canceled"
                        | "response.done"
                )
            ))
            .count(),
        1
    );
    let serialized = serde_json::to_string(events).unwrap();
    assert!(!serialized.contains("secret"));
    assert!(!serialized.contains("private"));
    if events.first().unwrap()["type"] == "response.output_text.delta" {
        assert_eq!(events[0]["delta"], "partial");
    }
}

#[tokio::test]
async fn http_sse_bytes_have_exactly_one_terminal_event() {
    for (chunks, terminal) in cases() {
        let response = build_passthrough_sse_response(upstream(chunks), storage(), metadata());
        let bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        // Inspect actual wire data. Do not run the production SSE decoder here.
        let text = String::from_utf8(bytes.to_vec()).unwrap();
        let events: Vec<Value> = text
            .lines()
            .filter_map(|line| line.strip_prefix("data: "))
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        check(&events, terminal);
        if text.contains("upstream_stream_interrupted") {
            assert_eq!(events.last().unwrap()["response"]["status"], "failed");
            if events.len() > 1 {
                assert_eq!(events.last().unwrap()["sequence_number"], 8);
            }
        }
    }
}

#[tokio::test]
async fn websocket_wire_frames_have_exactly_one_terminal_event() {
    for (chunks, terminal) in cases() {
        let chunks = Arc::new(std::sync::Mutex::new(Some(chunks)));
        let app = Router::new().route(
            "/",
            get(move |ws: WebSocketUpgrade| {
                let chunks = chunks.lock().unwrap().take().unwrap();
                async move {
                    ws.on_upgrade(move |mut socket| async move {
                        relay_responses_sse_to_websocket(
                            &mut socket,
                            upstream(chunks),
                            storage(),
                            metadata(),
                        )
                        .await
                        .unwrap();
                        let _ = socket.close().await;
                    })
                }
            }),
        );
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let (mut socket, _) = connect_async(format!("ws://{address}/")).await.unwrap();
        let mut events = Vec::new();
        while let Some(message) =
            tokio::time::timeout(std::time::Duration::from_secs(5), socket.next())
                .await
                .unwrap()
        {
            match message.unwrap() {
                Message::Text(text) => events.push(serde_json::from_str(&text).unwrap()),
                Message::Close(_) => break,
                _ => {}
            }
        }
        server.abort();
        check(&events, terminal);
    }
}

#[tokio::test]
async fn fragmented_error_keeps_response_id_and_only_bounded_error_code() {
    let error = format!(
        "event: error\ndata: {}\n\n",
        json!({"error":{"code":"x".repeat(200),"message":"secret"}})
    );
    let mut chunks = vec![frame(
        json!({"type":"response.created", "response":{"id":"resp_original"}}),
    )];
    chunks.extend(error.bytes().map(|byte| Ok(Bytes::from(vec![byte]))));
    let response = build_passthrough_sse_response(upstream(chunks), storage(), metadata());
    let bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let text = String::from_utf8(bytes.to_vec()).unwrap();
    assert!(text.contains("resp_original"));
    assert!(text.contains("upstream_error"));
    assert!(!text.contains("secret"));
    assert!(!text.contains(&"x".repeat(200)));
}
