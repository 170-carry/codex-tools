use super::{local, oauth, store, summary, ClaudeState};
use serde_json::Value;
use tauri::{AppHandle, State};

#[tauri::command]
pub async fn claude_prepare_oauth(state: State<'_, ClaudeState>) -> Result<String, String> {
    let (pending, url) = oauth::prepare();
    *state.pending.lock().await = Some(pending);
    Ok(url)
}

#[tauri::command]
pub async fn claude_cancel_oauth(state: State<'_, ClaudeState>) -> Result<(), String> {
    *state.pending.lock().await = None;
    Ok(())
}

#[tauri::command]
pub async fn claude_complete_oauth(
    app: AppHandle,
    state: State<'_, ClaudeState>,
    code: String,
    label: Option<String>,
    account_id: Option<String>,
) -> Result<Value, String> {
    let mut pending = state.pending.lock().await;
    let login = pending.as_ref().ok_or("请先生成 Claude 授权链接")?;
    let response = oauth::complete(&state.client, login, &code).await?;
    let mut account = store::import(response, label)?;
    let profile = super::remote::get(
        &state.client,
        account.credentials["accessToken"].as_str().unwrap(),
        "profile",
    )
    .await?;
    super::remote::apply_profile(&mut account, &profile)?;
    let _account_guard = match &account_id {
        Some(id) => Some(state.lock_account(id).await),
        None => None,
    };
    let _guard = state.store_lock.lock().await;
    let mut data = store::read(&app)?;
    if let Some(old) = account_id
        .as_ref()
        .and_then(|id| data.accounts.iter().find(|a| &a.id == id))
    {
        account.desktop = old.desktop.clone();
        super::remote::apply_profile(&mut account, &profile)?;
    }
    store::upsert(&mut data, account, account_id.as_deref())?;
    if let Some(id) = account_id
        .as_ref()
        .filter(|id| Some(*id) == data.current_id.as_ref())
    {
        local::activate(data.accounts.iter().find(|a| &a.id == id).unwrap())?;
    }
    store::save(&app, &data)?;
    *pending = None;
    Ok(summary(&data))
}
