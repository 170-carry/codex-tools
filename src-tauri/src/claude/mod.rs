mod desktop;
mod maintenance;
mod remote;
mod sync;
pub use maintenance::start_maintenance;
mod desktop_commands;
pub use desktop_commands::*;
mod lifecycle;
mod local;
mod login_commands;
mod oauth;
mod store;
pub use lifecycle::*;
pub use login_commands::*;

use serde_json::{json, Value};
use tauri::{AppHandle, State};
use tokio::sync::Mutex;

pub struct ClaudeState {
    pending: Mutex<Option<oauth::PendingLogin>>,
    store_lock: Mutex<()>,
    client: reqwest::Client,
    account_locks: Mutex<std::collections::HashMap<String, std::sync::Arc<Mutex<()>>>>,
}
impl Default for ClaudeState {
    fn default() -> Self {
        Self {
            pending: Mutex::new(None),
            account_locks: Mutex::new(Default::default()),
            store_lock: Mutex::new(()),
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("HTTP client"),
        }
    }
}

fn summary(store: &store::Store) -> Value {
    json!({"accounts":store.accounts.iter().map(|a| json!({
        "id":a.id, "label":a.label, "email":a.email, "isCurrent":if a.desktop.is_some() { store.desktop_current_id.as_ref() == Some(&a.id) } else { store.current_id.as_ref() == Some(&a.id) },
        "kind":if a.desktop.is_some() {"desktop"} else if a.api_key.is_some() {"api"} else {"oauth"}, "usage":a.usage,
        "expiresAt":a.credentials.get("expiresAt"), "plan":a.credentials.get("subscriptionType"),
        "canRefresh":a.credentials["refreshToken"].as_str().is_some_and(|s| !s.is_empty()),
        "baseUrl":a.api_base_url, "model":a.model, "sync":a.sync,
    })).collect::<Vec<_>>()})
}

#[tauri::command]
pub async fn claude_list_accounts(
    app: AppHandle,
    state: State<'_, ClaudeState>,
) -> Result<Value, String> {
    let _guard = state.store_lock.lock().await;
    Ok(summary(&store::read(&app)?))
}

#[tauri::command]
pub async fn claude_import_accounts(
    app: AppHandle,
    state: State<'_, ClaudeState>,
    contents: Vec<String>,
    label: Option<String>,
    current: bool,
) -> Result<Value, String> {
    let values = if current {
        vec![local::read_credentials()?]
    } else {
        contents
            .iter()
            .map(|s| serde_json::from_str(s).map_err(|e| e.to_string()))
            .collect::<Result<Vec<Value>, _>>()?
    };
    let accounts = values
        .into_iter()
        .map(|v| store::import(v, label.clone()))
        .collect::<Result<Vec<_>, _>>()?;
    let _guard = state.store_lock.lock().await;
    let mut data = store::read(&app)?;
    for account in accounts {
        let token = account.credentials["accessToken"].clone();
        store::upsert(&mut data, account, None)?;
        if current {
            data.current_id = data
                .accounts
                .iter()
                .find(|a| a.credentials["accessToken"] == token)
                .map(|a| a.id.clone());
        }
    }
    store::save(&app, &data)?;
    Ok(summary(&data))
}

#[tauri::command]
pub async fn claude_save_api_account(
    app: AppHandle,
    state: State<'_, ClaudeState>,
    label: String,
    base_url: String,
    api_key: String,
    model: String,
) -> Result<Value, String> {
    let base_url = base_url.trim().trim_end_matches('/').to_string();
    let response = state
        .client
        .get(format!("{base_url}/v1/models"))
        .header("x-api-key", &api_key)
        .header("anthropic-version", "2023-06-01")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(format!("Claude API 验证失败（HTTP {}）", response.status()));
    }
    let account = store::Account {
        sync: Default::default(),
        id: uuid::Uuid::new_v4().to_string(),
        label,
        email: None,
        credentials: json!({}),
        api_base_url: Some(base_url),
        api_key: Some(api_key),
        desktop: None,
        model: if model.is_empty() { None } else { Some(model) },
        usage: None,
    };
    let _guard = state.store_lock.lock().await;
    let mut data = store::read(&app)?;
    data.accounts.push(account);
    store::save(&app, &data)?;
    Ok(summary(&data))
}

#[tauri::command]
pub async fn claude_delete_account(
    app: AppHandle,
    state: State<'_, ClaudeState>,
    account_id: String,
) -> Result<Value, String> {
    let _account_guard = state.lock_account(&account_id).await;
    let _guard = state.store_lock.lock().await;
    let mut data = store::read(&app)?;
    data.accounts.retain(|a| a.id != account_id);
    if data.desktop_current_id.as_ref() == Some(&account_id) {
        data.desktop_current_id = None;
    }
    if data.current_id.as_ref() == Some(&account_id) {
        data.current_id = None;
    }
    store::save(&app, &data)?;
    Ok(summary(&data))
}
