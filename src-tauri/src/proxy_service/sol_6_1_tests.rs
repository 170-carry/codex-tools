use super::*;

const MODEL: &str = "gpt-6.1-sol";
const ALIASES: &[&str] = &[
    MODEL,
    "gpt6.1-sol",
    "gpt-6-1-sol",
    "gpt-6.1",
    "gpt6.1",
    "gpt-6-1",
];

fn assert_sol_lite(payload: &Value) {
    assert_eq!(payload["model"], MODEL);
    assert_eq!(payload["input"][0]["type"], "additional_tools");
    assert_eq!(payload["reasoning"]["context"], "all_turns");
    assert_eq!(payload["parallel_tool_calls"], false);
    assert!(payload_uses_responses_lite(payload));
}

#[test]
fn catalog_aliases_and_snapshots_select_sol_6_1_without_astra_remapping() {
    assert!(get_api_proxy_supported_models_internal().contains(&MODEL.to_string()));
    for alias in ALIASES {
        assert_eq!(map_client_model_to_upstream(alias).unwrap(), MODEL);
        assert_eq!(normalize_model_for_client(alias), MODEL);
        let snapshot = format!("{alias}-2026-09-29");
        let mapped = map_client_model_to_upstream(&snapshot).unwrap();
        assert_eq!(mapped, "gpt-6.1-sol-2026-09-29");
        assert!(is_responses_lite_model(&mapped));
        assert_eq!(
            model_catalog::normalize_model_for_permissions(&snapshot),
            MODEL
        );
    }
    assert!(!is_responses_lite_model("gpt-6.1-solish"));
    assert_eq!(normalize_model_for_client("custom-model"), "custom-model");
    assert_eq!(normalize_model_for_client("gpt-6-sol"), "gpt-6-sol");
    assert_eq!(normalize_model_for_client("gpt-6"), "gpt-6-astra");
}

#[test]
fn responses_and_compact_accept_documented_sol_6_1_efforts() {
    for effort in ["low", "medium", "high", "xhigh", "max", "ultra"] {
        let (payload, _) = normalize_openai_responses_request(json!({
            "model": MODEL, "input": "hello", "reasoning": { "effort": effort },
            "instructions": "Be concise.",
            "tools": [{"type": "function", "name": "lookup", "parameters": {"type": "object"}}]
        }))
        .unwrap();
        assert_sol_lite(&payload);
        assert_eq!(payload["input"][0]["tools"][0]["name"], "lookup");
        assert!(payload.get("tools").is_none());
        assert!(payload.get("instructions").is_none());
        let expected = if effort == "ultra" { "max" } else { effort };
        assert_eq!(payload["reasoning"]["effort"], expected);

        let compact = normalize_openai_compact_request(json!({
            "model": "gpt6.1", "input": [], "reasoning": { "effort": effort },
            "prompt_cache_key": "session-1"
        }))
        .unwrap();
        let upstream = normalize_codex_compact_reasoning_effort(compact);
        assert_eq!(upstream["model"], MODEL);
        assert_eq!(upstream["reasoning"]["effort"], expected);
        assert_eq!(upstream["prompt_cache_key"], "session-1");
    }
}

#[test]
fn chat_anthropic_and_websocket_use_sol_6_1_lite_transport() {
    let (chat, _) = convert_openai_chat_request_to_codex(&json!({
        "model": "gpt6.1", "messages": [{"role": "user", "content": "hello"}]
    }))
    .unwrap();
    assert_sol_lite(&chat);
    let (anthropic, _) = convert_anthropic_messages_request_to_codex(&json!({
        "model": "gpt-6-1-sol", "max_tokens": 10, "output_config": {"effort": "low"},
        "messages": [{"role": "user", "content": "hello"}]
    }))
    .unwrap();
    assert_sol_lite(&anthropic);
    assert_eq!(anthropic["reasoning"]["effort"], "low");
    let request = serde_json::to_vec(&json!({
        "type": "response.create", "model": "gpt6.1-sol", "input": "hello"
    }))
    .unwrap();
    let websocket = normalize_responses_websocket_create(&request).unwrap();
    assert_sol_lite(&websocket);
    let upstream = websocket_response_create_payload(&websocket, None);
    assert_eq!(
        upstream["client_metadata"]["ws_request_header_x_openai_internal_codex_responses_lite"],
        "true"
    );
    let mut headers = HeaderMap::new();
    headers.insert("version", HeaderValue::from_static("0.155.1"));
    headers.insert("user-agent", HeaderValue::from_static("old-client"));
    assert_eq!(
        upstream_codex_client_identity(&headers, true),
        ("0.159.0", "codex_cli_rs/0.159.0")
    );
}

#[test]
fn unsupported_sol_6_1_efforts_fail_before_any_transport() {
    for effort in ["none", "minimal", " NONE "] {
        let error = normalize_openai_responses_request(json!({
            "model": MODEL, "input": "hello", "reasoning": {"effort": effort}
        }))
        .unwrap_err();
        assert!(error.contains("GPT-6.1 Sol"), "{error}");
        let error = normalize_openai_compact_request(json!({
            "model": MODEL, "input": [], "reasoning": {"effort": effort}
        }))
        .unwrap_err();
        assert!(error.contains("GPT-6.1 Sol"), "{error}");
        let error = convert_openai_chat_request_to_codex(&json!({
            "model": MODEL, "messages": [{"role": "user", "content": "hello"}],
            "reasoning_effort": effort
        }))
        .unwrap_err();
        assert!(error.contains("GPT-6.1 Sol"), "{error}");
        let error = convert_anthropic_messages_request_to_codex(&json!({
            "model": MODEL, "messages": [{"role": "user", "content": "hello"}],
            "reasoning_effort": effort
        }))
        .unwrap_err();
        assert!(error.contains("GPT-6.1 Sol"), "{error}");
        let request = serde_json::to_vec(&json!({
            "type": "response.create", "model": MODEL, "input": "hello",
            "reasoning": {"effort": effort}
        }))
        .unwrap();
        let error = normalize_responses_websocket_create(&request).unwrap_err();
        assert!(error.contains("GPT-6.1 Sol"), "{error}");
    }
}

#[test]
fn sol_6_1_aliases_and_snapshots_obey_model_and_key_restrictions() {
    let key = ApiProxyKey {
        allowed_models: vec![MODEL.to_string()],
        ..ApiProxyKey::default()
    };
    let old_key = ApiProxyKey {
        allowed_models: vec!["gpt-6-sol".to_string()],
        ..ApiProxyKey::default()
    };
    for alias in ALIASES {
        let settings = AppSettings {
            api_proxy_disabled_models: vec![alias.to_string()],
            ..AppSettings::default()
        };
        for requested in [alias.to_string(), format!("{alias}-2026-09-29")] {
            let (payload, _) = normalize_openai_responses_request(json!({
                "model": requested, "input": "hello"
            }))
            .unwrap();
            assert!(ensure_api_proxy_payload_models_enabled(&payload, &settings).is_err());
            assert!(ensure_api_proxy_key_allows_payload(&key, &payload).is_ok());
            assert!(ensure_api_proxy_key_allows_payload(&old_key, &payload).is_err());
        }
        assert_eq!(
            sanitize_api_proxy_allowed_models(vec![alias.to_string()]),
            vec![MODEL]
        );
        assert_eq!(
            sanitize_api_proxy_disabled_models_for_settings(vec![alias.to_string()]),
            vec![MODEL]
        );
    }
}
