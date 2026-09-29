//! Translation of Responses stream events into Chat Completions chunks.
use super::*;

pub(super) fn translate_sse_event_to_chat_chunk(
    event: &SseEvent,
    state: &mut ChatStreamState,
) -> Vec<Value> {
    let Ok(parsed) = serde_json::from_str::<Value>(&event.data) else {
        return Vec::new();
    };
    let Some(kind) = parsed.get("type").and_then(Value::as_str) else {
        return Vec::new();
    };

    if state.terminal_received {
        return Vec::new();
    }
    if let Some(result) = terminal_chunk(&parsed, kind, state) {
        return vec![result];
    }
    match kind {
        "response.created" => {
            state.response_id = parsed
                .get("response")
                .and_then(|value| value.get("id"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            state.created_at = parsed
                .get("response")
                .and_then(|value| value.get("created_at"))
                .and_then(Value::as_i64)
                .unwrap_or(0);
            state.model = parsed
                .get("response")
                .and_then(|value| value.get("model"))
                .and_then(Value::as_str)
                .map(normalize_model_for_client)
                .unwrap_or_default();
            Vec::new()
        }
        "response.reasoning_summary_text.delta" => parsed
            .get("delta")
            .and_then(Value::as_str)
            .map(|delta| {
                vec![build_chat_chunk(
                    state,
                    json!({
                        "role": "assistant",
                        "reasoning_content": delta,
                    }),
                    None,
                    parsed.get("response").and_then(|value| value.get("usage")),
                )]
            })
            .unwrap_or_default(),
        "response.reasoning_summary_text.done" => vec![build_chat_chunk(
            state,
            json!({
                "role": "assistant",
                "reasoning_content": "\n\n",
            }),
            None,
            parsed.get("response").and_then(|value| value.get("usage")),
        )],
        "response.output_text.delta" => parsed
            .get("delta")
            .and_then(Value::as_str)
            .map(|delta| {
                vec![build_chat_chunk(
                    state,
                    json!({
                        "role": "assistant",
                        "content": delta,
                    }),
                    None,
                    parsed.get("response").and_then(|value| value.get("usage")),
                )]
            })
            .unwrap_or_default(),
        "response.output_item.added" => {
            let Some(item) = parsed.get("item").and_then(Value::as_object) else {
                return Vec::new();
            };
            if item.get("type").and_then(Value::as_str) != Some("function_call") {
                return Vec::new();
            }

            state.function_call_index += 1;
            state.has_received_arguments_delta = false;
            state.has_tool_call_announced = true;

            vec![build_chat_chunk(
                state,
                json!({
                    "role": "assistant",
                    "tool_calls": [{
                        "index": state.function_call_index,
                        "id": item.get("call_id").and_then(Value::as_str).unwrap_or_default(),
                        "type": "function",
                        "function": {
                            "name": item.get("name").and_then(Value::as_str).unwrap_or_default(),
                            "arguments": "",
                        }
                    }]
                }),
                None,
                parsed.get("response").and_then(|value| value.get("usage")),
            )]
        }
        "response.function_call_arguments.delta" => {
            state.has_received_arguments_delta = true;
            vec![build_chat_chunk(
                state,
                json!({
                    "tool_calls": [{
                        "index": state.function_call_index,
                        "function": {
                            "arguments": parsed.get("delta").and_then(Value::as_str).unwrap_or_default(),
                        }
                    }]
                }),
                None,
                parsed.get("response").and_then(|value| value.get("usage")),
            )]
        }
        "response.function_call_arguments.done" => {
            if state.has_received_arguments_delta {
                return Vec::new();
            }

            vec![build_chat_chunk(
                state,
                json!({
                    "tool_calls": [{
                        "index": state.function_call_index,
                        "function": {
                            "arguments": parsed.get("arguments").and_then(Value::as_str).unwrap_or_default(),
                        }
                    }]
                }),
                None,
                parsed.get("response").and_then(|value| value.get("usage")),
            )]
        }
        "response.output_item.done" => {
            let Some(item) = parsed.get("item").and_then(Value::as_object) else {
                return Vec::new();
            };
            if item.get("type").and_then(Value::as_str) != Some("function_call") {
                return Vec::new();
            }
            if state.has_tool_call_announced {
                state.has_tool_call_announced = false;
                return Vec::new();
            }

            state.function_call_index += 1;
            vec![build_chat_chunk(
                state,
                json!({
                    "role": "assistant",
                    "tool_calls": [{
                        "index": state.function_call_index,
                        "id": item.get("call_id").and_then(Value::as_str).unwrap_or_default(),
                        "type": "function",
                        "function": {
                            "name": item.get("name").and_then(Value::as_str).unwrap_or_default(),
                            "arguments": item.get("arguments").and_then(Value::as_str).unwrap_or_default(),
                        }
                    }]
                }),
                None,
                parsed.get("response").and_then(|value| value.get("usage")),
            )]
        }
        _ => {
            let _ = &event.event;
            Vec::new()
        }
    }
}

fn terminal_chunk(parsed: &Value, kind: &str, state: &mut ChatStreamState) -> Option<Value> {
    if !matches!(
        kind,
        "response.completed"
            | "response.done"
            | "response.incomplete"
            | "response.failed"
            | "error"
            | "response.cancelled"
            | "response.canceled"
    ) {
        return None;
    }
    state.terminal_received = true;
    let response = parsed.get("response").unwrap_or(parsed);
    let status = response.get("status").and_then(Value::as_str).unwrap_or("");
    let reason = response
        .pointer("/incomplete_details/reason")
        .and_then(Value::as_str);
    let finish = if kind == "response.incomplete" || status == "incomplete" {
        match reason {
            Some("max_output_tokens") => Some("length"),
            Some("content_filter") => Some("content_filter"),
            _ => None,
        }
    } else if matches!(kind, "response.completed" | "response.done")
        && !matches!(status, "failed" | "cancelled" | "canceled")
    {
        Some(if state.function_call_index >= 0 {
            "tool_calls"
        } else {
            "stop"
        })
    } else {
        None
    };
    Some(match finish {
        Some(reason) => build_chat_chunk(state, json!({}), Some(reason), response.get("usage")),
        None => {
            json!({"error": response.get("error").filter(|error| error.is_object()).cloned().unwrap_or_else(|| json!({"type":"upstream_error", "message": format!("Upstream response ended with {kind}: {}", reason.unwrap_or(status))}))})
        }
    })
}

pub(super) fn unterminated_stream_error(state: &ChatStreamState) -> Option<Value> {
    (!state.terminal_received).then(|| json!({"error":{"type":"upstream_error", "message":"Upstream stream ended before a terminal response event"}}))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn translate(value: Value, state: &mut ChatStreamState) -> Vec<Value> {
        translate_sse_event_to_chat_chunk(
            &SseEvent {
                event: None,
                data: value.to_string(),
            },
            state,
        )
    }
    #[test]
    fn completion_alias_produces_exactly_one_finish_reason() {
        let mut state = ChatStreamState::default();
        let event = json!({"type":"response.done", "response":{"status":"completed"}});
        assert_eq!(
            translate(event.clone(), &mut state)[0]["choices"][0]["finish_reason"],
            "stop"
        );
        assert!(translate(event, &mut state).is_empty());
        assert!(unterminated_stream_error(&state).is_none());
    }
    #[test]
    fn incomplete_and_failed_events_do_not_disappear() {
        let result = translate(
            json!({"type":"response.incomplete", "response":{"incomplete_details":{"reason":"max_output_tokens"}}}),
            &mut ChatStreamState::default(),
        );
        assert_eq!(result[0]["choices"][0]["finish_reason"], "length");
        let result = translate(
            json!({"type":"response.failed", "response":{"error":{"code":"model_not_enabled", "message":"Luna unavailable"}}}),
            &mut ChatStreamState::default(),
        );
        assert_eq!(result[0]["error"]["message"], "Luna unavailable");
        assert!(result[0].get("choices").is_none());
    }
    #[test]
    fn truncated_stream_is_an_error_not_a_successful_empty_response() {
        assert!(unterminated_stream_error(&ChatStreamState::default())
            .unwrap()
            .get("error")
            .is_some());
    }
}
