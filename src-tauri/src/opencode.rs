use serde_json::Map;
use serde_json::Number;
use serde_json::Value;
use std::env;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::thread;
use std::time::Duration;
use std::time::Instant;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use crate::auth::extract_codex_oauth_tokens;
use crate::auth::CodexOAuthTokens;
use crate::utils::set_private_permissions;

const FALLBACK_EXPIRES_IN_MS: i64 = 55 * 60 * 1000;
const OPENCODE_DESKTOP_RESTART_SETTLE_MS: u64 = 220;
const OPENCODE_DESKTOP_RESTART_TIMEOUT_MS: u64 = 6_000;
const OPENCODE_DESKTOP_RESTART_POLL_MS: u64 = 120;

#[cfg(target_os = "macos")]
const OPENCODE_DESKTOP_MAC_APP_NAMES: &[&str] = &["Opencode.app", "OpenCode.app", "opencode.app"];
#[cfg(target_os = "macos")]
const OPENCODE_DESKTOP_MAC_BUNDLE_ID: &str = "ai.opencode.desktop";
#[cfg(target_os = "macos")]
const OPENCODE_DESKTOP_MAC_PROCESS_NAMES: &[&str] =
    &["opencode-cli", "Opencode", "OpenCode", "opencode"];
#[cfg(target_os = "windows")]
const OPENCODE_DESKTOP_WINDOWS_PROCESS_NAMES: &[&str] = &[
    "opencode-cli.exe",
    "Opencode.exe",
    "OpenCode.exe",
    "opencode.exe",
];

/// 同步 opencode 的 OpenAI 认证（openai.access/openai.refresh）。
///
/// 会自动探测：
/// 1. opencode 可执行文件位置（用于确认已安装）
/// 2. opencode 认证文件 `auth.json` 的位置
pub(crate) fn sync_openai_auth_from_codex_auth(auth_json: &Value) -> Result<(), String> {
    let tokens = extract_codex_oauth_tokens(auth_json)?;
    let install_path = detect_opencode_install_path();
    let auth_paths = detect_opencode_auth_paths();

    if install_path.is_none() && auth_paths.is_empty() {
        return Err("未检测到 opencode 安装位置或认证文件".to_string());
    }

    if auth_paths.is_empty() {
        return Err("未能定位 opencode 认证文件路径".to_string());
    }

    let mut success_paths = Vec::<String>::new();
    let mut errors = Vec::<String>::new();

    for auth_path in auth_paths {
        match sync_openai_auth_to_path(&auth_path, &tokens) {
            Ok(()) => success_paths.push(auth_path.display().to_string()),
            Err(err) => errors.push(format!("{}: {}", auth_path.display(), err)),
        }
    }

    if success_paths.is_empty() {
        return Err(errors.join(" | "));
    }

    log::info!(
        "Opencode OpenAI 认证已同步到: {}",
        success_paths.join(" | ")
    );
    if !errors.is_empty() {
        log::warn!("部分 opencode 认证文件同步失败: {}", errors.join(" | "));
    }
    Ok(())
}

pub(crate) fn is_opencode_desktop_app_installed() -> bool {
    detect_opencode_desktop_app_path().is_some()
}

pub(crate) fn restart_opencode_desktop_app() -> Result<(), String> {
    let app_path = detect_opencode_desktop_app_path()
        .ok_or_else(|| "未检测到 opencode 桌面端应用".to_string())?;

    #[cfg(target_os = "macos")]
    let previous_pids = list_running_opencode_desktop_pids();

    request_opencode_desktop_quit();
    thread::sleep(Duration::from_millis(OPENCODE_DESKTOP_RESTART_SETTLE_MS));
    force_kill_opencode_desktop_processes();

    #[cfg(target_os = "macos")]
    wait_for_opencode_desktop_exit()?;

    thread::sleep(Duration::from_millis(OPENCODE_DESKTOP_RESTART_SETTLE_MS));
    reopen_opencode_desktop_app(&app_path)?;

    #[cfg(target_os = "macos")]
    wait_for_opencode_desktop_launch(&previous_pids)?;

    Ok(())
}

fn sync_openai_auth_to_path(auth_path: &Path, tokens: &CodexOAuthTokens) -> Result<(), String> {
    let mut root = read_or_init_json_object(auth_path)?;
    let expires_ms = tokens
        .expires_at_ms
        .unwrap_or_else(|| now_unix_millis().saturating_add(FALLBACK_EXPIRES_IN_MS));
    let mut openai = root
        .get("openai")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();

    // 保留既有类型；若不存在则补默认值。
    let auth_type = openai
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("oauth")
        .to_string();
    openai.insert("type".to_string(), Value::String(auth_type));
    openai.insert(
        "access".to_string(),
        Value::String(tokens.access_token.clone()),
    );
    openai.insert(
        "refresh".to_string(),
        Value::String(tokens.refresh_token.clone()),
    );
    openai.insert(
        "expires".to_string(),
        Value::Number(Number::from(expires_ms)),
    );
    if let Some(account_id) = tokens.account_id.as_ref() {
        openai.insert(
            "accountId".to_string(),
            Value::String(account_id.to_string()),
        );
    }

    root.insert("openai".to_string(), Value::Object(openai));
    write_json_object(auth_path, &root)
}

fn detect_opencode_install_path() -> Option<PathBuf> {
    let mut candidates = Vec::<PathBuf>::new();

    if let Some(path_os) = env::var_os("PATH") {
        for dir in env::split_paths(&path_os) {
            candidates.push(dir.join("opencode"));
            #[cfg(windows)]
            {
                candidates.push(dir.join("opencode.exe"));
                candidates.push(dir.join("opencode.cmd"));
                candidates.push(dir.join("opencode.bat"));
            }
        }
    }

    if let Some(home) = dirs::home_dir() {
        candidates.push(home.join(".opencode").join("bin").join("opencode"));
        candidates.push(home.join(".local").join("bin").join("opencode"));
        #[cfg(windows)]
        {
            candidates.push(home.join(".opencode").join("bin").join("opencode.exe"));
        }
    }

    #[cfg(target_os = "macos")]
    {
        candidates.push(PathBuf::from("/opt/homebrew/bin/opencode"));
        candidates.push(PathBuf::from("/usr/local/bin/opencode"));
    }

    candidates.into_iter().find(|path| is_executable_file(path))
}

fn detect_opencode_desktop_app_path() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        let mut candidates = Vec::<PathBuf>::new();

        for bundle_name in OPENCODE_DESKTOP_MAC_APP_NAMES {
            candidates.push(PathBuf::from("/Applications").join(bundle_name));
            if let Some(home) = dirs::home_dir() {
                candidates.push(home.join("Applications").join(bundle_name));
            }
        }

        return candidates.into_iter().find(|path| path.exists());
    }

    #[cfg(target_os = "windows")]
    {
        let mut candidates = Vec::<PathBuf>::new();

        for base in [
            env::var_os("LOCALAPPDATA").map(PathBuf::from),
            env::var_os("APPDATA").map(PathBuf::from),
            env::var_os("ProgramFiles").map(PathBuf::from),
            env::var_os("ProgramFiles(x86)").map(PathBuf::from),
            dirs::home_dir().map(|home| home.join("AppData").join("Local")),
        ]
        .into_iter()
        .flatten()
        {
            for relative in [
                ["Programs", "Opencode", "Opencode.exe"],
                ["Programs", "OpenCode", "OpenCode.exe"],
                ["Programs", "opencode", "Opencode.exe"],
                ["Programs", "opencode", "opencode.exe"],
                ["Opencode", "Opencode.exe"],
                ["OpenCode", "OpenCode.exe"],
                ["opencode", "opencode.exe"],
            ] {
                let mut candidate = base.clone();
                for segment in relative {
                    candidate = candidate.join(segment);
                }
                push_unique_path(&mut candidates, candidate);
            }
        }

        return candidates.into_iter().find(|path| path.is_file());
    }

    #[allow(unreachable_code)]
    None
}

fn detect_opencode_auth_paths() -> Vec<PathBuf> {
    if let Some(custom) = env::var_os("OPENCODE_AUTH_PATH").map(PathBuf::from) {
        return vec![custom];
    }

    let candidates = build_opencode_auth_candidates();
    let mut existing = Vec::<PathBuf>::new();
    for path in &candidates {
        if path.exists() {
            push_unique_path(&mut existing, path.clone());
        }
    }
    if !existing.is_empty() {
        return existing;
    }

    candidates
        .into_iter()
        .next()
        .map(|path| vec![path])
        .unwrap_or_default()
}

fn build_opencode_auth_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::<PathBuf>::new();

    // 优先读取配置目录，兼容 Windows 用户常见路径 `~/.config/opencode/auth.json`。
    if let Some(opencode_config_home) = env::var_os("OPENCODE_CONFIG_HOME").map(PathBuf::from) {
        push_unique_path(&mut candidates, opencode_config_home.join("auth.json"));
    }
    if let Some(xdg_config_home) = env::var_os("XDG_CONFIG_HOME").map(PathBuf::from) {
        push_unique_path(
            &mut candidates,
            xdg_config_home.join("opencode").join("auth.json"),
        );
    }

    if let Some(home) = dirs::home_dir() {
        push_unique_path(
            &mut candidates,
            home.join(".config").join("opencode").join("auth.json"),
        );
    }

    #[cfg(windows)]
    {
        if let Some(app_data) = env::var_os("APPDATA").map(PathBuf::from) {
            push_unique_path(&mut candidates, app_data.join("opencode").join("auth.json"));
        }
        if let Some(local_app_data) = env::var_os("LOCALAPPDATA").map(PathBuf::from) {
            push_unique_path(
                &mut candidates,
                local_app_data.join("opencode").join("auth.json"),
            );
        }
    }

    if let Some(xdg_data_home) = env::var_os("XDG_DATA_HOME").map(PathBuf::from) {
        push_unique_path(
            &mut candidates,
            xdg_data_home.join("opencode").join("auth.json"),
        );
    }

    if let Some(home) = dirs::home_dir() {
        push_unique_path(
            &mut candidates,
            home.join(".local")
                .join("share")
                .join("opencode")
                .join("auth.json"),
        );
        push_unique_path(
            &mut candidates,
            home.join("Library")
                .join("Application Support")
                .join("opencode")
                .join("auth.json"),
        );
        push_unique_path(&mut candidates, home.join(".opencode").join("auth.json"));
    }

    candidates
}

fn push_unique_path(paths: &mut Vec<PathBuf>, candidate: PathBuf) {
    if !paths.iter().any(|existing| existing == &candidate) {
        paths.push(candidate);
    }
}

fn request_opencode_desktop_quit() {
    #[cfg(target_os = "macos")]
    {
        let script = format!(
            r#"tell application id "{}" to quit"#,
            OPENCODE_DESKTOP_MAC_BUNDLE_ID
        );
        let _ = Command::new("osascript").args(["-e", &script]).status();
    }
}

#[cfg(target_os = "macos")]
fn list_running_opencode_desktop_pids() -> Vec<u32> {
    let mut pids = Vec::<u32>::new();

    for name in OPENCODE_DESKTOP_MAC_PROCESS_NAMES {
        let Ok(output) = Command::new("pgrep").args(["-x", name]).output() else {
            continue;
        };
        if !output.status.success() {
            continue;
        }

        for line in String::from_utf8_lossy(&output.stdout).lines() {
            let Ok(pid) = line.trim().parse::<u32>() else {
                continue;
            };
            if !pids.contains(&pid) {
                pids.push(pid);
            }
        }
    }

    pids
}

#[cfg(target_os = "macos")]
fn wait_for_opencode_desktop_exit() -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_millis(OPENCODE_DESKTOP_RESTART_TIMEOUT_MS);

    loop {
        if list_running_opencode_desktop_pids().is_empty() {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err("等待 Opencode 桌面端退出超时".to_string());
        }
        thread::sleep(Duration::from_millis(OPENCODE_DESKTOP_RESTART_POLL_MS));
    }
}

#[cfg(target_os = "macos")]
fn wait_for_opencode_desktop_launch(previous_pids: &[u32]) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_millis(OPENCODE_DESKTOP_RESTART_TIMEOUT_MS);

    loop {
        let current_pids = list_running_opencode_desktop_pids();
        let launched = if previous_pids.is_empty() {
            !current_pids.is_empty()
        } else {
            current_pids.iter().any(|pid| !previous_pids.contains(pid))
        };

        if launched {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err("等待 Opencode 桌面端重新启动超时".to_string());
        }
        thread::sleep(Duration::from_millis(OPENCODE_DESKTOP_RESTART_POLL_MS));
    }
}

fn force_kill_opencode_desktop_processes() {
    #[cfg(target_os = "macos")]
    {
        for name in OPENCODE_DESKTOP_MAC_PROCESS_NAMES {
            let _ = Command::new("pkill").args(["-9", "-x", name]).status();
        }
    }

    #[cfg(target_os = "windows")]
    {
        for name in OPENCODE_DESKTOP_WINDOWS_PROCESS_NAMES {
            let _ = Command::new("taskkill")
                .args(["/F", "/IM", name, "/T"])
                .status();
        }
    }
}

fn reopen_opencode_desktop_app(app_path: &Path) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let status = Command::new("open")
            .arg("-na")
            .arg(app_path)
            .status()
            .map_err(|error| format!("重启 opencode 桌面端失败: {error}"))?;
        if !status.success() {
            return Err("opencode 桌面端重启失败".to_string());
        }
        return Ok(());
    }

    #[cfg(target_os = "windows")]
    {
        Command::new(app_path)
            .spawn()
            .map_err(|error| format!("重启 opencode 桌面端失败: {error}"))?;
        return Ok(());
    }

    #[allow(unreachable_code)]
    {
        let _ = app_path;
        Err("当前平台暂不支持重启 opencode 桌面端".to_string())
    }
}

fn read_or_init_json_object(path: &Path) -> Result<Map<String, Value>, String> {
    if !path.exists() {
        return Ok(Map::new());
    }

    let raw = fs::read_to_string(path)
        .map_err(|e| format!("读取 opencode auth.json 失败 {}: {e}", path.display()))?;
    let parsed: Value =
        serde_json::from_str(&raw).map_err(|e| format!("opencode auth.json 格式无效: {e}"))?;
    Ok(parsed.as_object().cloned().unwrap_or_default())
}

fn write_json_object(path: &Path, value: &Map<String, Value>) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("无法解析 opencode auth 目录 {}", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|e| format!("创建 opencode auth 目录失败 {}: {e}", parent.display()))?;

    let content = serde_json::to_string_pretty(&Value::Object(value.clone()))
        .map_err(|e| format!("序列化 opencode auth.json 失败: {e}"))?;
    fs::write(path, content)
        .map_err(|e| format!("写入 opencode auth.json 失败 {}: {e}", path.display()))?;
    set_private_permissions(path);
    Ok(())
}

fn now_unix_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or_default()
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
