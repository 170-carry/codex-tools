use serde_json::{json, Map, Value};

// The standalone Images API used by the official Codex image-generation
// extension has no text controller model and returns a completed JSON result.
// Contract: openai/codex at 8c076942cb70fba8e5c4732c23f1faabc4aed484,
// codex-rs/codex-api/src/images.rs and endpoint/images.rs.
pub(super) fn native_image_payload(payload: &Value) -> Result<Value, String> {
    let tools = payload
        .get("tools")
        .and_then(Value::as_array)
        .ok_or_else(|| "图片请求缺少 image_generation 工具".to_string())?;
    let tool = match tools.as_slice() {
        [tool] if tool.get("type").and_then(Value::as_str) == Some("image_generation") => tool,
        _ => return Err("原生图片请求必须包含且仅包含一个 image_generation 工具".to_string()),
    };
    let model = tool
        .get("model")
        .and_then(Value::as_str)
        .filter(|model| !model.trim().is_empty())
        .ok_or_else(|| "图片请求缺少图片模型".to_string())?;

    if let Some(format) = tool.get("output_format") {
        if format.as_str() != Some("png") {
            return Err(
                "ChatGPT 原生图片接口暂只支持 PNG 输出；请省略 output_format 或设为 png"
                    .to_string(),
            );
        }
    }
    if tool.get("output_compression").is_some() {
        return Err("ChatGPT 原生图片接口暂不支持 output_compression/compression 参数".to_string());
    }
    if let Some(fidelity) = tool.get("input_fidelity") {
        if fidelity.as_str() != Some("high") {
            return Err(
                "ChatGPT 原生图片接口暂不支持自定义 input_fidelity；请省略该参数或设为 high"
                    .to_string(),
            );
        }
    }

    let input = payload
        .get("input")
        .and_then(Value::as_array)
        .ok_or_else(|| "图片请求缺少 input 数组".to_string())?;
    let mut text = Vec::new();
    let mut images = Vec::new();
    for message in input {
        if message.get("role").and_then(Value::as_str) != Some("user") {
            continue;
        }
        let Some(content) = message.get("content").and_then(Value::as_array) else {
            continue;
        };
        for item in content {
            match item.get("type").and_then(Value::as_str) {
                Some("input_text") => {
                    if let Some(value) = item.get("text").and_then(Value::as_str) {
                        text.push(value);
                    }
                }
                Some("input_image") => {
                    let mut image = Map::new();
                    for key in ["image_url", "file_id"] {
                        if let Some(value) = item
                            .get(key)
                            .and_then(Value::as_str)
                            .filter(|value| !value.trim().is_empty())
                        {
                            image.insert(key.to_string(), Value::String(value.to_string()));
                        }
                    }
                    if image.len() != 1 {
                        return Err(
                            "原生图片编辑的每张图片必须提供一个 image_url 或 file_id".to_string()
                        );
                    }
                    images.push(Value::Object(image));
                }
                _ => {}
            }
        }
    }
    let prompt = text.join("\n\n");
    if prompt.trim().is_empty() {
        return Err("图片请求缺少非空提示词".to_string());
    }

    let mut request = Map::from_iter([
        ("model".to_string(), Value::String(model.to_string())),
        ("prompt".to_string(), Value::String(prompt)),
        // The proxy owns the n-image loop so a candidate retry cannot multiply n.
        ("n".to_string(), json!(1)),
    ]);
    for key in ["size", "quality", "background"] {
        if let Some(value) = tool.get(key) {
            request.insert(key.to_string(), value.clone());
        }
    }
    if !images.is_empty() {
        request.insert("images".to_string(), Value::Array(images));
    }
    // Accepted PNG/high compatibility values need no native request fields.
    // partial_images is omitted: this API returns JSON, and downstream SSE emits
    // only completion.
    Ok(Value::Object(request))
}

pub(super) fn validate_native_image_response(response: &Value) -> Result<(), String> {
    let data = response
        .get("data")
        .and_then(Value::as_array)
        .filter(|data| !data.is_empty())
        .ok_or_else(|| "Codex 原生图片响应中没有图片数据 data".to_string())?;
    if data.iter().any(|item| {
        item.get("b64_json")
            .and_then(Value::as_str)
            .map_or(true, |value| value.trim().is_empty())
    }) {
        return Err("Codex 原生图片响应包含缺失或为空的 b64_json".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn generation_payload() -> Value {
        json!({
            "model": "gpt-5.5",
            "stream": true,
            "reasoning": {"effort": "medium"},
            "input": [{
                "type": "message",
                "role": "user",
                "content": [{"type": "input_text", "text": "Draw a blue circle"}],
            }],
            "tools": [{
                "type": "image_generation",
                "model": "gpt-image-2",
                "action": "generate",
                "quality": "high",
                "size": "1024x1024",
                "background": "transparent",
                "partial_images": 2,
            }],
        })
    }

    #[test]
    fn generation_uses_the_image_model_without_controller_or_stream_options() {
        let request = native_image_payload(&generation_payload()).expect("native generation");
        assert_eq!(
            request,
            json!({
                "model": "gpt-image-2",
                "prompt": "Draw a blue circle",
                "n": 1,
                "quality": "high",
                "size": "1024x1024",
                "background": "transparent",
            })
        );
    }

    #[test]
    fn edit_preserves_ordered_image_references_and_mask_instructions() {
        let mut payload = generation_payload();
        payload["tools"][0]["action"] = json!("edit");
        payload["input"][0]["content"] = json!([
            {"type": "input_text", "text": "Change the hat"},
            {"type": "input_image", "image_url": "data:image/png;base64,source", "detail": "high"},
            {"type": "input_image", "file_id": "file-reference"},
            {"type": "input_text", "text": "Use the following image as the edit mask."},
            {"type": "input_image", "image_url": "data:image/png;base64,mask"},
        ]);
        let request = native_image_payload(&payload).expect("native edit");
        assert_eq!(
            request["prompt"],
            "Change the hat\n\nUse the following image as the edit mask."
        );
        assert_eq!(
            request["images"],
            json!([
                {"image_url": "data:image/png;base64,source"},
                {"file_id": "file-reference"},
                {"image_url": "data:image/png;base64,mask"},
            ])
        );
    }

    #[test]
    fn unsupported_options_fail_instead_of_silently_changing_output() {
        for (field, value) in [
            ("output_format", json!("jpeg")),
            ("output_format", json!("webp")),
            ("output_compression", json!(80)),
            ("input_fidelity", json!("low")),
        ] {
            let mut payload = generation_payload();
            payload["tools"][0][field] = value;
            let error = native_image_payload(&payload).expect_err("unsupported native option");
            assert!(error.contains(field), "{error}");
        }
        let mut payload = generation_payload();
        payload["tools"][0]["output_format"] = json!("png");
        payload["tools"][0]["input_fidelity"] = json!("high");
        let request = native_image_payload(&payload).expect("native defaults are accepted");
        assert!(request.get("output_format").is_none());
        assert!(request.get("input_fidelity").is_none());
    }

    #[test]
    fn edit_rejects_missing_or_ambiguous_image_references() {
        for image in [
            json!({"type": "input_image"}),
            json!({"type": "input_image", "image_url": " "}),
            json!({"type": "input_image", "image_url": "data:image/png;base64,a", "file_id": "file-a"}),
        ] {
            let mut payload = generation_payload();
            payload["input"][0]["content"]
                .as_array_mut()
                .expect("content array")
                .push(image);
            assert!(native_image_payload(&payload).is_err());
        }
    }

    #[test]
    fn response_requires_nonempty_image_data_for_every_result() {
        for response in [
            json!({}),
            json!({"data": []}),
            json!({"data": [{}]}),
            json!({"data": [{"b64_json": ""}]}),
            json!({"data": [{"b64_json": "valid"}, {"b64_json": " "}]}),
        ] {
            assert!(validate_native_image_response(&response).is_err());
        }
        assert!(validate_native_image_response(&json!({
            "created": 1,
            "data": [{"b64_json": "image-one", "generation_id": "gen-one"}, {"b64_json": "image-two"}],
            "usage": {"total_tokens": 42},
        }))
        .is_ok());
    }
}
