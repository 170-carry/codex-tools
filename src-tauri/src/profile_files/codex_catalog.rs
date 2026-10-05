//! Own only the proxy's explicit catalog; never overwrite Codex's shared cache.
use super::*;

pub(super) const CATALOG_FILE_NAME: &str = "codex-tools-models.json";

pub(super) fn catalog_path(config_path: &Path) -> PathBuf {
    config_path.with_file_name(CATALOG_FILE_NAME)
}

pub(super) fn remove_managed_catalog(document: &mut DocumentMut) {
    let managed = document
        .get("model_catalog_json")
        .and_then(toml_edit::Item::as_str)
        .is_some_and(|path| {
            // Handle profiles copied between Windows and Unix too.
            path.rsplit(['/', '\\']).next() == Some(CATALOG_FILE_NAME)
        });
    if managed {
        document.remove("model_catalog_json");
    }
}

pub(super) fn write_catalog(config_path: &Path, catalog: &Value) -> Result<(), String> {
    let path = catalog_path(config_path);
    let bytes = serde_json::to_vec(catalog)
        .map_err(|error| format!("序列化 Codex 模型目录失败: {error}"))?;
    // Settings and key updates often leave the model selection unchanged.
    if fs::read(&path).ok().as_deref() != Some(bytes.as_slice()) {
        write_file_atomically(&path, &bytes)?;
    }
    Ok(())
}

pub(super) fn record_bound_base_url(metadata_path: &Path, base_url: &str) -> Result<(), String> {
    let raw = fs::read_to_string(metadata_path).map_err(|error| error.to_string())?;
    let mut metadata: CodexProxyBindingBackupMetadata =
        serde_json::from_str(&raw).map_err(|error| error.to_string())?;
    if metadata.bound_base_url != base_url {
        metadata.bound_base_url = base_url.to_string();
        let bytes = serde_json::to_vec_pretty(&metadata).map_err(|error| error.to_string())?;
        write_file_atomically(metadata_path, &bytes)?;
    }
    Ok(())
}

/// Return credentials only for an existing Tools-managed binding. A normal
/// account switch or a manually edited provider must not be taken over.
pub(crate) fn bound_proxy_key() -> Result<Option<String>, String> {
    bound_proxy_key_in_paths(
        &current_codex_config_path()?,
        &app_paths::codex_auth_path()?,
        &codex_proxy_backup_metadata_path()?,
    )
}

fn bound_proxy_key_in_paths(
    config_path: &Path,
    auth_path: &Path,
    metadata_path: &Path,
) -> Result<Option<String>, String> {
    let Some(metadata) = read_optional_text(metadata_path)? else {
        return Ok(None);
    };
    let metadata: CodexProxyBindingBackupMetadata = serde_json::from_str(&metadata)
        .map_err(|error| format!("Codex 反代绑定备份元数据无效: {error}"))?;
    let Some(config) = read_optional_text(config_path)? else {
        return Ok(None);
    };
    let document = config
        .parse::<DocumentMut>()
        .map_err(|error| error.to_string())?;
    if codex_config_openai_base_url(&config).as_deref() != Some(&metadata.bound_base_url)
        || document
            .get("model_provider")
            .and_then(toml_edit::Item::as_str)
            != Some("openai")
    {
        return Ok(None);
    }
    if document
        .get("model_catalog_json")
        .and_then(toml_edit::Item::as_str)
        .is_some_and(|path| Path::new(path) != catalog_path(config_path))
    {
        return Ok(None);
    }
    let Some(auth) = read_optional_text(auth_path)? else {
        return Ok(None);
    };
    let auth: Value = serde_json::from_str(&auth).map_err(|error| error.to_string())?;
    Ok(auth
        .get("OPENAI_API_KEY")
        .and_then(Value::as_str)
        .map(str::to_string))
}

pub(crate) fn sync_proxy_catalog(catalog: &Value) -> Result<(), String> {
    let path = current_codex_config_path()?;
    sync_proxy_catalog_at(&path, catalog)
}

fn sync_proxy_catalog_at(path: &Path, catalog: &Value) -> Result<(), String> {
    let config = read_optional_text(path)?.unwrap_or_default();
    let mut document = config
        .parse::<DocumentMut>()
        .map_err(|error| error.to_string())?;
    write_catalog(path, catalog)?;
    document["model_catalog_json"] = value(catalog_path(path).to_string_lossy().as_ref());
    let updated = document.to_string();
    if updated != config {
        write_file_atomically(path, updated.as_bytes())?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "codex_catalog_tests.rs"]
mod tests;
