use super::{oauth, remote, store, ClaudeState};
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatus {
    pub last_usage_at: Option<i64>,
    pub last_refresh_at: Option<i64>,
    pub error: Option<String>,
    pub needs_auth: bool,
    pub retry_at: i64,
    pub failures: u32,
}

pub fn needs_refresh(account: &store::Account, now: i64) -> bool {
    account.credentials["expiresAt"]
        .as_i64()
        .is_some_and(|t| t <= (now + 180) * 1000)
}

pub async fn save_account(
    app: &AppHandle,
    state: &ClaudeState,
    account: &store::Account,
) -> Result<(), String> {
    let _guard = state.store_lock.lock().await;
    let mut data = store::read(app)?;
    if let Some(existing) = data.accounts.iter_mut().find(|a| a.id == account.id) {
        existing.credentials = account.credentials.clone();
        existing.email = account.email.clone();
        existing.usage = account.usage.clone();
        existing.sync = account.sync.clone();
        if existing.label == "Claude" || existing.label.starts_with("Claude Desktop ") {
            existing.label = account.label.clone();
        }
        store::save(app, &data)?;
    }
    Ok(())
}

impl ClaudeState {
    pub async fn lock_account(&self, id: &str) -> tokio::sync::OwnedMutexGuard<()> {
        let lock = self
            .account_locks
            .lock()
            .await
            .entry(id.to_string())
            .or_default()
            .clone();
        lock.lock_owned().await
    }
}

async fn renew(
    app: &AppHandle,
    state: &ClaudeState,
    account: &mut store::Account,
) -> Result<(), String> {
    let refresh = account.credentials["refreshToken"]
        .as_str()
        .ok_or("Claude 令牌无法自动续期，请重新授权")?;
    let previous = account.credentials.clone();
    let response = oauth::refresh(&state.client, refresh).await?;
    account.credentials = oauth::credentials(&response, Some(&account.credentials))?;
    account.sync.last_refresh_at = Some(crate::utils::now_unix_seconds());
    // Store a rotated refresh token immediately, even if the subsequent usage call fails.
    save_account(app, state, account).await?;
    let is_current = {
        let _guard = state.store_lock.lock().await;
        store::read(app)?.current_id.as_deref() == Some(account.id.as_str())
    };
    if is_current {
        super::local::replace_if_matches(&previous, &account.credentials)?;
    }
    Ok(())
}

pub async fn update(
    app: &AppHandle,
    state: &ClaudeState,
    account: &mut store::Account,
    usage: bool,
) -> Result<(), String> {
    if account.credentials["accessToken"].as_str().is_none() {
        let desktop = account
            .desktop
            .as_ref()
            .ok_or("缺少 Claude 凭据，请重新授权")?;
        let response = remote::authorize_desktop(&state.client, desktop).await?;
        account.credentials = oauth::credentials(&response, None)?;
        save_account(app, state, account).await?;
    }
    if needs_refresh(account, crate::utils::now_unix_seconds()) {
        renew(app, state, account).await?;
    }
    if usage {
        let token = account.credentials["accessToken"].as_str().unwrap();
        let result = remote::get(&state.client, token, "usage").await;
        let value = match result {
            Err(e) if e.contains("HTTP 401") => {
                renew(app, state, account).await?;
                remote::get(
                    &state.client,
                    account.credentials["accessToken"].as_str().unwrap(),
                    "usage",
                )
                .await?
            }
            result => result?,
        };
        let profile = remote::get(
            &state.client,
            account.credentials["accessToken"].as_str().unwrap(),
            "profile",
        )
        .await?;
        remote::apply_profile(account, &profile)?;
        account.usage = Some(value);
        account.sync.last_usage_at = Some(crate::utils::now_unix_seconds());
    }
    Ok(())
}

pub fn record_result(account: &mut store::Account, result: &Result<(), String>, now: i64) {
    match result {
        Ok(()) => {
            account.sync.error = None;
            account.sync.needs_auth = false;
            account.sync.failures = 0;
            account.sync.retry_at = 0;
        }
        Err(error) => {
            account.sync.failures += 1;
            account.sync.needs_auth = error.contains("HTTP 401")
                || error.contains("HTTP 403")
                || error.contains("invalid_grant")
                || error.contains("重新授权")
                || error.contains("不一致");
            account.sync.error = Some(error.clone());
            account.sync.retry_at =
                now + (60_i64 * 2_i64.pow(account.sync.failures.min(6))).min(3600);
        }
    }
}

pub async fn run(
    app: &AppHandle,
    state: &ClaudeState,
    id: &str,
    usage: bool,
    force: bool,
) -> Result<(), String> {
    let _account_guard = state.lock_account(id).await;
    let mut account = {
        let _guard = state.store_lock.lock().await;
        store::read(app)?
            .accounts
            .into_iter()
            .find(|a| a.id == id)
            .ok_or("Claude 账号不存在")?
    };
    if account.api_key.is_some() {
        return Ok(());
    }
    let now = crate::utils::now_unix_seconds();
    if !force && (account.sync.needs_auth || account.sync.retry_at > now) {
        return Ok(());
    }
    let usage = usage && (force || account.sync.last_usage_at.is_none_or(|t| now - t >= 300));
    let is_current = {
        let _guard = state.store_lock.lock().await;
        store::read(app)?.current_id.as_deref() == Some(id)
    };
    if is_current {
        if let Ok(local) = super::local::read_credentials() {
            let current = &local["claudeAiOauth"];
            if current["accountUuid"].is_string()
                && current["accountUuid"] == account.credentials["accountUuid"]
                && current["orgUuid"] == account.credentials["orgUuid"]
                && current["expiresAt"].as_i64() > account.credentials["expiresAt"].as_i64()
            {
                account.credentials = current.clone();
            }
        }
    }
    let result = update(app, state, &mut account, usage).await;
    record_result(&mut account, &result, now);
    save_account(app, state, &account).await?;
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn refresh_window_and_backoff() {
        let mut a = store::Account {
            credentials: serde_json::json!({"expiresAt":1180000}),
            ..Default::default()
        };
        assert!(needs_refresh(&a, 1000));
        assert!(!needs_refresh(&a, 999));
        record_result(&mut a, &Err("Claude 请求失败（HTTP 429）".into()), 1000);
        assert!(!a.sync.needs_auth);
        assert_eq!(a.sync.retry_at, 1120);
        record_result(&mut a, &Err("invalid_grant".into()), 1000);
        assert!(a.sync.needs_auth);
        record_result(&mut a, &Ok(()), 1000);
        assert!(!a.sync.needs_auth);
        assert_eq!(a.sync.retry_at, 0);
    }
}
