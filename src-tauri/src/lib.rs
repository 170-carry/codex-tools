mod account_service;
mod app_paths;
mod auth;
mod cli;
mod cloudflared_service;
mod command_line;
mod editor_apps;
mod i18n;
mod models;
mod opencode;
mod profile_files;
pub mod proxy_daemon;
mod proxy_service;
mod remote_service;
mod settings_service;
mod state;
mod store;
mod token_usage;
mod tray;
mod usage;
mod utils;

use std::io::ErrorKind;
use std::io::Read;
use std::io::Write;
use std::net::TcpListener;
use std::net::TcpStream;
use std::process::Command;
use std::thread;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use rfd::FileDialog;
use tauri::AppHandle;
use tauri::Emitter;
use tauri::Manager;
use tauri::State;
use tauri::WindowEvent;

use models::AccountSummary;
use models::ApiProxyKey;
use models::ApiProxyKeyUsageLogEntry;
use models::ApiProxyStatus;
use models::ApiProxyUsageStats;
use models::AppSettings;
use models::AppSettingsPatch;
use models::AuthJsonImportInput;
use models::CloudflaredStatus;
use models::CreateApiAccountInput;
use models::CreateApiProxyKeyInput;
use models::DeleteCodexSessionResult;
use models::DeployRemoteProxyInput;
use models::EditorAppId;
use models::ImportAccountsResult;
use models::InstalledEditorApp;
use models::OauthCallbackFinishedEvent;
use models::PreparedOauthLogin;
use models::RemoteProxyStatus;
use models::RemoteServerConfig;
use models::StartCloudflaredTunnelInput;
use models::SwitchAccountResult;
use models::TestApiAccountConnectionInput;
use models::TestApiAccountConnectionResult;
use models::UpdateApiProxyKeyInput;
use state::AppState;
use state::OauthCallbackListenerHandle;
#[cfg(target_os = "windows")]
use utils::new_background_command;

const OAUTH_CALLBACK_FINISHED_EVENT: &str = "oauth-callback-finished";
const APP_MENU_OPEN_SETTINGS_EVENT: &str = "app-menu-open-settings";
const APP_MENU_CHECK_UPDATE_EVENT: &str = "app-menu-check-update";
const CODEX_COST_ANALYTICS_PROGRESS_EVENT: &str = "codex-cost-analytics-progress";
const CODEX_COST_ANALYTICS_CACHE_FILE: &str = "codex-cost-analytics-cache.json";
const APP_MENU_SETTINGS_ID: &str = "app_menu_settings";
const APP_MENU_CHECK_UPDATES_ID: &str = "app_menu_check_updates";
const AUTH_KEEPALIVE_INTERVAL_SECS: u64 = 300;

#[cfg(target_os = "macos")]
const APP_MENU_ABOUT_ICON: tauri::image::Image<'_> = tauri::include_image!("./icons/icon.png");

fn escape_html(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn write_oauth_html_response(
    stream: &mut std::net::TcpStream,
    status_line: &str,
    title: &str,
    detail: &str,
) {
    let body = format!(
        "<!doctype html><html lang=\"zh-CN\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>{}</title><style>body{{margin:0;padding:32px;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif;background:#f4f7fb;color:#152033}}main{{max-width:560px;margin:0 auto;padding:24px;border-radius:20px;background:#fff;box-shadow:0 14px 34px rgba(21,32,51,.08)}}h1{{margin:0 0 10px;font-size:24px;line-height:1.2}}p{{margin:0;color:#52627b;line-height:1.6;word-break:break-word}}</style></head><body><main><h1>{}</h1><p>{}</p></main></body></html>",
        escape_html(title),
        escape_html(title),
        escape_html(detail)
    );
    let response = format!(
        "HTTP/1.1 {status_line}\r\nContent-Type: text/html; charset=utf-8\r\nCache-Control: no-store\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
}

fn read_oauth_request_path(stream: &mut std::net::TcpStream) -> Result<String, String> {
    stream
        .set_read_timeout(Some(Duration::from_secs(4)))
        .map_err(|error| format!("设置 OAuth 回调读取超时失败: {error}"))?;
    let mut buffer = [0_u8; 8192];
    let bytes_read = stream
        .read(&mut buffer)
        .map_err(|error| format!("读取 OAuth 回调请求失败: {error}"))?;
    if bytes_read == 0 {
        return Err("OAuth 回调连接已关闭".to_string());
    }

    let request = String::from_utf8_lossy(&buffer[..bytes_read]);
    let request_line = request
        .lines()
        .next()
        .ok_or_else(|| "OAuth 回调请求为空".to_string())?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    if method != "GET" {
        return Err(format!("不支持的 OAuth 回调请求方法: {method}"));
    }

    parts
        .next()
        .map(ToString::to_string)
        .ok_or_else(|| "OAuth 回调请求缺少路径".to_string())
}

fn build_oauth_callback_url(redirect_uri: &str, path: &str) -> Result<String, String> {
    let mut callback_url = reqwest::Url::parse(redirect_uri)
        .map_err(|error| format!("OAuth redirect_uri 无效: {error}"))?;
    let request_url = reqwest::Url::parse(&format!("http://localhost{path}"))
        .map_err(|error| format!("OAuth 回调路径无效: {error}"))?;
    callback_url.set_path(request_url.path());
    callback_url.set_query(request_url.query());
    callback_url.set_fragment(request_url.fragment());
    Ok(callback_url.to_string())
}

fn bind_oauth_callback_listener(preferred_port: u16) -> Result<(Vec<TcpListener>, u16), String> {
    match bind_oauth_callback_listener_on_port(preferred_port) {
        Ok(listeners) => return Ok((listeners, preferred_port)),
        Err(error) if error.kind() == ErrorKind::AddrInUse => {
            cancel_oauth_listener_on_port(preferred_port);
            for _ in 0..10 {
                thread::sleep(Duration::from_millis(100));
                match bind_oauth_callback_listener_on_port(preferred_port) {
                    Ok(listeners) => return Ok((listeners, preferred_port)),
                    Err(retry_error) if retry_error.kind() == ErrorKind::AddrInUse => {}
                    Err(retry_error) => {
                        return Err(format!(
                            "无法启动 OAuth 回调监听 localhost:{preferred_port}: {retry_error}"
                        ));
                    }
                }
            }

            let (fallback, port) = bind_oauth_callback_listener_on_ephemeral().map_err(
                |fallback_error| {
                    format!(
                        "无法启动 OAuth 回调监听 localhost:{preferred_port}: {error}；自动回退到空闲端口也失败: {fallback_error}"
                    )
                },
            )?;
            log::warn!(
                "OAuth 回调默认端口 {} 已占用，已自动回退到本地空闲端口 {}",
                preferred_port,
                port
            );
            Ok((fallback, port))
        }
        Err(error) => Err(format!(
            "无法启动 OAuth 回调监听 localhost:{preferred_port}: {error}"
        )),
    }
}

fn bind_oauth_callback_listener_on_port(port: u16) -> std::io::Result<Vec<TcpListener>> {
    let ipv4 = TcpListener::bind(("127.0.0.1", port))?;
    let mut listeners = vec![ipv4];
    if let Some(ipv6) = bind_optional_oauth_ipv6_listener(port)? {
        listeners.push(ipv6);
    }
    Ok(listeners)
}

fn bind_oauth_callback_listener_on_ephemeral() -> std::io::Result<(Vec<TcpListener>, u16)> {
    let mut last_error = None;
    for _ in 0..10 {
        let ipv4 = TcpListener::bind(("127.0.0.1", 0))?;
        let port = ipv4.local_addr()?.port();
        let mut listeners = vec![ipv4];
        match bind_optional_oauth_ipv6_listener(port) {
            Ok(Some(ipv6)) => {
                listeners.push(ipv6);
                return Ok((listeners, port));
            }
            Ok(None) => return Ok((listeners, port)),
            Err(error) if error.kind() == ErrorKind::AddrInUse => {
                last_error = Some(error);
            }
            Err(error) => return Err(error),
        }
    }

    Err(last_error.unwrap_or_else(|| {
        std::io::Error::new(ErrorKind::AddrInUse, "无法找到可用的 OAuth 回调端口")
    }))
}

fn bind_optional_oauth_ipv6_listener(port: u16) -> std::io::Result<Option<TcpListener>> {
    match TcpListener::bind(("::1", port)) {
        Ok(listener) => Ok(Some(listener)),
        Err(error)
            if matches!(
                error.kind(),
                ErrorKind::AddrNotAvailable | ErrorKind::Unsupported
            ) =>
        {
            log::warn!("当前系统无法监听 IPv6 OAuth 回调 ::1:{port}: {error}");
            Ok(None)
        }
        Err(error) => Err(error),
    }
}

fn cancel_oauth_listener_on_port(port: u16) {
    for host in ["127.0.0.1", "::1"] {
        if let Err(error) = send_oauth_cancel_request(host, port) {
            log::debug!("取消旧 OAuth 回调监听 {host}:{port} 失败: {error}");
        }
    }
}

fn send_oauth_cancel_request(host: &str, port: u16) -> std::io::Result<()> {
    let address = if host == "::1" {
        format!("[::1]:{port}")
    } else {
        format!("{host}:{port}")
    };
    let mut stream = TcpStream::connect_timeout(
        &address
            .parse()
            .map_err(|error| std::io::Error::new(ErrorKind::InvalidInput, error))?,
        Duration::from_millis(350),
    )?;
    stream.set_read_timeout(Some(Duration::from_millis(350)))?;
    stream.set_write_timeout(Some(Duration::from_millis(350)))?;
    stream.write_all(b"GET /cancel HTTP/1.1\r\n")?;
    stream.write_all(format!("Host: {address}\r\n").as_bytes())?;
    stream.write_all(b"Connection: close\r\n\r\n")?;
    let mut buffer = [0_u8; 64];
    let _ = stream.read(&mut buffer);
    Ok(())
}

async fn stop_oauth_callback_listener(state: &AppState) {
    let handle = {
        let mut guard = state.oauth_listener.lock().await;
        guard.take()
    };

    let Some(mut handle) = handle else {
        return;
    };

    if let Some(shutdown_tx) = handle.shutdown_tx.take() {
        let _ = shutdown_tx.send(());
    }

    if let Some(task) = handle.task.take() {
        let _ = tauri::async_runtime::spawn_blocking(move || {
            let _ = task.join();
        })
        .await;
    }
}

async fn clear_pending_oauth_if_matches(state: &AppState, expected_state: &str) {
    let mut guard = state.pending_oauth_login.lock().await;
    if guard
        .as_ref()
        .is_some_and(|pending| pending.state.as_str() == expected_state)
    {
        *guard = None;
    }
}

async fn import_oauth_auth_json(
    app: &AppHandle,
    state: &AppState,
    auth_json: serde_json::Value,
    source: &str,
) -> Result<ImportAccountsResult, String> {
    let serialized = serde_json::to_string(&auth_json)
        .map_err(|error| format!("序列化 OAuth 登录结果失败: {error}"))?;
    let result = account_service::import_auth_json_accounts_internal(
        app,
        state,
        vec![AuthJsonImportInput {
            source: source.to_string(),
            content: serialized,
            label: None,
        }],
    )
    .await?;

    if result.imported_count > 0 || result.updated_count > 0 {
        let _ = tray::refresh_macos_tray_snapshot(app);
    }

    Ok(result)
}

async fn complete_oauth_login_internal(
    app: &AppHandle,
    state: &AppState,
    callback_url: &str,
) -> Result<ImportAccountsResult, String> {
    let pending = {
        let guard = state.pending_oauth_login.lock().await;
        guard
            .clone()
            .ok_or_else(|| "请先打开授权页面".to_string())?
    };

    let auth_json = auth::complete_oauth_callback_login(&pending, callback_url).await?;
    if let Some(account_id) = pending.reauthorize_account_id.as_deref() {
        account_service::reauthorize_account_internal(app, state, account_id, auth_json).await
    } else {
        import_oauth_auth_json(app, state, auth_json, "oauth-callback").await
    }
}

async fn emit_oauth_callback_finished(app: &AppHandle, payload: OauthCallbackFinishedEvent) {
    let _ = app.emit(OAUTH_CALLBACK_FINISHED_EVENT, payload);
}

fn run_oauth_callback_listener(
    app: AppHandle,
    listeners: Vec<TcpListener>,
    pending: auth::PendingOauthLogin,
    shutdown_rx: std::sync::mpsc::Receiver<()>,
) {
    loop {
        if shutdown_rx.try_recv().is_ok() {
            break;
        }

        let now = match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(duration) => duration.as_secs() as i64,
            Err(_) => 0,
        };
        if now >= pending.expires_at {
            tauri::async_runtime::block_on(async {
                let state = app.state::<AppState>();
                clear_pending_oauth_if_matches(state.inner(), &pending.state).await;
                emit_oauth_callback_finished(
                    &app,
                    OauthCallbackFinishedEvent {
                        result: None,
                        error: Some("OAuth 授权已超时，请重新打开授权页面。".to_string()),
                    },
                )
                .await;
            });
            break;
        }

        let mut accepted_stream = None;
        let mut listener_error = None;
        for listener in &listeners {
            match listener.accept() {
                Ok((stream, _)) => {
                    accepted_stream = Some(stream);
                    break;
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(error) => {
                    listener_error = Some(error);
                    break;
                }
            }
        }

        if let Some(error) = listener_error {
            tauri::async_runtime::block_on(async {
                emit_oauth_callback_finished(
                    &app,
                    OauthCallbackFinishedEvent {
                        result: None,
                        error: Some(format!("OAuth 回调监听失败: {error}")),
                    },
                )
                .await;
            });
            break;
        }

        if let Some(mut stream) = accepted_stream {
            let path = match read_oauth_request_path(&mut stream) {
                Ok(value) => value,
                Err(error) => {
                    write_oauth_html_response(&mut stream, "400 Bad Request", "授权失败", &error);
                    break;
                }
            };

            if path == "/cancel" {
                write_oauth_html_response(
                    &mut stream,
                    "200 OK",
                    "授权已取消",
                    "当前授权监听已取消，可以关闭这个页面。",
                );
                break;
            }

            if !path.starts_with("/auth/callback") {
                write_oauth_html_response(
                    &mut stream,
                    "404 Not Found",
                    "未识别的回调地址",
                    "当前地址不是 Codex Tools 的 OAuth 回调地址，可以关闭这个页面。",
                );
                continue;
            }

            let callback_url = match build_oauth_callback_url(&pending.redirect_uri, &path) {
                Ok(value) => value,
                Err(error) => {
                    write_oauth_html_response(&mut stream, "400 Bad Request", "授权失败", &error);
                    break;
                }
            };
            let callback_result = tauri::async_runtime::block_on(async {
                let state = app.state::<AppState>();
                let pending_matches = {
                    let guard = state.pending_oauth_login.lock().await;
                    guard
                        .as_ref()
                        .is_some_and(|current| current.state.as_str() == pending.state.as_str())
                };
                if !pending_matches {
                    return Err("当前授权会话已失效，请回到应用重新打开授权页面。".to_string());
                }

                let result =
                    complete_oauth_login_internal(&app, state.inner(), &callback_url).await;
                clear_pending_oauth_if_matches(state.inner(), &pending.state).await;
                result
            });

            match callback_result {
                Ok(result) => {
                    write_oauth_html_response(
                        &mut stream,
                        "200 OK",
                        "授权完成",
                        "账号已经写入 Codex Tools，可以回到应用继续操作。",
                    );
                    restore_main_window(&app);
                    tauri::async_runtime::block_on(async {
                        emit_oauth_callback_finished(
                            &app,
                            OauthCallbackFinishedEvent {
                                result: Some(result),
                                error: None,
                            },
                        )
                        .await;
                    });
                }
                Err(error) => {
                    write_oauth_html_response(&mut stream, "400 Bad Request", "授权失败", &error);
                    restore_main_window(&app);
                    if !error.contains("会话已失效") {
                        tauri::async_runtime::block_on(async {
                            emit_oauth_callback_finished(
                                &app,
                                OauthCallbackFinishedEvent {
                                    result: None,
                                    error: Some(error),
                                },
                            )
                            .await;
                        });
                    }
                }
            }
            break;
        } else {
            thread::sleep(Duration::from_millis(120));
        }
    }

    tauri::async_runtime::block_on(async {
        let state = app.state::<AppState>();
        let mut guard = state.oauth_listener.lock().await;
        *guard = None;
    });
}

async fn start_oauth_callback_listener(
    app: &AppHandle,
    state: &AppState,
    listeners: Vec<TcpListener>,
    pending: &auth::PendingOauthLogin,
) -> Result<(), String> {
    for listener in &listeners {
        listener
            .set_nonblocking(true)
            .map_err(|error| format!("无法设置 OAuth 回调监听模式: {error}"))?;
    }

    let (shutdown_tx, shutdown_rx) = std::sync::mpsc::channel();
    let app_handle = app.clone();
    let pending_login = pending.clone();
    let task = thread::spawn(move || {
        run_oauth_callback_listener(app_handle, listeners, pending_login, shutdown_rx);
    });

    let mut guard = state.oauth_listener.lock().await;
    *guard = Some(OauthCallbackListenerHandle {
        shutdown_tx: Some(shutdown_tx),
        task: Some(task),
    });
    Ok(())
}

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
async fn create_api_account(
    app: AppHandle,
    state: State<'_, AppState>,
    input: CreateApiAccountInput,
) -> Result<AccountSummary, String> {
    let summary = account_service::create_api_account_internal(&app, state.inner(), input).await?;
    let _ = tray::refresh_macos_tray_snapshot(&app);
    Ok(summary)
}

#[tauri::command]
async fn test_api_account_connection(
    input: TestApiAccountConnectionInput,
) -> Result<TestApiAccountConnectionResult, String> {
    account_service::test_api_account_connection_internal(input).await
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
    account_key: Option<String>,
) -> Result<Option<String>, String> {
    account_service::export_accounts_zip_internal(&app, state.inner(), account_key).await
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
async fn update_account_api_proxy_enabled(
    app: AppHandle,
    state: State<'_, AppState>,
    account_key: String,
    enabled: bool,
) -> Result<bool, String> {
    let resolved_enabled = account_service::update_account_api_proxy_enabled_internal(
        &app,
        state.inner(),
        &account_key,
        enabled,
    )
    .await?;

    {
        let api_proxy = state.api_proxy.lock().await;
        if let Some(handle) = api_proxy.as_ref() {
            let mut snapshot = handle.shared.lock().await;
            if !resolved_enabled
                && snapshot.active_account_key.as_deref() == Some(account_key.as_str())
            {
                snapshot.active_account_key = None;
                snapshot.active_account_id = None;
                snapshot.active_account_label = None;
                snapshot.sequential_account_key = None;
            }
        }
    }

    Ok(resolved_enabled)
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
async fn get_codex_token_usage() -> Result<token_usage::CodexTokenUsageSnapshot, String> {
    tauri::async_runtime::spawn_blocking(token_usage::collect_codex_token_usage_snapshot)
        .await
        .map_err(|error| format!("统计 Codex token 用量失败: {error}"))?
}

#[tauri::command]
async fn get_codex_cost_analytics(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<token_usage::CodexCostAnalyticsSnapshot, String> {
    let budget = settings_service::get_app_settings_internal(&app, state.inner())
        .await?
        .codex_analytics_weekly_budget_usd;
    let cache_path = codex_cost_analytics_cache_path(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        load_or_refresh_codex_cost_analytics_cache(&cache_path, budget)
    })
    .await
    .map_err(|error| format!("统计 Codex 成本分析失败: {error}"))?
}

#[tauri::command]
async fn get_cached_codex_cost_analytics(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<token_usage::CodexCostAnalyticsSnapshot>, String> {
    let budget = settings_service::get_app_settings_internal(&app, state.inner())
        .await?
        .codex_analytics_weekly_budget_usd;
    let cache_path = codex_cost_analytics_cache_path(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        load_cached_codex_cost_analytics_from_path(&cache_path, budget)
    })
    .await
    .map_err(|error| format!("读取 Codex 成本分析缓存失败: {error}"))?
}

#[tauri::command]
async fn refresh_codex_cost_analytics(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<token_usage::CodexCostAnalyticsSnapshot, String> {
    let budget = settings_service::get_app_settings_internal(&app, state.inner())
        .await?
        .codex_analytics_weekly_budget_usd;
    let cache_path = codex_cost_analytics_cache_path(&app)?;
    let progress_app = app.clone();

    tauri::async_runtime::spawn_blocking(
        move || -> Result<token_usage::CodexCostAnalyticsSnapshot, String> {
            let snapshot = token_usage::collect_codex_cost_analytics_snapshot_with_progress(
                budget,
                |progress| {
                    let _ = progress_app.emit(CODEX_COST_ANALYTICS_PROGRESS_EVENT, progress);
                },
            )?;

            let _ = progress_app.emit(
                CODEX_COST_ANALYTICS_PROGRESS_EVENT,
                token_usage::CodexCostAnalyticsProgress {
                    stage: "caching".to_string(),
                    processed_files: snapshot.source_path_count,
                    total_files: snapshot.source_path_count,
                    percent: 100,
                    current_path: Some(cache_path.to_string_lossy().to_string()),
                },
            );
            write_codex_cost_analytics_cache_to_path(&cache_path, &snapshot)?;

            let cached = load_cached_codex_cost_analytics_from_path(&cache_path, budget)?
                .unwrap_or(snapshot);
            let _ = progress_app.emit(
                CODEX_COST_ANALYTICS_PROGRESS_EVENT,
                token_usage::CodexCostAnalyticsProgress {
                    stage: "complete".to_string(),
                    processed_files: cached.source_path_count,
                    total_files: cached.source_path_count,
                    percent: 100,
                    current_path: None,
                },
            );
            Ok(cached)
        },
    )
    .await
    .map_err(|error| format!("刷新 Codex 成本分析失败: {error}"))?
}

#[tauri::command]
async fn export_codex_cost_analytics(
    app: AppHandle,
    state: State<'_, AppState>,
    format: String,
) -> Result<Option<String>, String> {
    let normalized_format = match format.as_str() {
        "csv" | "json" => format,
        other => return Err(format!("不支持的导出格式: {other}")),
    };
    let budget = settings_service::get_app_settings_internal(&app, state.inner())
        .await?
        .codex_analytics_weekly_budget_usd;
    let cache_path = codex_cost_analytics_cache_path(&app)?;

    tauri::async_runtime::spawn_blocking(move || {
        let snapshot = load_or_refresh_codex_cost_analytics_cache(&cache_path, budget)?;
        let bytes =
            token_usage::serialize_codex_cost_analytics_export(&snapshot, &normalized_format)?;
        export_codex_cost_analytics_file(&normalized_format, &bytes)
    })
    .await
    .map_err(|error| format!("导出 Codex 成本分析失败: {error}"))?
}

#[tauri::command]
async fn delete_codex_session(
    app: AppHandle,
    source_path: String,
    session_id: String,
) -> Result<DeleteCodexSessionResult, String> {
    let cache_path = codex_cost_analytics_cache_path(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let codex_dir = app_paths::codex_dir()?;
        let roots = [
            codex_dir.join("sessions"),
            codex_dir.join("archived_sessions"),
        ];
        let deleted_path = delete_codex_session_from_roots(
            &roots,
            std::path::Path::new(&source_path),
            &session_id,
        )?;
        match std::fs::remove_file(&cache_path) {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "删除成本分析缓存失败 {}: {error}",
                    cache_path.display()
                ));
            }
        }

        Ok(DeleteCodexSessionResult {
            session_id,
            deleted_path: deleted_path.to_string_lossy().to_string(),
        })
    })
    .await
    .map_err(|error| format!("删除 Codex 会话失败: {error}"))?
}

fn delete_codex_session_from_roots(
    roots: &[std::path::PathBuf],
    source_path: &std::path::Path,
    session_id: &str,
) -> Result<std::path::PathBuf, String> {
    let session_id = session_id.trim();
    if session_id.is_empty() {
        return Err("会话 ID 为空".to_string());
    }
    if source_path.extension().and_then(|value| value.to_str()) != Some("jsonl") {
        return Err("只允许删除 Codex JSONL 会话文件".to_string());
    }

    let canonical_source = source_path
        .canonicalize()
        .map_err(|error| format!("读取 Codex 会话文件失败 {}: {error}", source_path.display()))?;
    let allowed = roots.iter().any(|root| {
        root.canonicalize()
            .map(|canonical_root| canonical_source.starts_with(canonical_root))
            .unwrap_or(false)
    });
    if !allowed {
        return Err("只允许删除 Codex sessions 目录内的会话文件".to_string());
    }

    if !codex_session_file_matches_id(&canonical_source, session_id)? {
        return Err("会话文件与请求的会话 ID 不匹配".to_string());
    }

    std::fs::remove_file(&canonical_source).map_err(|error| {
        format!(
            "删除 Codex 会话文件失败 {}: {error}",
            canonical_source.display()
        )
    })?;
    Ok(canonical_source)
}

fn codex_session_file_matches_id(path: &std::path::Path, session_id: &str) -> Result<bool, String> {
    let raw = std::fs::read_to_string(path)
        .map_err(|error| format!("读取 Codex 会话文件失败 {}: {error}", path.display()))?;
    for line in raw.lines() {
        let Ok(root) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if root.get("type").and_then(serde_json::Value::as_str) != Some("session_meta") {
            continue;
        }
        let Some(id) = root
            .get("payload")
            .and_then(|payload| payload.get("id"))
            .and_then(serde_json::Value::as_str)
        else {
            continue;
        };
        return Ok(id == session_id);
    }

    Ok(path.file_stem().and_then(|value| value.to_str()) == Some(session_id))
}

fn codex_cost_analytics_cache_path(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    Ok(app_paths::app_data_dir(app)?.join(CODEX_COST_ANALYTICS_CACHE_FILE))
}

fn load_cached_codex_cost_analytics_from_path(
    cache_path: &std::path::Path,
    budget: Option<f64>,
) -> Result<Option<token_usage::CodexCostAnalyticsSnapshot>, String> {
    match std::fs::read_to_string(cache_path) {
        Ok(raw) => token_usage::parse_codex_cost_analytics_cache(&raw, budget),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!(
            "读取成本分析缓存失败 {}: {error}",
            cache_path.display()
        )),
    }
}

fn load_or_refresh_codex_cost_analytics_cache(
    cache_path: &std::path::Path,
    budget: Option<f64>,
) -> Result<token_usage::CodexCostAnalyticsSnapshot, String> {
    if let Some(snapshot) = load_cached_codex_cost_analytics_from_path(cache_path, budget)? {
        return Ok(snapshot);
    }

    let snapshot = token_usage::collect_codex_cost_analytics_snapshot(budget)?;
    write_codex_cost_analytics_cache_to_path(cache_path, &snapshot)?;
    Ok(load_cached_codex_cost_analytics_from_path(cache_path, budget)?.unwrap_or(snapshot))
}

fn write_codex_cost_analytics_cache_to_path(
    cache_path: &std::path::Path,
    snapshot: &token_usage::CodexCostAnalyticsSnapshot,
) -> Result<(), String> {
    let bytes = token_usage::serialize_codex_cost_analytics_cache(snapshot)?;
    write_private_file(cache_path, &bytes, "成本分析缓存")
}

fn export_codex_cost_analytics_file(
    format: &str,
    export_payload: &[u8],
) -> Result<Option<String>, String> {
    let (label, extension) = match format {
        "json" => ("JSON", "json"),
        "csv" => ("CSV", "csv"),
        other => return Err(format!("不支持的导出格式: {other}")),
    };
    let default_file_name = format!(
        "codex-cost-analytics-{}.{}",
        utils::now_unix_seconds(),
        extension
    );

    let Some(selected_path) = FileDialog::new()
        .set_title("导出 Codex 成本分析")
        .add_filter(label, &[extension])
        .set_file_name(&default_file_name)
        .save_file()
    else {
        return Ok(None);
    };

    let export_path = ensure_extension(selected_path, extension);
    write_private_export_file(&export_path, export_payload)?;
    Ok(Some(export_path.to_string_lossy().to_string()))
}

fn ensure_extension(path: std::path::PathBuf, extension: &str) -> std::path::PathBuf {
    if path.extension().and_then(|value| value.to_str()) == Some(extension) {
        path
    } else {
        path.with_extension(extension)
    }
}

fn write_private_export_file(path: &std::path::Path, export_payload: &[u8]) -> Result<(), String> {
    write_private_file(path, export_payload, "导出")
}

fn write_private_file(path: &std::path::Path, payload: &[u8], label: &str) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("无法解析{label}目录 {}", path.display()))?;
    std::fs::create_dir_all(parent)
        .map_err(|error| format!("创建{label}目录失败 {}: {error}", parent.display()))?;
    let temp_path = parent.join(format!(
        ".{}.tmp-{}",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("codex-cost-analytics"),
        uuid::Uuid::new_v4()
    ));

    let write_result = (|| -> Result<(), String> {
        std::fs::write(&temp_path, payload)
            .map_err(|error| format!("写入{label}临时文件失败 {}: {error}", temp_path.display()))?;
        utils::set_private_permissions(&temp_path);
        std::fs::rename(&temp_path, path)
            .map_err(|error| format!("保存{label}文件失败 {}: {error}", path.display()))?;
        utils::set_private_permissions(path);
        Ok(())
    })();

    if write_result.is_err() {
        let _ = std::fs::remove_file(&temp_path);
    }
    write_result
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
    {
        Command::new("open")
            .arg(&url)
            .spawn()
            .map_err(|e| format!("打开外部链接失败: {e}"))?;
        return Ok(());
    }

    #[cfg(target_os = "windows")]
    {
        // Avoid `cmd /C start` here. OAuth URLs contain `&`, and cmd treats them
        // as command separators unless they are shell-escaped very carefully.
        // Prefer the Windows URL protocol handler so the link goes to the
        // user's default browser instead of opening a File Explorer window.
        let mut primary = new_background_command("rundll32.exe");
        primary
            .args(["url.dll,FileProtocolHandler", &url])
            .spawn()
            .or_else(|primary_error| {
                let mut fallback = new_background_command("explorer.exe");
                fallback.arg(&url).spawn().map_err(|fallback_error| {
                    format!("打开外部链接失败: rundll32={primary_error}; explorer={fallback_error}")
                })
            })?;
        Ok(())
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Command::new("xdg-open")
            .arg(&url)
            .spawn()
            .map_err(|e| format!("打开外部链接失败: {e}"))?;
        Ok(())
    }
}

#[tauri::command]
async fn pick_codex_launch_path(
    kind: String,
    current_path: Option<String>,
) -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut dialog = FileDialog::new().set_title("选择 Codex 启动路径");

        if let Some(current_path) = current_path {
            let current_path = std::path::PathBuf::from(current_path);
            let initial_dir = if current_path.is_dir() {
                current_path
            } else {
                current_path
                    .parent()
                    .map(std::path::Path::to_path_buf)
                    .unwrap_or(current_path)
            };
            dialog = dialog.set_directory(initial_dir);
        }

        let selected = match kind.as_str() {
            "file" => dialog.pick_file(),
            "directory" => dialog.pick_folder(),
            _ => return Err("不支持的路径选择类型".to_string()),
        };

        Ok(selected.map(|path| path.to_string_lossy().to_string()))
    })
    .await
    .map_err(|error| format!("打开 Codex 路径选择器失败: {error}"))?
}

#[tauri::command]
async fn prepare_oauth_login(
    app: AppHandle,
    state: State<'_, AppState>,
    account_id: Option<String>,
) -> Result<PreparedOauthLogin, String> {
    let _oauth_guard = state.oauth_flow_lock.lock().await;
    stop_oauth_callback_listener(state.inner()).await;
    let (listener, redirect_port) = bind_oauth_callback_listener(auth::oauth_redirect_port())?;
    let (mut pending, prepared) = auth::prepare_oauth_login(redirect_port)?;
    pending.reauthorize_account_id = account_id.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    });
    {
        let mut guard = state.pending_oauth_login.lock().await;
        *guard = Some(pending.clone());
    }
    if let Err(error) = start_oauth_callback_listener(&app, state.inner(), listener, &pending).await
    {
        let mut guard = state.pending_oauth_login.lock().await;
        *guard = None;
        return Err(error);
    }
    Ok(prepared)
}

#[tauri::command]
async fn complete_oauth_callback_login(
    app: AppHandle,
    state: State<'_, AppState>,
    callback_url: String,
) -> Result<ImportAccountsResult, String> {
    let _oauth_guard = state.oauth_flow_lock.lock().await;
    let pending = {
        let guard = state.pending_oauth_login.lock().await;
        guard
            .clone()
            .ok_or_else(|| "请先打开授权页面".to_string())?
    };
    let result = complete_oauth_login_internal(&app, state.inner(), &callback_url).await?;
    clear_pending_oauth_if_matches(state.inner(), &pending.state).await;
    stop_oauth_callback_listener(state.inner()).await;
    Ok(result)
}

#[tauri::command]
async fn cancel_oauth_login(state: State<'_, AppState>) -> Result<(), String> {
    let _oauth_guard = state.oauth_flow_lock.lock().await;
    {
        let mut guard = state.pending_oauth_login.lock().await;
        *guard = None;
    }
    stop_oauth_callback_listener(state.inner()).await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::bind_oauth_callback_listener;
    use super::build_oauth_callback_url;
    use super::delete_codex_session_from_roots;
    use std::fs;
    use std::net::TcpListener;
    use std::path::Path;
    use std::path::PathBuf;
    use std::time::SystemTime;
    use std::time::UNIX_EPOCH;

    fn unique_test_dir(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "codex-tools-lib-test-{name}-{}-{nonce}",
            std::process::id()
        ))
    }

    fn write_test_session(path: &Path, session_id: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create test session dir");
        }
        fs::write(
            path,
            format!(
                "{{\"type\":\"session_meta\",\"payload\":{{\"id\":\"{session_id}\",\"cwd\":\"/tmp/project\"}}}}\n"
            ),
        )
        .expect("write test session");
    }

    #[test]
    fn build_oauth_callback_url_uses_redirect_origin_and_runtime_query() {
        let callback_url = build_oauth_callback_url(
            "http://localhost:17888/auth/callback",
            "/auth/callback?code=abc&state=xyz",
        )
        .expect("callback url should be built");

        assert_eq!(
            callback_url,
            "http://localhost:17888/auth/callback?code=abc&state=xyz"
        );
    }

    #[test]
    fn bind_oauth_callback_listener_falls_back_when_preferred_port_is_busy() {
        let occupied = TcpListener::bind(("127.0.0.1", 0)).expect("should bind a local test port");
        let preferred_port = occupied
            .local_addr()
            .expect("should read local addr")
            .port();

        let (_listeners, resolved_port) =
            bind_oauth_callback_listener(preferred_port).expect("bind should fall back");

        assert_ne!(resolved_port, preferred_port);
    }

    #[test]
    fn bind_oauth_callback_listener_uses_preferred_port_when_available() {
        let probe = TcpListener::bind(("127.0.0.1", 0)).expect("should bind a local test port");
        let preferred_port = probe.local_addr().expect("should read local addr").port();
        drop(probe);

        let (listeners, resolved_port) =
            bind_oauth_callback_listener(preferred_port).expect("bind should use preferred port");

        assert_eq!(resolved_port, preferred_port);
        assert!(!listeners.is_empty());
    }

    #[test]
    fn delete_codex_session_rejects_paths_outside_session_roots() {
        let sandbox = unique_test_dir("outside-session-root");
        let sessions = sandbox.join("codex").join("sessions");
        let archived_sessions = sandbox.join("codex").join("archived_sessions");
        fs::create_dir_all(&sessions).expect("create sessions root");
        fs::create_dir_all(&archived_sessions).expect("create archived sessions root");
        let outside = sandbox.join("outside").join("session-1.jsonl");
        write_test_session(&outside, "session-1");

        let error =
            delete_codex_session_from_roots(&[sessions, archived_sessions], &outside, "session-1")
                .expect_err("outside file should be rejected");

        assert!(error.contains("sessions"));
        assert!(outside.is_file());
        let _ = fs::remove_dir_all(sandbox);
    }

    #[test]
    fn delete_codex_session_rejects_mismatched_session_id() {
        let sandbox = unique_test_dir("mismatched-session");
        let sessions = sandbox.join("codex").join("sessions");
        let archived_sessions = sandbox.join("codex").join("archived_sessions");
        fs::create_dir_all(&sessions).expect("create sessions root");
        fs::create_dir_all(&archived_sessions).expect("create archived sessions root");
        let source = sessions.join("session-1.jsonl");
        write_test_session(&source, "session-1");

        let error =
            delete_codex_session_from_roots(&[sessions, archived_sessions], &source, "session-2")
                .expect_err("mismatched session should be rejected");

        assert!(error.contains("不匹配"));
        assert!(source.is_file());
        let _ = fs::remove_dir_all(sandbox);
    }

    #[test]
    fn delete_codex_session_removes_only_target_jsonl() {
        let sandbox = unique_test_dir("delete-session");
        let sessions = sandbox.join("codex").join("sessions");
        let archived_sessions = sandbox.join("codex").join("archived_sessions");
        fs::create_dir_all(&sessions).expect("create sessions root");
        fs::create_dir_all(&archived_sessions).expect("create archived sessions root");
        let target = sessions.join("session-1.jsonl");
        let other = sessions.join("session-2.jsonl");
        write_test_session(&target, "session-1");
        write_test_session(&other, "session-2");

        let deleted =
            delete_codex_session_from_roots(&[sessions, archived_sessions], &target, "session-1")
                .expect("target session should be deleted");

        assert_eq!(
            deleted.file_name().and_then(|value| value.to_str()),
            Some("session-1.jsonl")
        );
        assert!(!target.exists());
        assert!(other.is_file());
        let _ = fs::remove_dir_all(sandbox);
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

    let mut account = store
        .accounts
        .iter()
        .find(|account| account.id == id)
        .cloned()
        .ok_or_else(|| "找不到要切换的账号".to_string())?;

    if matches!(account.source_kind, models::AccountSourceKind::Chatgpt)
        && auth::auth_tokens_need_refresh(&account.auth_json)
    {
        if account.auth_refresh_blocked {
            return Err(format!(
                "切换账号前刷新登录令牌失败: {}",
                account
                    .auth_refresh_error
                    .clone()
                    .unwrap_or_else(|| "工具保存的授权快照已失效，请重新登录授权。".to_string())
            ));
        }

        let refreshed_auth = match auth::refresh_chatgpt_auth_tokens_serialized(
            &account.auth_json,
            &state.auth_refresh_lock,
        )
        .await
        {
            Ok(refreshed_auth) => refreshed_auth,
            Err(error) => {
                let normalized_error = normalize_switch_refresh_error(&error);
                let should_block_refresh = normalized_error
                    == "当前账号的 refresh_token 已失效或已被轮换，请重新登录授权。"
                    || normalized_error == "当前账号授权已过期，请重新登录授权。";

                if should_block_refresh {
                    let blocked_message = "工具保存的授权快照已失效，请重新登录授权。";
                    match app_paths::app_data_dir(&app) {
                        Ok(data_dir) => {
                            let store_path = store::account_store_path_from_data_dir(&data_dir);
                            if let Err(persist_error) =
                                store::update_account_group_refresh_state_in_path(
                                    &store_path,
                                    &account.account_key(),
                                    None,
                                    true,
                                    Some(blocked_message),
                                    utils::now_unix_seconds(),
                                    true,
                                )
                            {
                                log::warn!("切换失败后写回账号停刷状态失败: {persist_error}");
                            }
                        }
                        Err(path_error) => {
                            log::warn!("切换失败后获取应用数据目录失败: {path_error}");
                        }
                    }
                }

                return Err(format!("切换账号前刷新登录令牌失败: {normalized_error}"));
            }
        };

        account.auth_json = refreshed_auth.clone();

        let refreshed_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| format!("读取系统时间失败: {error}"))?
            .as_secs() as i64;
        let _guard = state.store_lock.lock().await;
        let mut latest_store = store::load_store(&app)?;
        let stored_account = latest_store
            .accounts
            .iter_mut()
            .find(|stored| stored.id == id)
            .ok_or_else(|| "找不到要切换的账号".to_string())?;
        stored_account.auth_json = refreshed_auth;
        stored_account.updated_at = refreshed_at;
        stored_account.auth_refresh_blocked = false;
        stored_account.auth_refresh_error = None;
        store::save_store(&app, &latest_store)?;
    }

    let should_sync_opencode = store.settings.sync_opencode_openai_auth;
    let should_restart_opencode_desktop =
        should_sync_opencode && store.settings.restart_opencode_desktop_on_switch;
    let should_restart_editors =
        restart_editors_on_switch.unwrap_or(store.settings.restart_editors_on_switch);
    let effective_restart_targets =
        restart_editor_targets.unwrap_or_else(|| store.settings.restart_editor_targets.clone());
    let configured_codex_launch_path = store.settings.codex_launch_path.clone();
    let launch_codex_as_admin = store.settings.launch_codex_as_admin;
    {
        let _guard = state.store_lock.lock().await;
        let mut latest_store = store::load_store(&app)?;
        let stored_account = latest_store
            .accounts
            .iter_mut()
            .find(|stored| stored.id == id)
            .ok_or_else(|| "找不到要切换的账号".to_string())?;
        profile_files::sync_account_profile_in_store_path(
            &store::account_store_path_from_data_dir(&app_paths::app_data_dir(&app)?),
            stored_account,
        )?;
        profile_files::apply_account_profile(stored_account)?;
        latest_store.settings.active_account_id = Some(stored_account.id.clone());
        account = stored_account.clone();
        store::save_store(&app, &latest_store)?;
    }
    let _ = tray::refresh_macos_tray_snapshot(&app);

    let mut opencode_synced = false;
    let mut opencode_sync_error = None;
    let mut opencode_desktop_restarted = false;
    let mut opencode_desktop_restart_error = None;
    if should_sync_opencode {
        match if matches!(account.source_kind, models::AccountSourceKind::Chatgpt) {
            opencode::sync_openai_auth_from_codex_auth(&account.auth_json)
        } else {
            Err("当前条目为 API 中转站配置，无法同步为 opencode 的 OAuth 登录态。".to_string())
        } {
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

    let mut app_launch_error = None;
    if let Some(path) = cli::find_configured_codex_app_path(configured_codex_launch_path.as_deref())
        .or_else(cli::find_codex_app_path)
    {
        match launch_codex_app(&path, workspace_path.as_deref(), launch_codex_as_admin) {
            Ok(()) => {
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
            Err(error) => {
                log::warn!("通过 Codex 应用路径启动失败 {}: {}", path.display(), error);
                app_launch_error = Some(error);
            }
        }
    }

    #[cfg(target_os = "windows")]
    if cli::has_windows_store_codex_app() {
        if launch_codex_as_admin {
            let error =
                "微软商店版 Codex 不支持以管理员身份启动，请在设置里指定桌面版 Codex.exe 或安装 Codex CLI。"
                    .to_string();
            log::warn!("{error}");
            app_launch_error = Some(match app_launch_error {
                Some(previous_error) => format!("{previous_error}；且{error}"),
                None => error,
            });
        } else {
            match cli::launch_windows_store_codex() {
                Ok(()) => {
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
                Err(error) => {
                    log::warn!("通过 Windows Store AUMID 启动 Codex 失败: {error}");
                    app_launch_error = Some(match app_launch_error {
                        Some(previous_error) => {
                            format!(
                                "{previous_error}；且通过 Windows Store AUMID 启动失败: {error}"
                            )
                        }
                        None => format!("通过 Windows Store AUMID 启动失败: {error}"),
                    });
                }
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        if launch_codex_as_admin {
            let mut args = vec!["app".to_string()];
            if let Some(workspace) = workspace_path.as_deref() {
                args.push(workspace.to_string());
            }
            cli::launch_codex_command_elevated(configured_codex_launch_path.as_deref(), &args)
                .map_err(|e| {
                    if let Some(app_launch_error) = app_launch_error.as_ref() {
                        format!(
                            "通过 Codex 应用路径启动失败: {app_launch_error}；且通过管理员 codex app 启动失败: {e}"
                        )
                    } else {
                        format!("未检测到本地 Codex 应用，且通过管理员 codex app 启动失败: {e}")
                    }
                })?;
        } else {
            let mut cmd = cli::new_codex_command(configured_codex_launch_path.as_deref())?;
            cmd.arg("app");
            if let Some(workspace) = workspace_path.as_deref() {
                cmd.arg(workspace);
            }
            cmd.spawn().map_err(|e| {
                if let Some(app_launch_error) = app_launch_error.as_ref() {
                    format!(
                        "通过 Codex 应用路径启动失败: {app_launch_error}；且通过 codex app 启动失败: {e}"
                    )
                } else {
                    format!("未检测到本地 Codex 应用，且通过 codex app 启动失败: {e}")
                }
            })?;
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = launch_codex_as_admin;
        let mut cmd = cli::new_codex_command(configured_codex_launch_path.as_deref())?;
        cmd.arg("app");
        if let Some(workspace) = workspace_path.as_deref() {
            cmd.arg(workspace);
        }
        cmd.spawn().map_err(|e| {
            if let Some(app_launch_error) = app_launch_error.as_ref() {
                format!(
                    "通过 Codex 应用路径启动失败: {app_launch_error}；且通过 codex app 启动失败: {e}"
                )
            } else {
                format!("未检测到本地 Codex 应用，且通过 codex app 启动失败: {e}")
            }
        })?;
    }

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

fn launch_codex_app(
    path: &std::path::Path,
    workspace_path: Option<&str>,
    launch_as_admin: bool,
) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let _ = launch_as_admin;
        let mut cmd = Command::new("open");
        cmd.arg("-na").arg(path);
        if let Some(workspace) = workspace_path {
            cmd.arg(workspace);
        }
        let status = cmd
            .status()
            .map_err(|e| format!("启动 Codex 应用失败: {e}"))?;
        if !status.success() {
            return Err("启动 Codex 应用失败".to_string());
        }
        return Ok(());
    }

    #[cfg(target_os = "windows")]
    {
        if cli::is_windows_store_codex_path(path) {
            let _ = workspace_path;
            if launch_as_admin {
                return Err(
                    "微软商店版 Codex 不支持以管理员身份启动，请在设置里指定桌面版 Codex.exe 或安装 Codex CLI。"
                        .to_string(),
                );
            }
            return cli::launch_windows_store_codex();
        }

        if launch_as_admin {
            let args = workspace_path
                .map(|workspace| vec![workspace.to_string()])
                .unwrap_or_default();
            return cli::launch_elevated_process(path, &args)
                .map_err(|error| format!("启动 Codex 应用失败: {error}"));
        }

        let mut cmd = new_background_command(path);
        if let Some(workspace) = workspace_path {
            cmd.arg(workspace);
        }
        cmd.spawn()
            .map_err(|e| format!("启动 Codex 应用失败: {e}"))?;
        return Ok(());
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = launch_as_admin;
        let mut cmd = Command::new(path);
        if let Some(workspace) = workspace_path {
            cmd.arg(workspace);
        }
        cmd.spawn()
            .map_err(|e| format!("启动 Codex 应用失败: {e}"))?;
        return Ok(());
    }

    #[cfg(not(any(unix, target_os = "windows")))]
    {
        let _ = path;
        let _ = workspace_path;
        let _ = launch_as_admin;
        Err("当前平台暂不支持直接启动 Codex 应用".to_string())
    }
}

fn normalize_switch_refresh_error(raw_error: &str) -> String {
    let normalized = raw_error.to_ascii_lowercase();
    if normalized.contains("refresh_token_reused")
        || is_invalid_refresh_grant(&normalized)
        || normalized
            .contains("your refresh token has already been used to generate a new access token")
        || normalized.contains("refresh token expired")
        || normalized.contains("refresh_token expired")
        || normalized.contains("expired refresh token")
        || normalized.contains("refresh token is expired")
        || normalized.contains("refresh token revoked")
        || normalized.contains("refresh_token_revoked")
        || normalized.contains("refresh token invalid")
        || normalized.contains("invalid refresh token")
    {
        return "当前账号的 refresh_token 已失效或已被轮换，请重新登录授权。".to_string();
    }
    if normalized.contains("please try signing in again")
        || normalized.contains("provided authentication token is expired")
        || normalized.contains("token is expired")
    {
        return "当前账号授权已过期，请重新登录授权。".to_string();
    }
    raw_error.to_string()
}

fn is_invalid_refresh_grant(normalized_error: &str) -> bool {
    normalized_error.contains("invalid_grant")
        && (normalized_error.contains("refresh")
            || normalized_error.contains("expired")
            || normalized_error.contains("revoked")
            || normalized_error.contains("invalid"))
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
async fn bind_codex_to_api_proxy(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ApiProxyStatus, String> {
    proxy_service::bind_codex_to_api_proxy_internal(&app, state.inner()).await
}

#[tauri::command]
async fn restore_codex_proxy_binding(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ApiProxyStatus, String> {
    proxy_service::restore_codex_proxy_binding_internal(&app, state.inner()).await
}

#[tauri::command]
async fn list_api_proxy_keys(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<ApiProxyKey>, String> {
    proxy_service::list_api_proxy_keys_internal(&app, state.inner()).await
}

#[tauri::command]
async fn create_api_proxy_key(
    app: AppHandle,
    state: State<'_, AppState>,
    input: CreateApiProxyKeyInput,
) -> Result<Vec<ApiProxyKey>, String> {
    proxy_service::create_api_proxy_key_internal(&app, state.inner(), input).await
}

#[tauri::command]
async fn update_api_proxy_key(
    app: AppHandle,
    state: State<'_, AppState>,
    input: UpdateApiProxyKeyInput,
) -> Result<Vec<ApiProxyKey>, String> {
    proxy_service::update_api_proxy_key_internal(&app, state.inner(), input).await
}

#[tauri::command]
async fn delete_api_proxy_key(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<Vec<ApiProxyKey>, String> {
    proxy_service::delete_api_proxy_key_internal(&app, state.inner(), id).await
}

#[tauri::command]
async fn regenerate_api_proxy_key(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<Vec<ApiProxyKey>, String> {
    proxy_service::regenerate_api_proxy_key_internal(&app, state.inner(), id).await
}

#[tauri::command]
async fn get_api_proxy_key_usage_logs(
    app: AppHandle,
    state: State<'_, AppState>,
    limit: Option<usize>,
) -> Result<Vec<ApiProxyKeyUsageLogEntry>, String> {
    proxy_service::get_api_proxy_key_usage_logs_internal(&app, state.inner(), limit).await
}

#[tauri::command]
async fn get_api_proxy_usage_stats(
    app: AppHandle,
    state: State<'_, AppState>,
    range_seconds: Option<i64>,
) -> Result<ApiProxyUsageStats, String> {
    proxy_service::get_api_proxy_usage_stats_internal(&app, state.inner(), range_seconds).await
}

#[tauri::command]
async fn clear_api_proxy_usage_stats(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    proxy_service::clear_api_proxy_usage_stats_internal(&app, state.inner()).await
}

#[tauri::command]
fn get_api_proxy_supported_models() -> Result<Vec<String>, String> {
    Ok(proxy_service::get_api_proxy_supported_models_internal())
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
        let _ = new_background_command("taskkill")
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

fn start_auth_keepalive_loop(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            let state = app.state::<AppState>();
            match account_service::refresh_all_usage_internal(&app, state.inner(), true).await {
                Ok(summaries) => {
                    let _ = tray::update_macos_tray_snapshot(&app, &summaries);
                }
                Err(error) => {
                    log::warn!("后台账号保活失败: {error}");
                }
            }
            tokio::time::sleep(Duration::from_secs(AUTH_KEEPALIVE_INTERVAL_SECS)).await;
        }
    });
}

#[cfg(target_os = "macos")]
fn setup_macos_app_menu(app: &AppHandle) -> Result<(), String> {
    use tauri::menu::AboutMetadata;
    use tauri::menu::Menu;
    use tauri::menu::MenuItem;
    use tauri::menu::PredefinedMenuItem;
    use tauri::menu::Submenu;
    use tauri::menu::HELP_SUBMENU_ID;
    use tauri::menu::WINDOW_SUBMENU_ID;

    let locale = i18n::app_locale(app);
    let package_info = app.package_info();
    let app_name = package_info.name.clone();
    let app_version = package_info.version.to_string();
    let about_label = i18n::app_menu_about(locale, &app_name);
    let settings_label = i18n::app_menu_settings(locale);
    let check_updates_label = i18n::app_menu_check_updates(locale);
    let about_metadata = AboutMetadata {
        name: Some(app_name.clone()),
        version: Some(app_version.clone()),
        short_version: Some(app_version),
        copyright: app.config().bundle.copyright.clone(),
        authors: app
            .config()
            .bundle
            .publisher
            .clone()
            .map(|publisher| vec![publisher]),
        icon: Some(APP_MENU_ABOUT_ICON),
        ..Default::default()
    };

    let app_menu = Submenu::with_items(
        app,
        app_name,
        true,
        &[
            &PredefinedMenuItem::about(app, Some(&about_label), Some(about_metadata))
                .map_err(|e| format!("创建关于菜单失败: {e}"))?,
            &PredefinedMenuItem::separator(app).map_err(|e| format!("创建菜单分隔符失败: {e}"))?,
            &MenuItem::with_id(
                app,
                APP_MENU_SETTINGS_ID,
                settings_label,
                true,
                Some("CmdOrCtrl+,"),
            )
            .map_err(|e| format!("创建设置菜单失败: {e}"))?,
            &MenuItem::with_id(
                app,
                APP_MENU_CHECK_UPDATES_ID,
                check_updates_label,
                true,
                None::<&str>,
            )
            .map_err(|e| format!("创建更新菜单失败: {e}"))?,
            &PredefinedMenuItem::separator(app).map_err(|e| format!("创建菜单分隔符失败: {e}"))?,
            &PredefinedMenuItem::services(app, None)
                .map_err(|e| format!("创建服务菜单失败: {e}"))?,
            &PredefinedMenuItem::separator(app).map_err(|e| format!("创建菜单分隔符失败: {e}"))?,
            &PredefinedMenuItem::hide(app, None).map_err(|e| format!("创建隐藏菜单失败: {e}"))?,
            &PredefinedMenuItem::hide_others(app, None)
                .map_err(|e| format!("创建隐藏其他菜单失败: {e}"))?,
            &PredefinedMenuItem::show_all(app, None)
                .map_err(|e| format!("创建全部显示菜单失败: {e}"))?,
            &PredefinedMenuItem::separator(app).map_err(|e| format!("创建菜单分隔符失败: {e}"))?,
            &PredefinedMenuItem::quit(app, None).map_err(|e| format!("创建退出菜单失败: {e}"))?,
        ],
    )
    .map_err(|e| format!("创建应用菜单失败: {e}"))?;

    let file_menu = Submenu::with_items(
        app,
        "File",
        true,
        &[&PredefinedMenuItem::close_window(app, None)
            .map_err(|e| format!("创建关闭窗口菜单失败: {e}"))?],
    )
    .map_err(|e| format!("创建文件菜单失败: {e}"))?;

    let edit_menu = Submenu::with_items(
        app,
        "Edit",
        true,
        &[
            &PredefinedMenuItem::undo(app, None).map_err(|e| format!("创建撤销菜单失败: {e}"))?,
            &PredefinedMenuItem::redo(app, None).map_err(|e| format!("创建重做菜单失败: {e}"))?,
            &PredefinedMenuItem::separator(app).map_err(|e| format!("创建菜单分隔符失败: {e}"))?,
            &PredefinedMenuItem::cut(app, None).map_err(|e| format!("创建剪切菜单失败: {e}"))?,
            &PredefinedMenuItem::copy(app, None).map_err(|e| format!("创建复制菜单失败: {e}"))?,
            &PredefinedMenuItem::paste(app, None).map_err(|e| format!("创建粘贴菜单失败: {e}"))?,
            &PredefinedMenuItem::select_all(app, None)
                .map_err(|e| format!("创建全选菜单失败: {e}"))?,
        ],
    )
    .map_err(|e| format!("创建编辑菜单失败: {e}"))?;

    let view_menu = Submenu::with_items(
        app,
        "View",
        true,
        &[&PredefinedMenuItem::fullscreen(app, None)
            .map_err(|e| format!("创建全屏菜单失败: {e}"))?],
    )
    .map_err(|e| format!("创建视图菜单失败: {e}"))?;

    let window_menu = Submenu::with_id_and_items(
        app,
        WINDOW_SUBMENU_ID,
        "Window",
        true,
        &[
            &PredefinedMenuItem::minimize(app, None)
                .map_err(|e| format!("创建最小化菜单失败: {e}"))?,
            &PredefinedMenuItem::maximize(app, None)
                .map_err(|e| format!("创建缩放菜单失败: {e}"))?,
            &PredefinedMenuItem::separator(app).map_err(|e| format!("创建菜单分隔符失败: {e}"))?,
            &PredefinedMenuItem::close_window(app, None)
                .map_err(|e| format!("创建关闭窗口菜单失败: {e}"))?,
        ],
    )
    .map_err(|e| format!("创建窗口菜单失败: {e}"))?;

    let help_menu = Submenu::with_id_and_items(app, HELP_SUBMENU_ID, "Help", true, &[])
        .map_err(|e| format!("创建帮助菜单失败: {e}"))?;

    let menu = Menu::with_items(
        app,
        &[
            &app_menu,
            &file_menu,
            &edit_menu,
            &view_menu,
            &window_menu,
            &help_menu,
        ],
    )
    .map_err(|e| format!("创建顶部菜单失败: {e}"))?;

    app.set_menu(menu)
        .map(|_| ())
        .map_err(|e| format!("安装顶部菜单失败: {e}"))
}

#[cfg(not(target_os = "macos"))]
fn setup_macos_app_menu(_app: &AppHandle) -> Result<(), String> {
    Ok(())
}

fn handle_menu_event(app: &AppHandle, event: tauri::menu::MenuEvent) {
    let id = event.id().as_ref();
    if id == APP_MENU_SETTINGS_ID {
        restore_main_window(app);
        let _ = app.emit(APP_MENU_OPEN_SETTINGS_EVENT, ());
        return;
    }

    if id == APP_MENU_CHECK_UPDATES_ID {
        restore_main_window(app);
        let _ = app.emit(APP_MENU_CHECK_UPDATE_EVENT, ());
        return;
    }

    tray::handle_status_bar_menu_event(app, event);
}

// ===== App Bootstrap =====

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            log::info!("检测到重复启动请求，切换到现有实例");
            restore_main_window(app);
        }))
        .manage(AppState::default())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .on_menu_event(handle_menu_event)
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
            setup_macos_app_menu(app.handle())?;
            tray::setup_system_tray(app.handle())?;
            start_auth_keepalive_loop(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_accounts,
            import_current_auth_account,
            create_api_account,
            test_api_account_connection,
            import_auth_json_accounts,
            export_accounts_zip,
            delete_account,
            update_account_label,
            update_account_api_proxy_enabled,
            refresh_all_usage,
            get_codex_token_usage,
            get_codex_cost_analytics,
            get_cached_codex_cost_analytics,
            refresh_codex_cost_analytics,
            export_codex_cost_analytics,
            delete_codex_session,
            get_app_settings,
            update_app_settings,
            detect_codex_app,
            list_installed_editor_apps,
            is_opencode_desktop_app_installed,
            open_external_url,
            pick_codex_launch_path,
            prepare_oauth_login,
            complete_oauth_callback_login,
            cancel_oauth_login,
            switch_account_and_launch,
            get_api_proxy_status,
            start_api_proxy,
            stop_api_proxy,
            refresh_api_proxy_key,
            bind_codex_to_api_proxy,
            restore_codex_proxy_binding,
            list_api_proxy_keys,
            create_api_proxy_key,
            update_api_proxy_key,
            delete_api_proxy_key,
            regenerate_api_proxy_key,
            get_api_proxy_key_usage_logs,
            get_api_proxy_usage_stats,
            clear_api_proxy_usage_stats,
            get_api_proxy_supported_models,
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

pub fn try_run_cli_from_env() -> bool {
    command_line::try_run_from_env()
}

pub fn run_cli_from_env() -> ! {
    command_line::run_from_env_or_exit()
}
