use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Stdio;

use reqwest::Client;
use serde::Deserialize;
use serde_json::json;
use tauri::AppHandle;

use crate::app_paths;
use crate::models::CloudflaredStatus;
use crate::models::CloudflaredTunnelMode;
use crate::models::NamedCloudflaredTunnelInput;
use crate::models::StartCloudflaredTunnelInput;
use crate::state::AppState;
use crate::state::CloudflaredRuntimeHandle;
use crate::utils::new_background_command;
use crate::utils::new_resolved_command;
use crate::utils::now_unix_seconds;

const CLOUDFLARE_API_BASE_URL: &str = "https://api.cloudflare.com/client/v4";

#[derive(Debug, Deserialize)]
struct CloudflareApiError {
    code: Option<i64>,
    message: String,
}

#[derive(Debug, Deserialize)]
struct CloudflareApiResponse<T> {
    #[serde(default)]
    success: bool,
    result: Option<T>,
    #[serde(default)]
    errors: Vec<CloudflareApiError>,
}

#[derive(Debug, Deserialize)]
struct TunnelCreateResult {
    id: String,
    token: String,
}

#[derive(Debug, Deserialize)]
struct DnsRecordResult {
    id: String,
}

fn cloudflared_status_template(binary_path: Option<&PathBuf>) -> CloudflaredStatus {
    CloudflaredStatus {
        installed: binary_path.is_some(),
        binary_path: binary_path.map(|value| value.to_string_lossy().to_string()),
        running: false,
        tunnel_mode: None,
        public_url: None,
        custom_hostname: None,
        use_http2: false,
        last_error: None,
    }
}

fn status_from_handle(
    binary_path: Option<&PathBuf>,
    handle: &CloudflaredRuntimeHandle,
    running: bool,
) -> CloudflaredStatus {
    CloudflaredStatus {
        installed: binary_path.is_some(),
        binary_path: binary_path.map(|value| value.to_string_lossy().to_string()),
        running,
        tunnel_mode: Some(handle.mode),
        public_url: handle.public_url.clone(),
        custom_hostname: handle.custom_hostname.clone(),
        use_http2: handle.use_http2,
        last_error: handle.last_error.clone(),
    }
}

pub(crate) async fn get_cloudflared_status_internal(
    state: &AppState,
) -> Result<CloudflaredStatus, String> {
    let binary_path = find_cloudflared_path();
    let mut guard = state.cloudflared.lock().await;
    let Some(handle) = guard.as_mut() else {
        return Ok(cloudflared_status_template(binary_path.as_ref()));
    };

    refresh_cloudflared_runtime(handle)?;
    if let Some(exit_status) = handle
        .child
        .try_wait()
        .map_err(|e| format!("读取 cloudflared 进程状态失败: {e}"))?
    {
        if !exit_status.success() && handle.last_error.is_none() {
            handle.last_error = read_last_log_line(&handle.log_path);
        }
        let status = status_from_handle(binary_path.as_ref(), handle, false);
        *guard = None;
        return Ok(status);
    }

    Ok(status_from_handle(binary_path.as_ref(), handle, true))
}

pub(crate) async fn install_cloudflared_internal(
    state: &AppState,
) -> Result<CloudflaredStatus, String> {
    if find_cloudflared_path().is_some() {
        return get_cloudflared_status_internal(state).await;
    }

    #[cfg(target_os = "macos")]
    {
        ensure_command_available(
            "brew",
            "--version",
            "未检测到 Homebrew，请先安装 brew 后再一键安装 cloudflared。",
        )?;
        run_install_command(
            "brew",
            &["install", "cloudflared"],
            "通过 Homebrew 安装 cloudflared 失败",
        )?;
    }

    #[cfg(target_os = "windows")]
    {
        ensure_command_available(
            "winget",
            "--version",
            "未检测到 winget，请先安装 winget 后再一键安装 cloudflared。",
        )?;
        run_install_command(
            "winget",
            &["install", "--id", "Cloudflare.cloudflared"],
            "通过 winget 安装 cloudflared 失败",
        )?;
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        return Err(
            "当前平台暂未内置一键安装 cloudflared，请先按 Cloudflare 官方文档安装。".to_string(),
        );
    }

    get_cloudflared_status_internal(state).await
}

pub(crate) async fn start_cloudflared_tunnel_internal(
    app: &AppHandle,
    state: &AppState,
    input: StartCloudflaredTunnelInput,
) -> Result<CloudflaredStatus, String> {
    if input.api_proxy_port == 0 {
        return Err("请先启动本地 API 反代，再开启公网访问。".to_string());
    }

    let binary_path = find_cloudflared_path()
        .ok_or_else(|| "尚未安装 cloudflared，请先完成安装。".to_string())?;

    {
        let mut guard = state.cloudflared.lock().await;
        if let Some(handle) = guard.as_mut() {
            refresh_cloudflared_runtime(handle)?;
            if handle
                .child
                .try_wait()
                .map_err(|e| format!("读取 cloudflared 进程状态失败: {e}"))?
                .is_none()
            {
                return Ok(status_from_handle(Some(&binary_path), handle, true));
            }
            *guard = None;
        }
    }

    let service_url = format!("http://127.0.0.1:{}", input.api_proxy_port);
    let log_path = next_cloudflared_log_path(app)?;

    let mut handle = match input.mode {
        CloudflaredTunnelMode::Quick => {
            ensure_quick_tunnel_is_allowed()?;
            spawn_quick_tunnel(&binary_path, &log_path, &service_url, input.use_http2)?
        }
        CloudflaredTunnelMode::Named => {
            let named = normalize_named_input(input.named)?;
            spawn_named_tunnel(
                &binary_path,
                &log_path,
                &service_url,
                input.use_http2,
                &named,
            )
            .await?
        }
    };

    refresh_cloudflared_runtime(&mut handle)?;
    let status = status_from_handle(Some(&binary_path), &handle, true);

    let mut guard = state.cloudflared.lock().await;
    *guard = Some(handle);

    Ok(status)
}

pub(crate) async fn stop_cloudflared_tunnel_internal(
    state: &AppState,
) -> Result<CloudflaredStatus, String> {
    let binary_path = find_cloudflared_path();
    let mut guard = state.cloudflared.lock().await;
    let Some(mut handle) = guard.take() else {
        return Ok(cloudflared_status_template(binary_path.as_ref()));
    };

    let _ = handle.child.kill();
    let _ = handle.child.wait();
    if let (Some(api_token), Some(account_id), Some(tunnel_id)) = (
        handle.cleanup_api_token.as_deref(),
        handle.cleanup_account_id.as_deref(),
        handle.cleanup_tunnel_id.as_deref(),
    ) {
        let client = Client::new();
        if let Err(error) = delete_named_tunnel(&client, api_token, account_id, tunnel_id).await {
            handle.last_error = Some(error);
        }
    }
    refresh_cloudflared_runtime(&mut handle)?;

    Ok(status_from_handle(binary_path.as_ref(), &handle, false))
}

fn normalize_named_input(
    input: Option<NamedCloudflaredTunnelInput>,
) -> Result<NamedCloudflaredTunnelInput, String> {
    let mut named = input.ok_or_else(|| {
        "命名隧道需要填写 Cloudflare API Token、Account ID、Zone ID 和自定义域名。".to_string()
    })?;

    named.api_token = named.api_token.trim().to_string();
    named.account_id = named.account_id.trim().to_string();
    named.zone_id = named.zone_id.trim().to_string();
    named.hostname = named
        .hostname
        .trim()
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/')
        .to_ascii_lowercase();

    if named.api_token.is_empty()
        || named.account_id.is_empty()
        || named.zone_id.is_empty()
        || named.hostname.is_empty()
    {
        return Err("命名隧道的所有字段都必须填写。".to_string());
    }
    if !named.hostname.contains('.') {
        return Err("自定义域名格式无效，请填写完整 Hostname，例如 api.example.com。".to_string());
    }

    Ok(named)
}

fn spawn_quick_tunnel(
    binary_path: &Path,
    log_path: &Path,
    service_url: &str,
    use_http2: bool,
) -> Result<CloudflaredRuntimeHandle, String> {
    let mut command = new_background_command(binary_path);
    command
        .arg("tunnel")
        .arg("--loglevel")
        .arg("info")
        .arg("--logfile")
        .arg(log_path)
        .arg("--no-autoupdate")
        .arg("--url")
        .arg(service_url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    if use_http2 {
        command.env("TUNNEL_TRANSPORT_PROTOCOL", "http2");
    }

    let child = command
        .spawn()
        .map_err(|e| format!("启动 Quick Tunnel 失败: {e}"))?;

    Ok(CloudflaredRuntimeHandle {
        mode: CloudflaredTunnelMode::Quick,
        use_http2,
        public_url: None,
        custom_hostname: None,
        last_error: None,
        cleanup_api_token: None,
        cleanup_account_id: None,
        cleanup_tunnel_id: None,
        log_path: log_path.to_path_buf(),
        child,
    })
}

async fn spawn_named_tunnel(
    binary_path: &Path,
    log_path: &Path,
    service_url: &str,
    use_http2: bool,
    named: &NamedCloudflaredTunnelInput,
) -> Result<CloudflaredRuntimeHandle, String> {
    let client = Client::new();
    let created = create_named_tunnel(&client, named).await?;
    let tunnel_target = format!("{}.cfargotunnel.com", created.id);

    configure_named_tunnel(&client, named, &created.id, &named.hostname, service_url).await?;
    upsert_cname_record(
        &client,
        &named.api_token,
        &named.zone_id,
        &named.hostname,
        &tunnel_target,
    )
    .await?;

    let mut command = new_background_command(binary_path);
    command
        .arg("tunnel")
        .arg("--loglevel")
        .arg("info")
        .arg("--logfile")
        .arg(log_path)
        .arg("--no-autoupdate");

    if use_http2 {
        command.arg("--protocol").arg("http2");
    }

    command
        .arg("run")
        .arg("--token")
        .arg(&created.token)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    let child = command
        .spawn()
        .map_err(|e| format!("启动命名隧道失败: {e}"))?;

    Ok(CloudflaredRuntimeHandle {
        mode: CloudflaredTunnelMode::Named,
        use_http2,
        public_url: Some(format!("https://{}", named.hostname)),
        custom_hostname: Some(named.hostname.clone()),
        last_error: None,
        cleanup_api_token: Some(named.api_token.clone()),
        cleanup_account_id: Some(named.account_id.clone()),
        cleanup_tunnel_id: Some(created.id),
        log_path: log_path.to_path_buf(),
        child,
    })
}

async fn create_named_tunnel(
    client: &Client,
    named: &NamedCloudflaredTunnelInput,
) -> Result<TunnelCreateResult, String> {
    let url = format!(
        "{CLOUDFLARE_API_BASE_URL}/accounts/{}/cfd_tunnel",
        named.account_id
    );
    let response = client
        .post(url)
        .bearer_auth(&named.api_token)
        .json(&json!({
            "name": format!("codex-tools-{}", uuid::Uuid::new_v4().simple()),
            "config_src": "cloudflare"
        }))
        .send()
        .await
        .map_err(|e| format!("创建命名隧道失败: {e}"))?;

    parse_cloudflare_response::<TunnelCreateResult>(response, "创建命名隧道失败").await
}

async fn configure_named_tunnel(
    client: &Client,
    named: &NamedCloudflaredTunnelInput,
    tunnel_id: &str,
    hostname: &str,
    service_url: &str,
) -> Result<(), String> {
    let url = format!(
        "{CLOUDFLARE_API_BASE_URL}/accounts/{}/cfd_tunnel/{}/configurations",
        named.account_id, tunnel_id
    );
    let response = client
        .put(url)
        .bearer_auth(&named.api_token)
        .json(&json!({
            "config": {
                "ingress": [
                    {
                        "hostname": hostname,
                        "service": service_url
                    },
                    {
                        "service": "http_status:404"
                    }
                ]
            }
        }))
        .send()
        .await
        .map_err(|e| format!("写入命名隧道配置失败: {e}"))?;

    let _: serde_json::Value = parse_cloudflare_response(response, "写入命名隧道配置失败").await?;
    Ok(())
}

async fn upsert_cname_record(
    client: &Client,
    api_token: &str,
    zone_id: &str,
    hostname: &str,
    target: &str,
) -> Result<(), String> {
    let list_url = format!("{CLOUDFLARE_API_BASE_URL}/zones/{zone_id}/dns_records");
    let list_response = client
        .get(&list_url)
        .bearer_auth(api_token)
        .query(&[("type", "CNAME"), ("name", hostname)])
        .send()
        .await
        .map_err(|e| format!("查询 DNS 记录失败: {e}"))?;
    let existing: Vec<DnsRecordResult> =
        parse_cloudflare_response(list_response, "查询 DNS 记录失败").await?;

    let payload = json!({
        "type": "CNAME",
        "name": hostname,
        "content": target,
        "proxied": true
    });

    if let Some(record) = existing.first() {
        let update_url = format!(
            "{CLOUDFLARE_API_BASE_URL}/zones/{zone_id}/dns_records/{}",
            record.id
        );
        let response = client
            .put(update_url)
            .bearer_auth(api_token)
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("更新 DNS 记录失败: {e}"))?;
        let _: serde_json::Value = parse_cloudflare_response(response, "更新 DNS 记录失败").await?;
    } else {
        let response = client
            .post(list_url)
            .bearer_auth(api_token)
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("创建 DNS 记录失败: {e}"))?;
        let _: serde_json::Value = parse_cloudflare_response(response, "创建 DNS 记录失败").await?;
    }

    Ok(())
}

async fn delete_named_tunnel(
    client: &Client,
    api_token: &str,
    account_id: &str,
    tunnel_id: &str,
) -> Result<(), String> {
    let url = format!("{CLOUDFLARE_API_BASE_URL}/accounts/{account_id}/cfd_tunnel/{tunnel_id}");
    let response = client
        .delete(url)
        .bearer_auth(api_token)
        .send()
        .await
        .map_err(|e| format!("清理命名隧道失败: {e}"))?;
    let _: serde_json::Value = parse_cloudflare_response(response, "清理命名隧道失败").await?;
    Ok(())
}

async fn parse_cloudflare_response<T: for<'de> Deserialize<'de>>(
    response: reqwest::Response,
    prefix: &str,
) -> Result<T, String> {
    let envelope = response
        .json::<CloudflareApiResponse<T>>()
        .await
        .map_err(|e| format!("{prefix}: {e}"))?;

    if !envelope.success {
        return Err(format!(
            "{prefix}: {}",
            join_cloudflare_errors(&envelope.errors)
        ));
    }

    envelope
        .result
        .ok_or_else(|| format!("{prefix}: Cloudflare 返回结果为空"))
}

fn join_cloudflare_errors(errors: &[CloudflareApiError]) -> String {
    if errors.is_empty() {
        return "未知错误".to_string();
    }

    errors
        .iter()
        .map(|item| match item.code {
            Some(code) => format!("[{code}] {}", item.message),
            None => item.message.clone(),
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

fn refresh_cloudflared_runtime(handle: &mut CloudflaredRuntimeHandle) -> Result<(), String> {
    if matches!(handle.mode, CloudflaredTunnelMode::Quick) && handle.public_url.is_none() {
        handle.public_url = extract_trycloudflare_url(&handle.log_path);
    }

    if handle.last_error.is_none()
        && handle
            .child
            .try_wait()
            .map_err(|e| format!("读取 cloudflared 进程状态失败: {e}"))?
            .is_some()
    {
        handle.last_error = read_last_log_line(&handle.log_path);
    }

    Ok(())
}

fn extract_trycloudflare_url(path: &Path) -> Option<String> {
    let raw = fs::read_to_string(path).ok()?;
    raw.split_whitespace().find_map(|segment| {
        if !segment.contains("trycloudflare.com") {
            return None;
        }

        let cleaned = segment.trim_matches(|char| {
            matches!(
                char,
                '"' | '\'' | '`' | '(' | ')' | '[' | ']' | '{' | '}' | ',' | ';'
            )
        });

        if cleaned.starts_with("https://") || cleaned.starts_with("http://") {
            Some(cleaned.to_string())
        } else {
            None
        }
    })
}

fn read_last_log_line(path: &Path) -> Option<String> {
    let raw = fs::read_to_string(path).ok()?;
    raw.lines()
        .map(str::trim)
        .rfind(|line| !line.is_empty())
        .map(ToString::to_string)
}

fn next_cloudflared_log_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app_paths::app_data_dir(app)?.join("cloudflared");
    fs::create_dir_all(&dir)
        .map_err(|e| format!("创建 cloudflared 日志目录失败 {}: {e}", dir.display()))?;

    let path = dir.join(format!("cloudflared-{}.log", now_unix_seconds()));
    fs::write(&path, "")
        .map_err(|e| format!("初始化 cloudflared 日志文件失败 {}: {e}", path.display()))?;
    Ok(path)
}

fn ensure_quick_tunnel_is_allowed() -> Result<(), String> {
    let Some(home) = dirs::home_dir() else {
        return Ok(());
    };

    let cloudflared_dir = home.join(".cloudflared");
    for candidate in [
        cloudflared_dir.join("config.yml"),
        cloudflared_dir.join("config.yaml"),
    ] {
        if candidate.exists() {
            return Err(
                "Quick Tunnel 与 ~/.cloudflared/config.yml 或 config.yaml 不兼容，请先移走该配置文件，或改用命名隧道。".to_string(),
            );
        }
    }

    Ok(())
}

fn run_install_command(cmd: &str, args: &[&str], prefix: &str) -> Result<(), String> {
    let output = new_resolved_command(cmd)
        .args(args)
        .output()
        .map_err(|e| format!("{prefix}: {e}"))?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let detail = if !stderr.is_empty() {
        stderr
    } else if !stdout.is_empty() {
        stdout
    } else {
        "命令返回了非零状态".to_string()
    };
    Err(format!("{prefix}: {detail}"))
}

fn ensure_command_available(
    command: &str,
    version_arg: &str,
    missing_message: &str,
) -> Result<(), String> {
    let status = new_resolved_command(command).arg(version_arg).status();
    match status {
        Ok(_) => Ok(()),
        Err(_) => Err(missing_message.to_string()),
    }
}

fn find_cloudflared_path() -> Option<PathBuf> {
    let mut candidates = cloudflared_candidates();
    let mut seen = HashSet::new();

    for candidate in candidates.drain(..) {
        if !seen.insert(candidate.clone()) {
            continue;
        }
        if is_executable_file(&candidate) {
            return Some(candidate);
        }
    }

    None
}

fn cloudflared_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Some(path_os) = env::var_os("PATH") {
        for dir in env::split_paths(&path_os) {
            push_cloudflared_candidates_from_dir(&mut candidates, &dir);
        }
    }

    #[cfg(target_os = "macos")]
    {
        for dir in [
            PathBuf::from("/opt/homebrew/bin"),
            PathBuf::from("/usr/local/bin"),
            PathBuf::from("/usr/bin"),
        ] {
            push_cloudflared_candidates_from_dir(&mut candidates, &dir);
        }
    }

    if let Some(home) = dirs::home_dir() {
        for dir in [
            home.join(".local").join("bin"),
            home.join("bin"),
            home.join("AppData")
                .join("Local")
                .join("Microsoft")
                .join("WinGet")
                .join("Links"),
        ] {
            push_cloudflared_candidates_from_dir(&mut candidates, &dir);
        }
    }

    candidates
}

fn push_cloudflared_candidates_from_dir(candidates: &mut Vec<PathBuf>, dir: &Path) {
    #[cfg(windows)]
    let names = ["cloudflared.exe", "cloudflared.cmd", "cloudflared.bat"];
    #[cfg(not(windows))]
    let names = ["cloudflared"];

    for name in names {
        candidates.push(dir.join(name));
    }
}

fn is_executable_file(path: &Path) -> bool {
    let Ok(metadata) = fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}
