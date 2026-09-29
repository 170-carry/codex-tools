use axum::http::HeaderMap;

// GPT-6 Sol and Luna require Codex CLI 0.155.0 or newer. Keep both headers aligned.
pub(super) const CODEX_CLIENT_VERSION: &str = "0.155.1";
pub(super) const CODEX_USER_AGENT: &str = "codex_cli_rs/0.155.1";

pub(super) const MODELS: &[&str] = &[
    "gpt-6-astra",
    "gpt-6-sol",
    "gpt-6-luna",
    "gpt-5.6-sol",
    "gpt-5.6-terra",
    "gpt-5.6-luna",
    "gpt-5.5",
    "gpt-5.4",
    "gpt-image-2",
];
const MODEL_MAPPINGS: &[(&str, &str)] = &[
    ("gpt-6-astra", "gpt-6-astra"),
    ("gpt6-astra", "gpt-6-astra"),
    ("gpt-6-sol", "gpt-6-sol"),
    ("gpt6-sol", "gpt-6-sol"),
    ("gpt-6-luna", "gpt-6-luna"),
    ("gpt6-luna", "gpt-6-luna"),
    ("gpt6", "gpt-6-astra"),
    ("gpt-6", "gpt-6-astra"),
    ("gpt-5.6-sol", "gpt-5.6-sol"),
    ("gpt-5.6-terra", "gpt-5.6-terra"),
    ("gpt-5.6-luna", "gpt-5.6-luna"),
    ("gpt5.6-sol", "gpt-5.6-sol"),
    ("gpt-5-6-sol", "gpt-5.6-sol"),
    ("gpt5.6-terra", "gpt-5.6-terra"),
    ("gpt-5-6-terra", "gpt-5.6-terra"),
    ("gpt5.6-luna", "gpt-5.6-luna"),
    ("gpt-5-6-luna", "gpt-5.6-luna"),
    ("gpt5.6", "gpt-5.6-sol"),
    ("gpt-5-6", "gpt-5.6-sol"),
    ("gpt-5.6", "gpt-5.6-sol"),
    ("gpt5.5", "gpt-5.5"),
    ("gpt-5-5", "gpt-5.5"),
    ("gpt5.4", "gpt-5.4"),
    ("gpt-5-4", "gpt-5.4"),
];

pub(super) fn map_client_model_to_upstream(model: &str) -> Result<String, String> {
    Ok(remap_model_name(model, MODEL_MAPPINGS).unwrap_or_else(|| model.to_string()))
}

pub(super) fn normalize_model_for_client(model: &str) -> String {
    remap_model_name(model, MODEL_MAPPINGS).unwrap_or_else(|| model.to_string())
}

pub(super) fn normalize_model_for_permissions(model: &str) -> String {
    let normalized = normalize_model_for_client(model);
    // Controls select catalog families. Resolve aliases and dated variants to
    // that family only for permission checks; preserve the wire model elsewhere.
    MODELS
        .iter()
        .find(|base| {
            normalized == **base
                || normalized
                    .strip_prefix(**base)
                    .is_some_and(is_dated_snapshot_suffix)
        })
        .map(|base| (*base).to_string())
        .unwrap_or(normalized)
}

fn is_dated_snapshot_suffix(suffix: &str) -> bool {
    let bytes = suffix.as_bytes();
    bytes.len() == 11
        && bytes[0] == b'-'
        && bytes[5] == b'-'
        && bytes[8] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| matches!(index, 0 | 5 | 8) || byte.is_ascii_digit())
}

fn remap_model_name(model: &str, mappings: &[(&str, &str)]) -> Option<String> {
    for (from, to) in mappings {
        if model == *from {
            return Some((*to).to_string());
        }
        if let Some(rest) = model.strip_prefix(from) {
            if rest.starts_with('-') {
                return Some(format!("{to}{rest}"));
            }
        }
    }

    None
}

pub(super) fn is_responses_lite_model(model: &str) -> bool {
    [
        "gpt-6-astra",
        "gpt-6-sol",
        "gpt-6-luna",
        "gpt-5.6-sol",
        "gpt-5.6-terra",
        "gpt-5.6-luna",
    ]
    .iter()
    .any(|prefix| model == *prefix || model.starts_with(&format!("{prefix}-")))
}

pub(super) fn upstream_codex_client_identity(
    headers: &HeaderMap,
    uses_responses_lite: bool,
) -> (&str, &str) {
    if uses_responses_lite {
        return (CODEX_CLIENT_VERSION, CODEX_USER_AGENT);
    }

    let version = headers
        .get("version")
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(CODEX_CLIENT_VERSION);
    let user_agent = headers
        .get("user-agent")
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(CODEX_USER_AGENT);
    (version, user_agent)
}

pub(super) fn is_astra_model(model: &str) -> bool {
    model == "gpt-6-astra" || model.starts_with("gpt-6-astra-")
}

pub(super) fn is_gpt6_sol_or_luna_model(model: &str) -> bool {
    ["gpt-6-sol", "gpt-6-luna"]
        .iter()
        .any(|prefix| model == *prefix || model.starts_with(&format!("{prefix}-")))
}

pub(super) fn validate_reasoning(model: &str, effort: Option<&str>) -> Result<(), String> {
    let normalized = effort.map(|value| value.trim().to_ascii_lowercase());
    let effort = normalized.as_deref();
    if is_astra_model(model) && matches!(effort, Some("none" | "minimal")) {
        return Err(
            "GPT-6 Astra 支持 low、medium、high、xhigh、max 推理强度；ultra 映射为 max".to_string(),
        );
    }
    if is_gpt6_sol_or_luna_model(model) && matches!(effort, Some("none" | "minimal")) {
        return Err(
            "GPT-6 Sol/Luna 的 Codex 模型目录支持 low、medium、high、xhigh、max 推理强度；ultra 映射为 max".to_string(),
        );
    }
    if is_responses_lite_model(model) && effort == Some("minimal") {
        return Err(
            "此模型不支持 minimal 推理强度；请使用 none、low、medium、high、xhigh 或 max"
                .to_string(),
        );
    }
    Ok(())
}
