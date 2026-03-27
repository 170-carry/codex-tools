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
use std::io::Read;
use std::io::Write;
use std::io::ErrorKind;
use std::net::TcpListener;
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
use models::ApiProxyStatus;
use models::AppSettings;
use models::AppSettingsPatch;
use models::AuthJsonImportInput;
use models::CloudflaredStatus;
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
use state::AppState;
use state::OauthCallbackListenerHandle;

const OAUTH_CALLBACK_FINISHED_EVENT: &str = "oauth-callback-finished";

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
        body.as_bytes().len(),
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

fn build_oauth_callback_url(
    redirect_uri: &str,
    path: &str,
) -> Result<String, String> {
    let mut callback_url =
        reqwest::Url::parse(redirect_uri).map_err(|error| format!("OAuth redirect_uri 无效: {error}"))?;
    let request_url = reqwest::Url::parse(&format!("http://localhost{path}"))
        .map_err(|error| format!("OAuth 回调路径无效: {error}"))?;
    callback_url.set_path(request_url.path());
    callback_url.set_query(request_url.query());
    callback_url.set_fragment(request_url.fragment());
    Ok(callback_url.to_string())
}

fn bind_oauth_callback_listener(preferred_port: u16) -> Result<(TcpListener, u16), String> {
    match TcpListener::bind(("127.0.0.1", preferred_port)) {
        Ok(listener) => Ok((listener, preferred_port)),
        Err(error) if error.kind() == ErrorKind::AddrInUse => {
            let fallback = TcpListener::bind(("127.0.0.1", 0)).map_err(|fallback_error| {
                format!(
                    "无法启动 OAuth 回调监听 127.0.0.1:{preferred_port}: {error}；自动回退到空闲端口也失败: {fallback_error}"
                )
            })?;
            let port = fallback
                .local_addr()
                .map_err(|addr_error| format!("无法读取 OAuth 回调监听端口: {addr_error}"))?
                .port();
            log::warn!(
                "OAuth 回调默认端口 {} 已占用，已自动回退到本地空闲端口 {}",
                preferred_port,
                port
            );
            Ok((fallback, port))
        }
        Err(error) => Err(format!(
            "无法启动 OAuth 回调监听 127.0.0.1:{preferred_port}: {error}"
        )),
    }
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
    import_oauth_auth_json(app, state, auth_json, "oauth-callback").await
}

async fn emit_oauth_callback_finished(app: &AppHandle, payload: OauthCallbackFinishedEvent) {
    let _ = app.emit(OAUTH_CALLBACK_FINISHED_EVENT, payload);
}

fn run_oauth_callback_listener(
    app: AppHandle,
    listener: TcpListener,
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

        match listener.accept() {
            Ok((mut stream, _)) => {
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
                        write_oauth_html_response(
                            &mut stream,
                            "400 Bad Request",
                            "授权失败",
                            &error,
                        );
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

                    let result = complete_oauth_login_internal(&app, state.inner(), &callback_url).await;
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
                        write_oauth_html_response(
                            &mut stream,
                            "400 Bad Request",
                            "授权失败",
                            &error,
                        );
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
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(120));
            }
            Err(error) => {
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
    listener: TcpListener,
    pending: &auth::PendingOauthLogin,
) -> Result<(), String> {
    listener
        .set_nonblocking(true)
        .map_err(|error| format!("无法设置 OAuth 回调监听模式: {error}"))?;

    let (shutdown_tx, shutdown_rx) = std::sync::mpsc::channel();
    let app_handle = app.clone();
    let pending_login = pending.clone();
    let task = thread::spawn(move || {
        run_oauth_callback_listener(app_handle, listener, pending_login, shutdown_rx);
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
        Command::new("rundll32.exe")
            .args(["url.dll,FileProtocolHandler", &url])
            .spawn()
            .or_else(|primary_error| {
                Command::new("explorer.exe")
                    .arg(&url)
                    .spawn()
                    .map_err(|fallback_error| {
                        format!(
                            "打开外部链接失败: rundll32={primary_error}; explorer={fallback_error}"
                        )
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
) -> Result<PreparedOauthLogin, String> {
    let _oauth_guard = state.oauth_flow_lock.lock().await;
    stop_oauth_callback_listener(state.inner()).await;
    let (listener, redirect_port) = bind_oauth_callback_listener(auth::oauth_redirect_port())?;
    let (pending, prepared) = auth::prepare_oauth_login(redirect_port)?;
    {
        let mut guard = state.pending_oauth_login.lock().await;
        *guard = Some(pending.clone());
    }
    if let Err(error) = start_oauth_callback_listener(&app, state.inner(), listener, &pending).await {
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
    use std::net::TcpListener;

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

        let (_listener, resolved_port) =
            bind_oauth_callback_listener(preferred_port).expect("bind should fall back");

        assert_ne!(resolved_port, preferred_port);
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
    let configured_codex_launch_path = store.settings.codex_launch_path.clone();
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

    if let Some(path) = cli::find_configured_codex_app_path(configured_codex_launch_path.as_deref())
        .or_else(cli::find_codex_app_path)
    {
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

    let mut cmd = cli::new_codex_command(configured_codex_launch_path.as_deref())?;
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
            pick_codex_launch_path,
            prepare_oauth_login,
            complete_oauth_callback_login,
            cancel_oauth_login,
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
