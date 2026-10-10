//! Account warm-up eligibility, cooldown and observed activation.
use super::*;
use crate::models::AppSettings;
use chrono::Timelike;

/// 按操作系统当前本地时间判断活跃时段；支持跨午夜，结束时刻不再发起自动请求。
fn auto_warmup_allowed(settings: &AppSettings, minute: u16) -> bool {
    if !settings.auto_account_warmup_enabled {
        return false;
    }
    if !settings.auto_account_warmup_schedule_enabled {
        return true;
    }
    let start = settings.auto_account_warmup_start_minute;
    let end = settings.auto_account_warmup_end_minute;
    if start >= 1440 || end >= 1440 || start == end || minute >= 1440 {
        return false;
    }
    if start < end {
        (start..end).contains(&minute)
    } else {
        minute >= start || minute < end
    }
}

fn local_minute_now() -> u16 {
    // chrono 的系统时区读取支持多线程和夏令时，不使用 UTC 作为静默回退。
    let now = chrono::Local::now();
    (now.hour() * 60 + now.minute()) as u16
}

pub(crate) async fn warmup_account_internal(
    app: &AppHandle,
    state: &AppState,
    id: &str,
) -> Result<AccountWarmupResult, String> {
    let _warmup_guard = state.account_warmup_lock.lock().await;
    // Manual activation first refreshes cached window state, preventing a paid
    // request when another client has already started the 5h window.
    let _ = refresh_all_usage_coordinated(app, state, false, "warmup-preflight").await?;
    let mut status = attempt_account_warmup(app, state, id, false).await?;
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
    let settings = {
        let _store_guard = state.store_lock.lock().await;
        let store = load_store(app)?;
        if !auto_warmup_allowed(&store.settings, local_minute_now()) {
            return Ok(None);
        }
        store.settings
    };

    let mut activated = false;
    for account_id in &settings.auto_account_warmup_account_ids {
        // 每个账号发起请求前重新检查时间，避免批量预热跨过结束边界。
        if !auto_warmup_allowed(&settings, local_minute_now()) {
            break;
        }
        match attempt_account_warmup(app, state, account_id, true).await {
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
    automatic: bool,
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

    if automatic {
        // 授权刷新可能跨过结束边界，发送前再次读取最新开关、账号选择和本地时间。
        let _guard = state.store_lock.lock().await;
        let settings = load_store(app)?.settings;
        if !auto_warmup_allowed(&settings, local_minute_now())
            || !settings
                .auto_account_warmup_account_ids
                .iter()
                .any(|selected| selected == id)
        {
            return Err("自动预热已停用或离开允许时段，未发送请求".to_string());
        }
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
    fn automatic_warmup_respects_day_and_overnight_boundaries() {
        let mut settings = AppSettings {
            auto_account_warmup_enabled: true,
            auto_account_warmup_schedule_enabled: true,
            ..AppSettings::default()
        };
        assert!(!auto_warmup_allowed(&settings, 359));
        assert!(auto_warmup_allowed(&settings, 360));
        assert!(auto_warmup_allowed(&settings, 1319));
        assert!(!auto_warmup_allowed(&settings, 1320));
        settings.auto_account_warmup_start_minute = 1320;
        settings.auto_account_warmup_end_minute = 360;
        for minute in [1320, 1439, 0, 359] {
            assert!(auto_warmup_allowed(&settings, minute));
        }
        assert!(!auto_warmup_allowed(&settings, 360));
        settings.auto_account_warmup_end_minute = 1320;
        assert!(!auto_warmup_allowed(&settings, 1320));
        settings.auto_account_warmup_schedule_enabled = false;
        assert!(auto_warmup_allowed(&settings, 1320));
        settings.auto_account_warmup_enabled = false;
        assert!(!auto_warmup_allowed(&settings, 1320));
    }

    #[test]
    fn older_settings_preserve_unrestricted_warmup() {
        let settings: AppSettings =
            serde_json::from_str(r#"{"autoAccountWarmupEnabled":true}"#).unwrap();
        assert!(!settings.auto_account_warmup_schedule_enabled);
        assert_eq!(settings.auto_account_warmup_start_minute, 360);
        assert!(auto_warmup_allowed(&settings, 0));
    }
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
