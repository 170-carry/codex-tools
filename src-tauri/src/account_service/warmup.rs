//! Account warm-up eligibility, cooldown and observed activation.
use super::*;

pub(crate) async fn warmup_account_internal(
    app: &AppHandle,
    state: &AppState,
    id: &str,
) -> Result<AccountWarmupResult, String> {
    let _warmup_guard = state.account_warmup_lock.lock().await;
    // Manual activation first refreshes cached window state, preventing a paid
    // request when another client has already started the 5h window.
    let _ = refresh_all_usage_coordinated(app, state, false, "warmup-preflight").await?;
    let mut status = attempt_account_warmup(app, state, id).await?;
    let accounts = if status == AccountWarmupStatus::RequestSent {
        tokio::time::sleep(Duration::from_millis(WARMUP_FOLLOW_UP_DELAY_MS)).await;
        refresh_all_usage_coordinated(app, state, false, "warmup-follow-up").await?
    } else {
        list_accounts_internal(app, state).await?
    };
    if status == AccountWarmupStatus::RequestSent
        && accounts
            .iter()
            .find(|account| account.id == id)
            .is_some_and(|account| {
                account.usage_error.is_none()
                    && account
                        .usage
                        .as_ref()
                        .and_then(|u| u.five_hour.as_ref())
                        .is_some_and(|window| window_is_active(window, now_unix_seconds()))
            })
    {
        status = AccountWarmupStatus::Activated;
    }
    Ok(AccountWarmupResult {
        id: id.to_string(),
        status,
        accounts,
    })
}

pub(crate) async fn run_auto_account_warmups_internal(
    app: &AppHandle,
    state: &AppState,
) -> Result<Option<Vec<AccountSummary>>, String> {
    let _warmup_guard = state.account_warmup_lock.lock().await;
    let account_ids = {
        let _store_guard = state.store_lock.lock().await;
        let store = load_store(app)?;
        if !store.settings.auto_account_warmup_enabled {
            return Ok(None);
        }
        store.settings.auto_account_warmup_account_ids.clone()
    };

    let mut activated = false;
    for account_id in account_ids {
        match attempt_account_warmup(app, state, &account_id).await {
            Ok(AccountWarmupStatus::RequestSent) => activated = true,
            Ok(status) => log::info!(
                "ACCOUNT_WARMUP trigger=auto account_id={} action=skip status={:?}",
                account_id,
                status
            ),
            Err(error) => log::warn!(
                "ACCOUNT_WARMUP trigger=auto account_id={} action=failed error={}",
                account_id,
                error
            ),
        }
    }

    if !activated {
        return Ok(None);
    }
    tokio::time::sleep(Duration::from_millis(WARMUP_FOLLOW_UP_DELAY_MS)).await;
    refresh_all_usage_coordinated(app, state, false, "warmup-follow-up")
        .await
        .map(Some)
}

async fn attempt_account_warmup(
    app: &AppHandle,
    state: &AppState,
    id: &str,
) -> Result<AccountWarmupStatus, String> {
    let now = now_unix_seconds();
    let account = {
        let _store_guard = state.store_lock.lock().await;
        let store = load_store(app)?;
        let account = store
            .accounts
            .iter()
            .find(|account| account.id == id)
            .cloned()
            .ok_or_else(|| "找不到要预热的账号".to_string())?;
        if matches!(account.source_kind, AccountSourceKind::Relay) {
            return Err("API 中转站账号不支持 ChatGPT 5h 窗口预热".to_string());
        }
        if account.usage.is_none() {
            return Err("无法确认预热前额度状态，请先成功刷新用量".to_string());
        }
        if let Some(error) = &account.usage_error {
            return Err(format!("无法确认预热前额度状态: {error}"));
        }
        if account_has_active_five_hour_window(&account, now) {
            return Ok(AccountWarmupStatus::AlreadyActive);
        }
        if account_weekly_quota_exhausted(&account) {
            return Ok(AccountWarmupStatus::Exhausted);
        }
        if store
            .settings
            .account_warmup_attempts
            .get(id)
            .is_some_and(|attempt| warmup_attempt_is_recent(attempt, now))
        {
            return Ok(AccountWarmupStatus::RecentlyAttempted);
        }
        if account.auth_refresh_blocked {
            return Err(account
                .auth_refresh_error
                .clone()
                .unwrap_or_else(|| AUTH_EXPIRED_NOTICE.to_string()));
        }
        account
    };

    let account_key = account.account_key();
    let mut auth_json =
        refresh_latest_auth_json_if_newer(app, state, &account_key, &account.auth_json).await;
    if auth_tokens_need_refresh(&auth_json) {
        auth_json = refresh_account_auth_with_operation_guard(app, state, &account_key, &auth_json)
            .await
            .map_err(|failure| {
                failure
                    .refresh_error
                    .or(failure.auth_refresh_error)
                    .unwrap_or_else(|| "账号预热前刷新授权失败".to_string())
            })?
            .auth_json;
    }

    persist_warmup_attempt(
        app,
        state,
        id,
        now,
        false,
        Some("请求进行中".to_string()),
        Some(&auth_json),
    )
    .await?;
    let request_result = proxy_service::send_minimal_account_warmup_request(&auth_json).await;
    match request_result {
        Ok(()) => {
            persist_warmup_attempt(app, state, id, now, true, None, Some(&auth_json)).await?;
            log::info!(
                "ACCOUNT_WARMUP account_id={} action=request-completed prompt=hello",
                id
            );
            Ok(AccountWarmupStatus::RequestSent)
        }
        Err(error) => {
            persist_warmup_attempt(
                app,
                state,
                id,
                now,
                false,
                Some(error.clone()),
                Some(&auth_json),
            )
            .await?;
            Err(error)
        }
    }
}

async fn persist_warmup_attempt(
    app: &AppHandle,
    state: &AppState,
    id: &str,
    attempted_at: i64,
    succeeded: bool,
    error: Option<String>,
    auth_json: Option<&serde_json::Value>,
) -> Result<(), String> {
    let _store_guard = state.store_lock.lock().await;
    let mut store = load_store(app)?;
    let Some(account) = store.accounts.iter_mut().find(|account| account.id == id) else {
        store.settings.account_warmup_attempts.remove(id);
        save_store(app, &store)?;
        return Err("预热期间账号已被删除".to_string());
    };
    if let Some(auth_json) =
        auth_json.filter(|candidate| has_newer_auth_refresh_snapshot(candidate, &account.auth_json))
    {
        account.auth_json = auth_json.clone();
        account.updated_at = attempted_at;
        account.auth_refresh_blocked = false;
        account.auth_refresh_error = None;
        let store_path = account_store_path_from_data_dir(&app_paths::app_data_dir(app)?);
        profile_files::sync_account_profile_in_store_path(&store_path, account)?;
    }
    store.settings.account_warmup_attempts.insert(
        id.to_string(),
        AccountWarmupAttempt {
            attempted_at,
            succeeded,
            error,
        },
    );
    save_store(app, &store)
}

pub(super) fn account_has_active_five_hour_window(account: &StoredAccount, now: i64) -> bool {
    account
        .usage
        .as_ref()
        .and_then(|usage| usage.five_hour.as_ref())
        .is_some_and(|window| window_is_active(window, now))
}

pub(super) fn account_weekly_quota_exhausted(account: &StoredAccount) -> bool {
    account
        .usage
        .as_ref()
        .and_then(|usage| usage.one_week.as_ref())
        .is_some_and(|window| window.used_percent >= 100.0)
}

pub(super) fn warmup_attempt_is_recent(attempt: &AccountWarmupAttempt, now: i64) -> bool {
    let cooldown = if attempt.succeeded {
        WARMUP_SUCCESS_COOLDOWN_SECS
    } else {
        WARMUP_FAILURE_COOLDOWN_SECS
    };
    now.saturating_sub(attempt.attempted_at) < cooldown
}

fn window_is_active(window: &crate::models::UsageWindow, now: i64) -> bool {
    (FIVE_HOUR_WINDOW_MIN_SECS..=FIVE_HOUR_WINDOW_MAX_SECS).contains(&window.window_seconds)
        && window.used_percent.is_finite()
        && window.used_percent > 0.0
        && window.reset_at.is_some_and(|reset_at| reset_at > now)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unused_window_with_future_reset_does_not_suppress_warmup() {
        let mut window = crate::models::UsageWindow {
            used_percent: 0.0,
            window_seconds: 18000,
            reset_at: Some(200),
        };
        assert!(!window_is_active(&window, 100));
        window.used_percent = 1.0;
        assert!(window_is_active(&window, 100));
        assert!(!window_is_active(&window, 201));
    }
}
