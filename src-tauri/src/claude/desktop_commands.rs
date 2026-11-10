use super::{desktop, store, summary, ClaudeState};
use serde_json::{json, Value};
use tauri::{AppHandle, State};

#[tauri::command]
pub async fn claude_import_desktop_account(
    app: AppHandle,
    state: State<'_, ClaudeState>,
    label: Option<String>,
) -> Result<Value, String> {
    let snapshot = tauri::async_runtime::spawn_blocking(|| desktop::capture(&desktop::data_dir()?))
        .await
        .map_err(|e| e.to_string())??;
    let _guard = state.store_lock.lock().await;
    let mut data = store::read(&app)?;
    let current_id = if let Some(account) = data.accounts.iter_mut().find(|a| {
        a.desktop.as_ref().is_some_and(|s| {
            s.account_uuid == snapshot.account_uuid && s.org_uuid == snapshot.org_uuid
        }) || (a.credentials["accountUuid"].as_str() == Some(snapshot.account_uuid.as_str())
            && a.credentials["orgUuid"].as_str() == Some(snapshot.org_uuid.as_str()))
    }) {
        account.desktop = Some(snapshot);
        if account.sync.needs_auth {
            account.credentials = json!({});
        }
        account.sync = Default::default();
        if let Some(label) = label.filter(|s| !s.trim().is_empty()) {
            account.label = label;
        }
        account.id.clone()
    } else {
        let id = uuid::Uuid::new_v4().to_string();
        data.accounts.push(store::Account {
            sync: Default::default(),
            id: id.clone(),
            label: label.filter(|s| !s.trim().is_empty()).unwrap_or_else(|| {
                format!(
                    "Claude Desktop {}",
                    snapshot.account_uuid.chars().take(8).collect::<String>()
                )
            }),
            email: None,
            credentials: json!({}),
            desktop: Some(snapshot),
            api_key: None,
            api_base_url: None,
            model: None,
            usage: None,
        });
        id
    };
    data.desktop_current_id = Some(current_id.clone());
    store::save(&app, &data)?;
    drop(_guard);
    let _ = super::sync::run(&app, &state, &current_id, true, true).await;
    let _guard = state.store_lock.lock().await;
    Ok(summary(&store::read(&app)?))
}

#[tauri::command]
pub async fn claude_switch_desktop_account(
    app: AppHandle,
    state: State<'_, ClaudeState>,
    account_id: String,
) -> Result<Value, String> {
    let _account_guard = state.lock_account(&account_id).await;
    let _guard = state.store_lock.lock().await;
    let mut data = store::read(&app)?;
    let target = data
        .accounts
        .iter()
        .find(|a| a.id == account_id)
        .and_then(|a| a.desktop.clone())
        .ok_or("请先从 Claude 桌面 App 导入此账号的登录")?;
    let backup = crate::app_paths::app_data_dir(&app)?
        .join("claude-desktop-backups")
        .join(uuid::Uuid::new_v4().to_string());
    let source = tauri::async_runtime::spawn_blocking(move || {
        desktop::switch(&desktop::data_dir()?, &backup, &target)
    })
    .await
    .map_err(|e| e.to_string())??;
    if let Some(previous) = data.accounts.iter_mut().find(|a| {
        a.desktop
            .as_ref()
            .is_some_and(|s| s.account_uuid == source.account_uuid && s.org_uuid == source.org_uuid)
    }) {
        previous.desktop = Some(source);
    }
    data.desktop_current_id = Some(account_id);
    store::save(&app, &data)?;
    Ok(summary(&data))
}
