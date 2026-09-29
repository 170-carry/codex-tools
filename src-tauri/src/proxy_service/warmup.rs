//! Bounded, non-retrying account warm-up transport.
use super::*;

pub(super) fn minimal_account_warmup_payload() -> Result<Value, String> {
    normalize_openai_responses_request(json!({
        "model": DEFAULT_API_PROXY_MODEL,
        "input": "hello",
        "stream": true,
        "store": false,
        "reasoning": {"effort": "none", "summary": "auto"},
        "text": {"verbosity": "low"},
        "service_tier": "default"
    }))
    .map(|(payload, _)| payload)
}

/// Sends the smallest supported real inference request for intentionally
/// activating an account's short usage window. The fixed prompt contains no
/// user data, disables storage and reasoning, and never retries by itself.
pub(crate) async fn send_minimal_account_warmup_request(auth_json: &Value) -> Result<(), String> {
    let auth = extract_auth(auth_json)?;
    let payload = payload_for_upstream(&minimal_account_warmup_payload()?);
    let body =
        serde_json::to_vec(&payload).map_err(|error| format!("序列化账号预热请求失败: {error}"))?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(ACCOUNT_WARMUP_TIMEOUT_SECS))
        .build()
        .map_err(|error| format!("创建账号预热客户端失败: {error}"))?;
    let url = format!(
        "{}/responses",
        resolve_codex_upstream_base_url().trim_end_matches('/')
    );
    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", auth.access_token))
        .header("ChatGPT-Account-Id", auth.account_id)
        .header("Accept", "text/event-stream")
        .header("Content-Type", "application/json")
        .header("User-Agent", CODEX_USER_AGENT)
        .header("Originator", "codex_cli_rs")
        .header("Version", CODEX_CLIENT_VERSION)
        .header("session-id", uuid::Uuid::new_v4().to_string())
        .header(RESPONSES_LITE_HEADER, "true")
        .body(body)
        .send()
        .await
        .map_err(|error| format!("发送账号预热请求失败: {error}"))?;
    let status = response.status();
    let response_body = read_account_warmup_response_limited(response).await?;
    let response_text = String::from_utf8_lossy(&response_body);

    if !status.is_success() {
        return Err(format!(
            "账号预热请求被上游拒绝 HTTP {}: {}",
            status.as_u16(),
            truncate_for_error(response_text.trim(), 240)
        ));
    }
    let mut decoder = SseDecoder::default();
    for event in decoder
        .push(&response_body)
        .into_iter()
        .chain(decoder.finish())
    {
        if let Some(result) = warmup_terminal_result(&event) {
            return result;
        }
    }
    Err("账号预热响应未包含完整的成功完成事件".to_string())
}

async fn read_account_warmup_response_limited(
    response: reqwest::Response,
) -> Result<Vec<u8>, String> {
    let mut body = Vec::new();
    let mut decoder = SseDecoder::default();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|error| format!("读取账号预热响应失败: {error}"))?;
        if body.len().saturating_add(chunk.len()) > MAX_ACCOUNT_WARMUP_RESPONSE_BYTES {
            return Err("账号预热响应超过 1 MiB 安全上限".to_string());
        }
        body.extend_from_slice(&chunk);
        if decoder
            .push(&chunk)
            .iter()
            .any(|event| warmup_terminal_result(event).is_some())
        {
            break;
        }
    }
    Ok(body)
}

fn warmup_terminal_result(event: &SseEvent) -> Option<Result<(), String>> {
    let value: Value = serde_json::from_str(&event.data).ok()?;
    let kind = value.get("type").and_then(Value::as_str)?;
    if !matches!(
        kind,
        "response.completed"
            | "response.done"
            | "response.failed"
            | "response.incomplete"
            | "error"
            | "response.cancelled"
            | "response.canceled"
    ) {
        return None;
    }
    let response = value.get("response");
    let status = response
        .and_then(|r| r.get("status"))
        .and_then(Value::as_str);
    if matches!(kind, "response.completed" | "response.done")
        && response.is_some()
        && status == Some("completed")
    {
        Some(Ok(()))
    } else {
        Some(Err(format!(
            "账号预热推理未成功完成: {}",
            truncate_for_error(&value.to_string(), 500)
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn warmup_requires_complete_structured_terminal_event() {
        let mut decoder = SseDecoder::default();
        assert!(decoder
            .push(b"event: response.completed\ndata: ")
            .is_empty());
        let events = decoder
            .push(b"{\"type\":\"response.completed\",\"response\":{\"status\":\"completed\"}}\n\n");
        assert!(warmup_terminal_result(&events[0]).unwrap().is_ok());
        let delta = SseEvent {
            event: None,
            data: json!({"type":"response.output_text.delta", "delta":"response.completed"})
                .to_string(),
        };
        assert!(warmup_terminal_result(&delta).is_none());
    }
    #[test]
    fn warmup_accepts_done_but_rejects_failed_status() {
        for (kind, status, success) in [
            ("response.done", "completed", true),
            ("response.done", "failed", false),
            ("response.failed", "failed", false),
            ("response.incomplete", "incomplete", false),
        ] {
            let event = SseEvent {
                event: None,
                data: json!({"type":kind, "response":{"status":status}}).to_string(),
            };
            assert_eq!(warmup_terminal_result(&event).unwrap().is_ok(), success);
        }
    }
}
