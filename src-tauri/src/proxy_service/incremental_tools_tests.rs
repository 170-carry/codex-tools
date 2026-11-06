use super::*;

fn request() -> Value {
    json!({"model":"gpt-6.1-sol", "input":[
        {"type":"message","role":"user","content":"first"},
        {"type":"additional_tools","role":"developer","tools":[{"type":"function","name":"first_tool","parameters":{}}]},
        {"type":"message","role":"user","content":"second"},
        {"type":"additional_tools","role":"developer","tools":[{"type":"function","name":"second_tool","parameters":{}}]}
    ]})
}

#[test]
fn incremental_tools_preserve_conversation_order_and_normalization_is_idempotent() {
    let original = request();
    let (payload, _) = normalize_openai_responses_request(original.clone()).unwrap();
    assert_eq!(
        &payload["input"].as_array().unwrap()[1..],
        original["input"].as_array().unwrap()
    );
    let (again, _) = normalize_openai_responses_request(payload.clone()).unwrap();
    assert_eq!(again, payload);
    let websocket =
        normalize_responses_websocket_create(&serde_json::to_vec(&original).unwrap()).unwrap();
    assert_eq!(websocket["input"], payload["input"]);
}

#[test]
fn prefix_tools_and_instructions_do_not_merge_later_catalogs() {
    let mut original = request();
    original["tools"] = json!([{"type":"function","name":"prefix_tool","parameters":{}}]);
    original["instructions"] = json!("Be concise");
    let (payload, _) = normalize_openai_responses_request(original.clone()).unwrap();
    assert_eq!(payload["input"][0]["tools"][0]["name"], "prefix_tool");
    assert_eq!(payload["input"][1]["content"][0]["text"], "Be concise");
    assert_eq!(
        &payload["input"].as_array().unwrap()[2..],
        original["input"].as_array().unwrap()
    );
}

#[test]
fn every_incremental_catalog_is_validated() {
    let mut original = request();
    original["input"][3]["tools"] = json!({});
    assert!(normalize_openai_responses_request(original)
        .unwrap_err()
        .contains("必须是数组"));
    for hosted in ["web_search", "image_generation", "web_search_preview"] {
        let mut original = request();
        original["input"][3]["tools"] = json!([{"type":hosted}]);
        assert!(normalize_openai_responses_request(original)
            .unwrap_err()
            .contains("hosted tool"));
    }
}
