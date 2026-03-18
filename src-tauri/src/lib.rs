mod account_service;
mod auth;
mod cli;
mod cloudflared_service;
mod editor_apps;
mod i18n;
mod models;
mod opencode;
pub mod proxy_daemon;
mod proxy_service;
mod remote_service;
mod settings_service;
mod state;
mod store;
mod tray;
mod usage;
mod utils;

use std::process::Command;
use std::process::Stdio;
use std::thread;
use std::time::Duration;

use tauri::AppHandle;
use tauri::Manager;
use tauri::State;
use tauri::WindowEvent;

use models::AccountSummary;
use models::ApiProxyStatus;
use models::AppSettings;
use models::AppSettingsPatch;
use models::AuthJsonImportInput;
use models::CloudflaredStatus;
use models::CurrentAuthStatus;
use models::DeployRemoteProxyInput;
use models::EditorAppId;
use models::ImportAccountsResult;
use models::InstalledEditorApp;
use models::RemoteProxyStatus;
use models::RemoteServerConfig;
use models::StartCloudflaredTunnelInput;
use models::SwitchAccountResult;
use state::AppState;

// ===== Tauri Commands (thin wrappers) =====
// 命令函数仅负责参数编排与跨模块调用，
// 核心业务逻辑放在 account_service/auth/store/tray 等模块。

#[tauri::command]
async fn list_accounts(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<AccountSummary>, String> {
    account_service::list_accounts_internal(&app, state.inner()).await
}

#[tauri::command]
async fn import_current_auth_account(
    app: AppHandle,
    state: State<'_, AppState>,
    label: Option<String>,
) -> Result<AccountSummary, String> {
    let summary =
        account_service::import_current_auth_account_internal(&app, state.inner(), label).await?;
    let _ = tray::refresh_macos_tray_snapshot(&app);
    Ok(summary)
}

#[tauri::command]
async fn import_auth_json_accounts(
    app: AppHandle,
    state: State<'_, AppState>,
    items: Vec<AuthJsonImportInput>,
) -> Result<ImportAccountsResult, String> {
    let result =
        account_service::import_auth_json_accounts_internal(&app, state.inner(), items).await?;
    if result.imported_count > 0 || result.updated_count > 0 {
        let _ = tray::refresh_macos_tray_snapshot(&app);
    }
    Ok(result)
}

#[tauri::command]
async fn export_accounts_zip(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<String>, String> {
    account_service::export_accounts_zip_internal(&app, state.inner()).await
}

#[tauri::command]
async fn delete_account(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    account_service::delete_account_internal(&app, state.inner(), &id).await?;
    let _ = tray::refresh_macos_tray_snapshot(&app);
    Ok(())
}

#[tauri::command]
async fn update_account_label(
    app: AppHandle,
    state: State<'_, AppState>,
    account_key: String,
    label: String,
) -> Result<String, String> {
    let resolved_label =
        account_service::update_account_label_internal(&app, state.inner(), &account_key, label)
            .await?;

    {
        let api_proxy = state.api_proxy.lock().await;
        if let Some(handle) = api_proxy.as_ref() {
            let mut snapshot = handle.shared.lock().await;
            if snapshot.active_account_key.as_deref() == Some(account_key.as_str()) {
                snapshot.active_account_label = Some(resolved_label.clone());
            }
        }
    }

    let _ = tray::refresh_macos_tray_snapshot(&app);
    Ok(resolved_label)
}

#[tauri::command]
async fn refresh_all_usage(
    app: AppHandle,
    state: State<'_, AppState>,
    force_auth_refresh: Option<bool>,
) -> Result<Vec<AccountSummary>, String> {
    let summaries = account_service::refresh_all_usage_internal(
        &app,
        state.inner(),
        force_auth_refresh.unwrap_or(false),
    )
    .await?;
    let _ = tray::update_macos_tray_snapshot(&app, &summaries);
    Ok(summaries)
}

#[tauri::command]
async fn get_app_settings(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<AppSettings, String> {
    settings_service::get_app_settings_internal(&app, state.inner()).await
}

#[tauri::command]
async fn update_app_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    patch: AppSettingsPatch,
) -> Result<AppSettings, String> {
    let settings =
        settings_service::update_app_settings_internal(&app, state.inner(), patch).await?;
    let _ = tray::refresh_macos_tray_snapshot(&app);
    Ok(settings)
}

#[tauri::command]
fn detect_codex_app() -> Result<Option<String>, String> {
    Ok(cli::find_codex_app_path().map(|path| path.to_string_lossy().to_string()))
}

#[tauri::command]
fn list_installed_editor_apps() -> Result<Vec<InstalledEditorApp>, String> {
    Ok(editor_apps::list_installed_editor_apps())
}

#[tauri::command]
fn is_opencode_desktop_app_installed() -> Result<bool, String> {
    Ok(opencode::is_opencode_desktop_app_installed())
}

#[tauri::command]
fn open_external_url(url: String) -> Result<(), String> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("仅允许打开 http/https 链接".to_string());
    }

    #[cfg(target_os = "macos")]
    let mut cmd = {
        let mut command = Command::new("open");
        command.arg(&url);
        command
    };

    #[cfg(target_os = "windows")]
    let mut cmd = {
        let mut command = Command::new("cmd");
        command.args(["/C", "start", "", &url]);
        command
    };

    #[cfg(all(unix, not(target_os = "macos")))]
    let mut cmd = {
        let mut command = Command::new("xdg-open");
        command.arg(&url);
        command
    };

    cmd.spawn().map_err(|e| format!("打开外部链接失败: {e}"))?;
    Ok(())
}

#[tauri::command]
fn get_current_auth_status() -> Result<CurrentAuthStatus, String> {
    auth::read_current_auth_status()
}

#[tauri::command]
async fn launch_codex_login(state: State<'_, AppState>) -> Result<(), String> {
    // 添加账号流程前先备份当前 auth.json，确保授权结束后可回滚。
    let current_auth = auth::read_current_codex_auth_optional()?;
    {
        let mut backup = state.add_flow_auth_backup.lock().await;
        *backup = Some(current_auth);
    }

    let mut cmd = cli::new_codex_command()?;
    cmd.arg("login")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("无法启动 codex login: {e}"))?;
    Ok(())
}

#[tauri::command]
async fn restore_auth_after_add_flow(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    let backup = {
        let mut guard = state.add_flow_auth_backup.lock().await;
        guard.take()
    };

    match backup {
        None => Ok(false),
        Some(Some(auth_json)) => {
            auth::write_active_codex_auth(&auth_json)?;
            let _ = tray::refresh_macos_tray_snapshot(&app);
            Ok(true)
        }
        Some(None) => {
            auth::remove_active_codex_auth()?;
            let _ = tray::refresh_macos_tray_snapshot(&app);
            Ok(true)
        }
    }
}

#[tauri::command]
async fn switch_account_and_launch(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    workspace_path: Option<String>,
    launch_codex: Option<bool>,
    restart_editors_on_switch: Option<bool>,
    restart_editor_targets: Option<Vec<EditorAppId>>,
) -> Result<SwitchAccountResult, String> {
    let store = {
        let _guard = state.store_lock.lock().await;
        store::load_store(&app)?
    };

    let account = store
        .accounts
        .iter()
        .find(|account| account.id == id)
        .cloned()
        .ok_or_else(|| "找不到要切换的账号".to_string())?;

    let should_sync_opencode = store.settings.sync_opencode_openai_auth;
    let should_restart_opencode_desktop =
        should_sync_opencode && store.settings.restart_opencode_desktop_on_switch;
    let should_restart_editors =
        restart_editors_on_switch.unwrap_or(store.settings.restart_editors_on_switch);
    let effective_restart_targets =
        restart_editor_targets.unwrap_or_else(|| store.settings.restart_editor_targets.clone());
    auth::write_active_codex_auth(&account.auth_json)?;
    let _ = tray::refresh_macos_tray_snapshot(&app);

    let mut opencode_synced = false;
    let mut opencode_sync_error = None;
    let mut opencode_desktop_restarted = false;
    let mut opencode_desktop_restart_error = None;
    if should_sync_opencode {
        match opencode::sync_openai_auth_from_codex_auth(&account.auth_json) {
            Ok(()) => {
                opencode_synced = true;
                if should_restart_opencode_desktop {
                    match opencode::restart_opencode_desktop_app() {
                        Ok(()) => {
                            opencode_desktop_restarted = true;
                        }
                        Err(err) => {
                            log::warn!("重启 opencode 桌面端失败: {err}");
                            opencode_desktop_restart_error = Some(err);
                        }
                    }
                }
            }
            Err(err) => {
                log::warn!("同步 opencode OpenAI 认证失败: {err}");
                opencode_sync_error = Some(err);
            }
        }
    }

    let (restarted_editor_apps, editor_restart_error) = if should_restart_editors {
        editor_apps::restart_selected_editor_apps(&effective_restart_targets)
    } else {
        (Vec::new(), None)
    };

    // 向后兼容：旧前端未传参数时仍按“切换并启动”处理。
    let should_launch_codex = launch_codex.unwrap_or(true);
    if !should_launch_codex {
        return Ok(SwitchAccountResult {
            account_id: account.account_id,
            launched_app_path: None,
            used_fallback_cli: false,
            opencode_synced,
            opencode_sync_error,
            opencode_desktop_restarted,
            opencode_desktop_restart_error,
            restarted_editor_apps,
            editor_restart_error,
        });
    }

    // 切换时强制结束旧实例，避免触发“是否退出”确认弹窗。
    force_stop_running_codex();

    if let Some(path) = cli::find_codex_app_path() {
        let mut cmd = Command::new("open");
        cmd.arg("-na").arg(&path);
        if let Some(workspace) = workspace_path.as_deref() {
            cmd.arg(workspace);
        }
        let status = cmd
            .status()
            .map_err(|e| format!("启动 Codex.app 失败: {e}"))?;
        if !status.success() {
            return Err("Codex.app 启动失败".to_string());
        }

        return Ok(SwitchAccountResult {
            account_id: account.account_id,
            launched_app_path: Some(path.to_string_lossy().to_string()),
            used_fallback_cli: false,
            opencode_synced,
            opencode_sync_error,
            opencode_desktop_restarted,
            opencode_desktop_restart_error,
            restarted_editor_apps,
            editor_restart_error,
        });
    }

    let mut cmd = cli::new_codex_command()?;
    cmd.arg("app");
    if let Some(workspace) = workspace_path.as_deref() {
        cmd.arg(workspace);
    }
    cmd.spawn()
        .map_err(|e| format!("未检测到 Codex.app，且通过 codex app 启动失败: {e}"))?;

    Ok(SwitchAccountResult {
        account_id: account.account_id,
        launched_app_path: None,
        used_fallback_cli: true,
        opencode_synced,
        opencode_sync_error,
        opencode_desktop_restarted,
        opencode_desktop_restart_error,
        restarted_editor_apps,
        editor_restart_error,
    })
}

#[tauri::command]
async fn get_api_proxy_status(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ApiProxyStatus, String> {
    proxy_service::get_api_proxy_status_internal(&app, state.inner()).await
}

#[tauri::command]
async fn start_api_proxy(
    app: AppHandle,
    state: State<'_, AppState>,
    port: Option<u16>,
) -> Result<ApiProxyStatus, String> {
    proxy_service::start_api_proxy_internal(&app, state.inner(), port).await
}

#[tauri::command]
async fn stop_api_proxy(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ApiProxyStatus, String> {
    proxy_service::stop_api_proxy_internal(&app, state.inner()).await
}

#[tauri::command]
async fn refresh_api_proxy_key(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ApiProxyStatus, String> {
    proxy_service::refresh_api_proxy_key_internal(&app, state.inner()).await
}

#[tauri::command]
async fn get_cloudflared_status(state: State<'_, AppState>) -> Result<CloudflaredStatus, String> {
    cloudflared_service::get_cloudflared_status_internal(state.inner()).await
}

#[tauri::command]
async fn install_cloudflared(state: State<'_, AppState>) -> Result<CloudflaredStatus, String> {
    cloudflared_service::install_cloudflared_internal(state.inner()).await
}

#[tauri::command]
async fn start_cloudflared_tunnel(
    app: AppHandle,
    state: State<'_, AppState>,
    input: StartCloudflaredTunnelInput,
) -> Result<CloudflaredStatus, String> {
    cloudflared_service::start_cloudflared_tunnel_internal(&app, state.inner(), input).await
}

#[tauri::command]
async fn stop_cloudflared_tunnel(state: State<'_, AppState>) -> Result<CloudflaredStatus, String> {
    cloudflared_service::stop_cloudflared_tunnel_internal(state.inner()).await
}

#[tauri::command]
async fn get_remote_proxy_status(server: RemoteServerConfig) -> Result<RemoteProxyStatus, String> {
    remote_service::get_remote_proxy_status_internal(server).await
}

#[tauri::command]
async fn deploy_remote_proxy(
    app: AppHandle,
    input: DeployRemoteProxyInput,
) -> Result<RemoteProxyStatus, String> {
    remote_service::deploy_remote_proxy_internal(&app, input).await
}

#[tauri::command]
async fn start_remote_proxy(server: RemoteServerConfig) -> Result<RemoteProxyStatus, String> {
    remote_service::start_remote_proxy_internal(server).await
}

#[tauri::command]
async fn stop_remote_proxy(server: RemoteServerConfig) -> Result<RemoteProxyStatus, String> {
    remote_service::stop_remote_proxy_internal(server).await
}

#[tauri::command]
async fn read_remote_proxy_logs(
    server: RemoteServerConfig,
    lines: Option<usize>,
) -> Result<String, String> {
    remote_service::read_remote_proxy_logs_internal(server, lines.unwrap_or(120)).await
}

#[tauri::command]
async fn pick_local_identity_file() -> Result<Option<String>, String> {
    remote_service::pick_local_identity_file_internal().await
}

#[tauri::command]
async fn is_sshpass_available() -> Result<bool, String> {
    Ok(remote_service::is_sshpass_available_internal().await)
}

#[tauri::command]
async fn install_sshpass() -> Result<(), String> {
    remote_service::install_sshpass_internal().await
}

fn force_stop_running_codex() {
    #[cfg(target_os = "macos")]
    {
        let _ = Command::new("pkill").args(["-9", "-x", "Codex"]).status();
        let _ = Command::new("pkill")
            .args(["-9", "-x", "Codex Desktop"])
            .status();
    }

    #[cfg(target_os = "windows")]
    {
        let _ = Command::new("taskkill")
            .args(["/F", "/IM", "Codex.exe", "/T"])
            .status();
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = Command::new("pkill").args(["-9", "-x", "Codex"]).status();
    }

    // 等待进程树收敛，避免新实例拉起时与旧实例短暂重叠。
    thread::sleep(Duration::from_millis(220));
}

fn handle_window_close_to_background(window: &tauri::Window, event: &WindowEvent) {
    if let WindowEvent::CloseRequested { api, .. } = event {
        api.prevent_close();
        if let Err(err) = window.hide() {
            log::warn!("隐藏窗口失败: {err}");
        }
        #[cfg(target_os = "macos")]
        {
            // 仅隐藏主窗口到后台时，同时隐藏 Dock 图标；
            // 应用仍继续运行，可从状态栏再次打开。
            if let Err(err) = window.app_handle().set_dock_visibility(false) {
                log::warn!("隐藏 Dock 图标失败: {err}");
            }
        }
    }
}

pub(crate) fn restore_main_window(app: &AppHandle) {
    #[cfg(target_os = "macos")]
    if let Err(err) = app.set_dock_visibility(true) {
        log::warn!("恢复 Dock 图标失败: {err}");
    }

    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

async fn auto_start_api_proxy_if_enabled(app: AppHandle) {
    let state = app.state::<AppState>();
    let (should_auto_start, saved_port) = {
        let _guard = state.store_lock.lock().await;
        match store::load_store(&app) {
            Ok(store) => (
                store.settings.auto_start_api_proxy,
                store.settings.api_proxy_port,
            ),
            Err(err) => {
                log::warn!("读取自动启动 API 反代设置失败: {err}");
                (false, 8787)
            }
        }
    };

    if !should_auto_start {
        return;
    }

    if let Err(err) =
        proxy_service::start_api_proxy_internal(&app, state.inner(), Some(saved_port)).await
    {
        log::warn!("应用启动时自动启动 API 反代失败: {err}");
    }
}

// ===== App Bootstrap =====

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .manage(AppState::default())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .on_menu_event(tray::handle_status_bar_menu_event)
        .on_window_event(handle_window_close_to_background)
        .setup(|app| {
            utils::prepare_process_path();

            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            if let Err(err) = settings_service::sync_autostart_from_store(app.handle()) {
                log::warn!("启动时同步开机启动状态失败: {err}");
            }
            // 启动阶段先同步当前本机登录账号，再初始化状态栏，保证首次展示即一致。
            store::sync_current_auth_account_on_startup(app.handle())?;
            tray::setup_system_tray(app.handle())?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_accounts,
            import_current_auth_account,
            import_auth_json_accounts,
            export_accounts_zip,
            delete_account,
            update_account_label,
            refresh_all_usage,
            get_app_settings,
            update_app_settings,
            detect_codex_app,
            list_installed_editor_apps,
            is_opencode_desktop_app_installed,
            open_external_url,
            get_current_auth_status,
            launch_codex_login,
            restore_auth_after_add_flow,
            switch_account_and_launch,
            get_api_proxy_status,
            start_api_proxy,
            stop_api_proxy,
            refresh_api_proxy_key,
            get_cloudflared_status,
            install_cloudflared,
            start_cloudflared_tunnel,
            stop_cloudflared_tunnel,
            get_remote_proxy_status,
            deploy_remote_proxy,
            start_remote_proxy,
            stop_remote_proxy,
            read_remote_proxy_logs,
            pick_local_identity_file,
            is_sshpass_available,
            install_sshpass
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app_handle, event| match event {
        tauri::RunEvent::Ready => {
            let app_handle = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                auto_start_api_proxy_if_enabled(app_handle).await;
            });
        }
        #[cfg(target_os = "macos")]
        tauri::RunEvent::Reopen { .. } => {
            restore_main_window(app_handle);
        }
        _ => {}
    });
}
