use std::time::Duration;

use tauri::AppHandle;
use tauri::Manager;

use crate::account_service;
use crate::state::AppState;

/// 应用启动后等待这么久再开始首次检查，避免与初始化流程竞争。
const STARTUP_DELAY_SECS: u64 = 30;
/// 每次检查之间的间隔。
const CHECK_INTERVAL_SECS: u64 = 60;

pub(crate) async fn run(app: AppHandle) {
    tokio::time::sleep(Duration::from_secs(STARTUP_DELAY_SECS)).await;

    let state = app.state::<AppState>();

    loop {
        account_service::daemon_refresh_next_expiring(&app, state.inner()).await;
        tokio::time::sleep(Duration::from_secs(CHECK_INTERVAL_SECS)).await;
    }
}
