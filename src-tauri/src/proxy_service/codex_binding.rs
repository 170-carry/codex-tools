//! Sync only on binding, startup, or policy changes, never on status polling.
use super::*;

pub(crate) async fn bind_codex_to_api_proxy_internal(
    app: &AppHandle,
    state: &AppState,
) -> Result<ApiProxyStatus, String> {
    let status = get_api_proxy_status_internal(app, state).await?;
    let base_url = status
        .base_url
        .as_deref()
        .ok_or_else(|| "请先启动 API 反代，再绑定 Codex App/CLI。".to_string())?;
    let api_key = status
        .api_key
        .as_deref()
        .ok_or_else(|| "请先生成 API 反代 API Key，再绑定 Codex App/CLI。".to_string())?;

    let storage = app_proxy_storage_context(app, state)?;
    {
        let _guard = storage.store_lock.lock().await;
        let settings =
            load_store_from_path(&account_store_path_from_data_dir(&storage.data_dir))?.settings;
        let key = load_api_proxy_key_store_from_path(&api_proxy_keys_path(&storage)?)?
            .keys
            .into_iter()
            .find(|key| key.enabled && key.key == api_key)
            .ok_or_else(|| "当前反代 API Key 已失效，请重新生成后绑定。".to_string())?;
        let catalog = codex_catalog::catalog_for_key(&settings, &key);
        profile_files::bind_codex_to_api_proxy(base_url, api_key, &catalog)?;
    }
    get_api_proxy_status_internal(app, state).await
}

pub(crate) async fn sync_bound_codex_catalog(storage: &ProxyStorageContext) -> Result<(), String> {
    // Account switching and policy writes use this same lock. Read and write
    // the binding without yielding so a stale sync cannot undo a switch.
    let _guard = storage.store_lock.lock().await;
    let Some(api_key) = profile_files::bound_proxy_key()? else {
        return Ok(());
    };
    let settings =
        load_store_from_path(&account_store_path_from_data_dir(&storage.data_dir))?.settings;
    let keys = load_api_proxy_key_store_from_path(&api_proxy_keys_path(storage)?)?.keys;
    let key = keys
        .into_iter()
        .find(|key| key.key == api_key)
        .unwrap_or_else(|| ApiProxyKey {
            enabled: false,
            ..ApiProxyKey::default()
        });
    let catalog = codex_catalog::catalog_for_key(&settings, &key);
    profile_files::sync_proxy_catalog(&catalog)
}

#[cfg(feature = "desktop")]
pub(crate) async fn sync_bound_codex_catalog_internal(
    app: &AppHandle,
    state: &AppState,
) -> Result<(), String> {
    sync_bound_codex_catalog(&app_proxy_storage_context(app, state)?).await
}
