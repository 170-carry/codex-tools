//! The public Codex catalog supplies native picker metadata and transport flags together.
use super::model_catalog::MODELS;
use serde::Deserialize;
use serde_json::Value;
use std::path::Path;
use std::sync::{Arc, OnceLock, RwLock};
use std::time::Duration;

const SOURCE: &str =
    "https://raw.githubusercontent.com/openai/codex/main/codex-rs/models-manager/models.json";
const CACHE_FILE: &str = "upstream-codex-models.json";

#[derive(Deserialize)]
struct CatalogFile {
    models: Vec<Value>,
}

pub(super) struct Catalog {
    pub names: Vec<String>,
    pub entries: Vec<Value>,
}

impl Catalog {
    fn from_files(bundled: &str, upstream: Option<&[u8]>) -> Result<Self, String> {
        let seed: CatalogFile = serde_json::from_str(bundled).expect("bundled Codex catalog");
        let mut entries = Vec::new();
        let mut names = Vec::new();
        if let Some(bytes) = upstream {
            let file: CatalogFile =
                serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
            for entry in file.models {
                if entry["visibility"] == "list" && entry["supported_in_api"] == true {
                    let slug = entry["slug"].as_str().ok_or("missing Codex model slug")?;
                    if !names.iter().any(|name| name == slug) {
                        names.push(slug.to_owned());
                        entries.push(entry);
                    }
                }
            }
            if entries.is_empty() {
                return Err("upstream Codex catalog has no visible API models".into());
            }
        }
        for model in MODELS {
            if !names.iter().any(|name| name == model) {
                names.push((*model).to_owned());
            }
        }
        for entry in seed.models {
            if !entries
                .iter()
                .any(|existing| existing["slug"] == entry["slug"])
            {
                entries.push(entry);
            }
        }
        for entry in &mut entries {
            if let Some(template) = entry["model_messages"]["instructions_template"].as_str() {
                entry["base_instructions"] = Value::String(template.to_owned());
            }
        }
        Ok(Self { names, entries })
    }

    pub fn metadata(&self, model: &str) -> Option<&Value> {
        self.entries
            .iter()
            .find(|entry| entry["slug"].as_str() == Some(model))
    }
}

const SEED: &str = include_str!("catalog/codex-models.json");

fn active() -> &'static RwLock<Arc<Catalog>> {
    static CATALOG: OnceLock<RwLock<Arc<Catalog>>> = OnceLock::new();
    CATALOG.get_or_init(|| RwLock::new(Arc::new(Catalog::from_files(SEED, None).unwrap())))
}

pub(super) fn snapshot() -> Arc<Catalog> {
    active().read().unwrap().clone()
}

pub(super) fn load_cache(data_dir: &Path) {
    static LOADED: OnceLock<()> = OnceLock::new();
    LOADED.get_or_init(|| {
        if let Some(catalog) = std::fs::read(data_dir.join(CACHE_FILE))
            .ok()
            .and_then(|bytes| Catalog::from_files(SEED, Some(&bytes)).ok())
        {
            *active().write().unwrap() = Arc::new(catalog);
        }
    });
}

pub(super) async fn refresh(data_dir: &Path) -> Result<(), String> {
    load_cache(data_dir);
    static ETAG: tokio::sync::Mutex<Option<String>> = tokio::sync::Mutex::const_new(None);
    let mut etag = ETAG.lock().await;
    let mut request = reqwest::Client::new()
        .get(SOURCE)
        .timeout(Duration::from_secs(10));
    if let Some(tag) = etag.as_ref() {
        request = request.header(reqwest::header::IF_NONE_MATCH, tag);
    }
    let response = request
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?;
    if response.status() == reqwest::StatusCode::NOT_MODIFIED {
        return Ok(());
    }
    let new_tag = response
        .headers()
        .get(reqwest::header::ETAG)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let bytes = response.bytes().await.map_err(|e| e.to_string())?;
    let catalog = Catalog::from_files(SEED, Some(&bytes))?;
    super::write_private_named_file_atomically(
        &data_dir.join(CACHE_FILE),
        &bytes,
        "Codex model catalog",
    )?;
    *active().write().unwrap() = Arc::new(catalog);
    *etag = new_tag;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn upstream_additions_keep_metadata_and_exclude_hidden_models() {
        let file = json!({"models":[
            {"slug":"future","visibility":"list","supported_in_api":true,"use_responses_lite":true,"model_messages":{"instructions_template":"fixture"}},
            {"slug":"private","visibility":"hide","supported_in_api":true}
        ]});
        let catalog = Catalog::from_files(SEED, Some(&serde_json::to_vec(&file).unwrap())).unwrap();
        assert_eq!(catalog.names[0], "future");
        assert_eq!(
            catalog.metadata("future").unwrap()["base_instructions"],
            "fixture"
        );
        assert!(catalog.names.iter().any(|name| name == "gpt-image-2"));
        assert!(catalog.metadata("private").is_none());
        assert!(Catalog::from_files(SEED, Some(b"{}")).is_err());
    }
}
