use super::*;

#[test]
fn terra_aliases_use_lite_and_preserve_key_permissions() {
    for model in [
        "gpt-6.1-terra",
        "gpt6.1-terra",
        "gpt-6-1-terra",
        "gpt-6.1-terra-2026-10-29",
    ] {
        let (payload, _) =
            normalize_openai_responses_request(json!({"model":model,"input":"hello"})).unwrap();
        assert!(payload["model"]
            .as_str()
            .unwrap()
            .starts_with("gpt-6.1-terra"));
        assert_eq!(payload["input"][0]["type"], "additional_tools");
        let old_key = ApiProxyKey {
            allowed_models: vec!["gpt-6.1-sol".into()],
            ..ApiProxyKey::default()
        };
        assert!(ensure_api_proxy_key_allows_payload(&old_key, &payload).is_err());
        let key = ApiProxyKey {
            allowed_models: vec!["gpt-6.1-terra".into()],
            ..ApiProxyKey::default()
        };
        assert!(ensure_api_proxy_key_allows_payload(&key, &payload).is_ok());
        let settings = AppSettings {
            api_proxy_disabled_models: vec!["gpt-6.1-terra".into()],
            ..AppSettings::default()
        };
        assert!(ensure_api_proxy_payload_models_enabled(&payload, &settings).is_err());
    }
    let catalog = codex_catalog::catalog_for_key(&AppSettings::default(), &ApiProxyKey::default());
    let terra = catalog["models"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["slug"] == "gpt-6.1-terra")
        .unwrap();
    assert_eq!(terra["use_responses_lite"], true);
    assert!(terra["base_instructions"].as_str().unwrap().len() > 1000);
    assert_eq!(terra["tool_mode"], "code_mode_only");
}

#[test]
fn image_2_5_models_reach_native_transport_and_respect_switches() {
    for model in ["gpt-image-2.5-sunburst", "gpt-image-2.5-flare"] {
        assert!(MODELS.contains(&model));
        for wire_model in [model.to_string(), format!("{model}-2026-09-08")] {
            let payload = json!({"model":"gpt-5.5", "input":[{"role":"user","content":[{"type":"input_text","text":"circle"}]}], "tools":[{"type":"image_generation","model":wire_model}]});
            assert_eq!(native_image_payload(&payload).unwrap()["model"], wire_model);
            let settings = AppSettings {
                api_proxy_disabled_models: vec![model.into()],
                ..AppSettings::default()
            };
            assert!(ensure_api_proxy_payload_models_enabled(&payload, &settings).is_err());
        }
    }
}
