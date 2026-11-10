use super::{store, sync, ClaudeState};
use tauri::{AppHandle, Manager};

pub fn start_maintenance(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut timer = tokio::time::interval(std::time::Duration::from_secs(60));
        timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            timer.tick().await;
            let state = app.state::<ClaudeState>();
            let accounts = {
                let _guard = state.store_lock.lock().await;
                store::read(&app).map(|s| s.accounts).unwrap_or_default()
            };
            let now = crate::utils::now_unix_seconds();
            for account in accounts {
                if account.api_key.is_none()
                    && !account.sync.needs_auth
                    && account.sync.retry_at <= now
                    && (sync::needs_refresh(&account, now)
                        || account.credentials["accessToken"].is_null())
                {
                    let _ = sync::run(&app, &state, &account.id, false, false).await;
                }
            }
        }
    });
}
