use super::*;

#[test]
fn astra_catalog_aliases_and_snapshots_are_consistent() {
    assert!(MODELS.contains(&"gpt-6-astra"));
    for alias in ["gpt-6-astra", "gpt6-astra", "gpt6", "gpt-6"] {
        assert_eq!(map_client_model_to_upstream(alias).unwrap(), "gpt-6-astra");
        assert_eq!(normalize_model_for_client(alias), "gpt-6-astra");
    }
    let model = map_client_model_to_upstream("gpt6-astra-2026-09-03").unwrap();
    assert_eq!(model, "gpt-6-astra-2026-09-03");
    assert!(is_responses_lite_model(&model));
    assert!(!is_responses_lite_model("gpt-6-astraish"));
    assert_eq!(
        map_client_model_to_upstream("custom-model").unwrap(),
        "custom-model"
    );
}

#[test]
fn sol_and_luna_use_codex_0155_responses_lite() {
    for (model, alias) in [("gpt-6-sol", "gpt6-sol"), ("gpt-6-luna", "gpt6-luna")] {
        assert!(MODELS.contains(&model));
        assert_eq!(map_client_model_to_upstream(alias).unwrap(), model);
        assert_eq!(normalize_model_for_client(alias), model);
        let (payload, _) = normalize_openai_responses_request(json!({
            "model": alias,
            "input": "hello",
            "reasoning": { "effort": "medium" }
        }))
        .unwrap();
        assert_eq!(payload["model"], model);
        assert_eq!(payload["input"][0]["type"], "additional_tools");
        assert_eq!(payload["reasoning"]["context"], "all_turns");
        assert!(payload_uses_responses_lite(&payload));
        assert!(normalize_openai_responses_request(json!({
            "model": model,
            "input": "hello",
            "reasoning": { "effort": "none" }
        }))
        .is_err());
    }
    assert_eq!(
        upstream_codex_client_identity(&HeaderMap::new(), true),
        ("0.155.1", "codex_cli_rs/0.155.1")
    );
}

fn assert_astra_lite(payload: &Value) {
    assert_eq!(payload["model"], "gpt-6-astra");
    assert_eq!(payload["input"][0]["type"], "additional_tools");
    assert_eq!(payload["reasoning"]["context"], "all_turns");
    assert_eq!(payload["parallel_tool_calls"], false);
    assert!(payload_uses_responses_lite(payload));
}

#[test]
fn astra_responses_supports_tools_and_all_documented_efforts() {
    for effort in ["low", "medium", "high", "xhigh", "max", "ultra"] {
        let (payload, _) = normalize_openai_responses_request(json!({
            "model": "gpt-6", "input": "hello", "instructions": "Be concise.",
            "reasoning": {"effort": effort},
            "tools": [{"type": "function", "name": "lookup", "parameters": {"type": "object"}}]
        }))
        .unwrap();
        assert_astra_lite(&payload);
        assert_eq!(payload["input"][0]["tools"][0]["name"], "lookup");
        assert_eq!(
            payload["reasoning"]["effort"],
            if effort == "ultra" { "max" } else { effort }
        );
        assert!(payload.get("instructions").is_none());
    }
}

#[test]
fn astra_rejects_none_and_minimal_before_upstream() {
    for effort in ["none", "minimal", " NONE "] {
        let error = normalize_openai_responses_request(json!({
            "model": "gpt-6-astra", "input": "hello", "reasoning": {"effort": effort}
        }))
        .unwrap_err();
        assert!(error.contains("GPT-6 Astra"), "{error}");
        assert!(normalize_openai_compact_request(json!({
            "model": "gpt-6-astra", "input": [], "reasoning": {"effort": effort}
        }))
        .is_err());
    }
}

#[test]
fn astra_chat_anthropic_and_websocket_share_the_lite_transport() {
    let (chat, _) = convert_openai_chat_request_to_codex(&json!({
        "model": "gpt6", "messages": [{"role": "user", "content": "hello"}]
    }))
    .unwrap();
    assert_astra_lite(&chat);
    let (anthropic, _) = convert_anthropic_messages_request_to_codex(&json!({
        "model": "gpt-6-astra", "max_tokens": 10,
        "output_config": {"effort": "low"},
        "messages": [{"role": "user", "content": "hello"}]
    }))
    .unwrap();
    assert_astra_lite(&anthropic);
    assert_eq!(anthropic["reasoning"]["effort"], "low");
    let request = serde_json::to_vec(&json!({
        "type": "response.create", "model": "gpt6-astra", "input": "hello"
    }))
    .unwrap();
    let websocket = normalize_responses_websocket_create(&request).unwrap();
    assert_astra_lite(&websocket);
    let upstream = websocket_response_create_payload(&websocket, Some("turn"));
    assert_eq!(
        upstream["client_metadata"]["ws_request_header_x_openai_internal_codex_responses_lite"],
        "true"
    );
    assert_eq!(upstream["client_metadata"][CODEX_TURN_STATE_HEADER], "turn");
}

#[test]
fn astra_uses_supported_client_version_even_for_old_downstream_clients() {
    let mut headers = HeaderMap::new();
    headers.insert("version", HeaderValue::from_static("0.144.0"));
    headers.insert("user-agent", HeaderValue::from_static("old-client"));
    assert_eq!(
        upstream_codex_client_identity(&headers, true),
        ("0.155.1", "codex_cli_rs/0.155.1")
    );
    assert_eq!(
        upstream_codex_client_identity(&HeaderMap::new(), false),
        ("0.155.1", "codex_cli_rs/0.155.1")
    );
}

#[test]
fn astra_compact_preserves_max_and_custom_fields() {
    let normalized = normalize_openai_compact_request(json!({
        "model": "gpt6-astra", "input": [], "prompt_cache_key": "session-1",
        "reasoning": {"effort": "ultra"}, "service_tier": "fast"
    }))
    .unwrap();
    let upstream = normalize_codex_compact_reasoning_effort(normalized);
    assert_eq!(upstream["model"], "gpt-6-astra");
    assert_eq!(upstream["reasoning"]["effort"], "max");
    assert_eq!(upstream["prompt_cache_key"], "session-1");
    assert_eq!(upstream["service_tier"], "priority");
}

#[test]
fn astra_aliases_and_snapshots_obey_catalog_family_permissions() {
    for configured in ["gpt-6-astra", "gpt6", "gpt6-astra-2026-09-03"] {
        let settings = AppSettings {
            api_proxy_disabled_models: vec![configured.to_string()],
            ..AppSettings::default()
        };
        let allowed_key = ApiProxyKey {
            allowed_models: vec![configured.to_string()],
            ..ApiProxyKey::default()
        };
        let other_key = ApiProxyKey {
            allowed_models: vec!["gpt-5.6-sol".to_string()],
            ..ApiProxyKey::default()
        };

        for (requested, upstream_model) in [
            ("gpt-6-astra", "gpt-6-astra"),
            ("gpt6", "gpt-6-astra"),
            ("gpt-6", "gpt-6-astra"),
            ("gpt6-astra", "gpt-6-astra"),
            ("gpt6-astra-2026-09-03", "gpt-6-astra-2026-09-03"),
            ("gpt-6-astra-2026-09-03", "gpt-6-astra-2026-09-03"),
        ] {
            let (payload, _) = normalize_openai_responses_request(json!({
                "model": requested,
                "input": "hello"
            }))
            .unwrap();
            assert_eq!(payload["model"], upstream_model);
            assert!(
                ensure_api_proxy_payload_models_enabled(&payload, &settings).is_err(),
                "disabled {configured} must reject {requested}"
            );
            assert!(
                ensure_api_proxy_key_allows_payload(&allowed_key, &payload).is_ok(),
                "allowed {configured} must accept {requested}"
            );
            assert!(ensure_api_proxy_key_allows_payload(&other_key, &payload).is_err());
        }
    }
}

#[test]
fn base_model_permissions_do_not_grant_distinct_pro_or_mini_variants() {
    for model in ["gpt-5.4-pro", "gpt-5.4-mini", "gpt-5.5-pro"] {
        assert_eq!(model_catalog::normalize_model_for_permissions(model), model);
    }
    assert_eq!(
        model_catalog::normalize_model_for_permissions("gpt-5.4-2026-03-05"),
        "gpt-5.4"
    );
}

#[test]
fn invalid_request_errors_are_not_retried_across_the_account_pool() {
    assert!(classify_retriable_failure(
        StatusCode::BAD_REQUEST,
        &Bytes::from_static(br#"{"error":{"message":"Unsupported parameter: temperature"}}"#)
    )
    .is_none());
}
