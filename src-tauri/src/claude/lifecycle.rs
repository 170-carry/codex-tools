use super::{local, store, summary, sync, ClaudeState};
use serde_json::Value;
use tauri::{AppHandle, State};

#[tauri::command]
pub async fn claude_switch_account(
    app: AppHandle,
    state: State<'_, ClaudeState>,
    account_id: String,
) -> Result<Value, String> {
    sync::run(&app, &state, &account_id, false, true).await?;
    let _account_guard = state.lock_account(&account_id).await;
    let _guard = state.store_lock.lock().await;
    let mut data = store::read(&app)?;
    let account = data
        .accounts
        .iter()
        .find(|a| a.id == account_id)
        .ok_or("Claude 账号不存在")?;
    local::activate(account)?;
    data.current_id = Some(account_id);
    store::save(&app, &data)?;
    Ok(summary(&data))
}

#[tauri::command]
pub async fn claude_refresh_usage(
    app: AppHandle,
    state: State<'_, ClaudeState>,
    account_id: String,
) -> Result<Value, String> {
    // Per-account failures are returned in the account summary without dropping cached usage.
    let _ = sync::run(&app, &state, &account_id, true, true).await;
    let _guard = state.store_lock.lock().await;
    Ok(summary(&store::read(&app)?))
}

#[tauri::command]
pub async fn claude_refresh_accounts(
    app: AppHandle,
    state: State<'_, ClaudeState>,
    force: bool,
) -> Result<Value, String> {
    let ids = {
        let _guard = state.store_lock.lock().await;
        store::read(&app)?
            .accounts
            .into_iter()
            .filter(|a| a.api_key.is_none())
            .map(|a| a.id)
            .collect::<Vec<_>>()
    };
    for id in ids {
        let _ = sync::run(&app, &state, &id, true, force).await;
    }
    let _guard = state.store_lock.lock().await;
    Ok(summary(&store::read(&app)?))
}
