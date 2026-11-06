use super::model_catalog::{is_responses_lite_model, validate_reasoning};
use serde_json::{json, Map, Value};

pub(super) fn normalize_responses_lite_payload(
    object: &mut Map<String, Value>,
) -> Result<(), String> {
    let Some(model) = object.get("model").and_then(Value::as_str) else {
        return Ok(());
    };
    if !is_responses_lite_model(model) {
        return Ok(());
    }
    validate_reasoning(
        model,
        object
            .get("reasoning")
            .and_then(|reasoning| reasoning.get("effort"))
            .and_then(Value::as_str),
    )?;

    if object.contains_key("tools") && !object.get("tools").is_some_and(Value::is_array) {
        return Err("Responses Lite 的 tools 必须是数组".to_string());
    }
    if object.contains_key("instructions")
        && !object.get("instructions").is_some_and(Value::is_string)
    {
        return Err("Responses Lite 的 instructions 必须是字符串".to_string());
    }

    let extra_tools = object
        .remove("tools")
        .and_then(|value| value.as_array().cloned())
        .unwrap_or_default();
    let instructions = object
        .remove("instructions")
        .and_then(|value| value.as_str().map(ToString::to_string))
        .filter(|value| !value.is_empty());
    let mut input = responses_lite_input_items(object.remove("input"))?;
    for item in &mut input {
        prepare_responses_lite_images(item);
    }

    // Incremental tool catalogs belong at their original conversation positions.
    // Validate every update, but only merge top-level tools into the prefix.
    for item in &mut input {
        if item.get("type").and_then(Value::as_str) == Some("additional_tools") {
            if !item.get("tools").is_some_and(Value::is_array) {
                return Err("Responses Lite 的 additional_tools.tools 必须是数组".to_string());
            }
            item.as_object_mut()
                .expect("tool item object")
                .insert("role".to_string(), Value::String("developer".to_string()));
        }
    }

    let already_has_additional_tools = input
        .first()
        .and_then(Value::as_object)
        .and_then(|item| item.get("type"))
        .and_then(Value::as_str)
        == Some("additional_tools");

    if already_has_additional_tools {
        let tools_are_array = input
            .first()
            .and_then(Value::as_object)
            .and_then(|item| item.get("tools"))
            .is_some_and(Value::is_array);
        if !tools_are_array {
            return Err("Responses Lite 的 additional_tools.tools 必须是数组".to_string());
        }
        if let Some(item) = input.first_mut().and_then(Value::as_object_mut) {
            item.insert("role".to_string(), Value::String("developer".to_string()));
        }
        if !extra_tools.is_empty() {
            if let Some(tools) = input
                .first_mut()
                .and_then(Value::as_object_mut)
                .and_then(|item| item.get_mut("tools"))
                .and_then(Value::as_array_mut)
            {
                tools.extend(extra_tools);
            }
        }
    } else {
        input.insert(
            0,
            json!({
                "type": "additional_tools",
                "role": "developer",
                "tools": extra_tools,
            }),
        );
    }

    if let Some(instructions) = instructions {
        input.insert(
            1,
            json!({
                "type": "message",
                "role": "developer",
                "content": [{
                    "type": "input_text",
                    "text": instructions,
                }],
            }),
        );
    }

    if let Some(tool_type) = responses_lite_unsupported_hosted_tool(&input) {
        return Err(format!(
            "Responses Lite 不支持 hosted tool 类型 {tool_type}；请使用客户端扩展工具"
        ));
    }

    object.insert("input".to_string(), Value::Array(input));
    object.insert("parallel_tool_calls".to_string(), Value::Bool(false));
    if !object.contains_key("tool_choice") {
        object.insert("tool_choice".to_string(), Value::String("auto".to_string()));
    }

    let reasoning = object
        .entry("reasoning".to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    if !reasoning.is_object() {
        *reasoning = Value::Object(Map::new());
    }
    if let Some(reasoning) = reasoning.as_object_mut() {
        reasoning.insert(
            "context".to_string(),
            Value::String("all_turns".to_string()),
        );
    }

    Ok(())
}

fn responses_lite_unsupported_hosted_tool(input: &[Value]) -> Option<&str> {
    let tools = input
        .iter()
        .filter(|item| item.get("type").and_then(Value::as_str) == Some("additional_tools"))
        .filter_map(|item| item.get("tools").and_then(Value::as_array))
        .flatten();

    tools.into_iter().find_map(|tool| {
        let tool_type = tool.get("type").and_then(Value::as_str)?;
        matches!(
            tool_type,
            "web_search" | "web_search_preview" | "image_generation"
        )
        .then_some(tool_type)
    })
}

fn responses_lite_input_items(input: Option<Value>) -> Result<Vec<Value>, String> {
    match input {
        Some(Value::Array(items)) => Ok(items),
        Some(Value::String(text)) => Ok(vec![json!({
            "type": "message",
            "role": "user",
            "content": [{
                "type": "input_text",
                "text": text,
            }],
        })]),
        Some(Value::Null) | None => Ok(Vec::new()),
        Some(_) => Err("Responses Lite 的 input 必须是字符串或数组".to_string()),
    }
}

fn is_remote_image_url(url: &str) -> bool {
    url.split_once(':').is_some_and(|(scheme, _)| {
        scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https")
    })
}

fn is_data_image_url(url: &str) -> bool {
    url.get(.."data:".len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("data:"))
}

fn prepare_responses_lite_images(value: &mut Value) {
    let remote_image_url = value.as_object().is_some_and(|object| {
        object.get("type").and_then(Value::as_str) == Some("input_image")
            && object
                .get("image_url")
                .and_then(Value::as_str)
                .is_some_and(is_remote_image_url)
    });
    if remote_image_url {
        *value = json!({
            "type": "input_text",
            "text": "image content omitted because remote image URLs are not supported",
        });
        return;
    }

    let unsupported_low_detail = value.as_object().is_some_and(|object| {
        object.get("type").and_then(Value::as_str) == Some("input_image")
            && object
                .get("image_url")
                .and_then(Value::as_str)
                .is_some_and(is_data_image_url)
            && object
                .get("detail")
                .and_then(Value::as_str)
                .is_some_and(|detail| detail.eq_ignore_ascii_case("low"))
    });
    if unsupported_low_detail {
        *value = json!({
            "type": "input_text",
            "text": "image content omitted because detail 'low' is not supported; use 'high', 'original', or 'auto'",
        });
        return;
    }

    match value {
        Value::Object(object) => {
            if object.get("type").and_then(Value::as_str) == Some("additional_tools") {
                return;
            }
            if object.get("type").and_then(Value::as_str) == Some("input_image") {
                object.remove("detail");
            }
            for value in object.values_mut() {
                prepare_responses_lite_images(value);
            }
        }
        Value::Array(items) => {
            for item in items {
                prepare_responses_lite_images(item);
            }
        }
        _ => {}
    }
}
