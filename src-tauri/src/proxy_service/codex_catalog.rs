//! Native Codex metadata, separate from the lightweight OpenAI /v1/models list.
use super::{api_proxy_visible_models_for_key, ApiProxyKey, AppSettings};
use serde_json::{json, Value};
use std::sync::OnceLock;

pub(super) fn catalog_for_key(settings: &AppSettings, key: &ApiProxyKey) -> Value {
    static CATALOG: OnceLock<Value> = OnceLock::new();
    let catalog = CATALOG.get_or_init(|| {
        serde_json::from_str(include_str!("catalog/codex-models.json"))
            .expect("bundled Codex catalog must be valid JSON")
    });
    let visible = api_proxy_visible_models_for_key(settings, key);
    let mut models: Vec<_> = catalog["models"]
        .as_array()
        .expect("bundled Codex catalog must contain models")
        .iter()
        .filter(|model| {
            key.enabled
                && model["slug"]
                    .as_str()
                    .is_some_and(|slug| visible.contains(&slug))
        })
        .cloned()
        .collect();
    if models.is_empty() {
        // Codex 0.159 treats an empty explicit catalog as absent and exposes
        // bundled models again. Keep a non-selectable entry to prevent that
        // fallback when every text model (or the bound key) is disabled.
        let mut hidden = catalog["models"][0].clone();
        hidden["visibility"] = json!("hide");
        hidden["supported_in_api"] = json!(false);
        models.push(hidden);
    }
    json!({ "models": models })
}

#[cfg(test)]
#[path = "codex_catalog_tests.rs"]
mod tests;
