use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
#[cfg(target_os = "windows")]
use std::thread;
#[cfg(target_os = "windows")]
use std::time::{Duration, Instant};

use crate::utils::new_background_command;
#[cfg(target_os = "windows")]
use crate::utils::new_resolved_command;
#[cfg(target_os = "windows")]
use windows::core::HSTRING;
#[cfg(target_os = "windows")]
use windows::Win32::Foundation::RPC_E_CHANGED_MODE;
#[cfg(target_os = "windows")]
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_LOCAL_SERVER, COINIT_APARTMENTTHREADED,
};
#[cfg(target_os = "windows")]
use windows::Win32::UI::Shell::{
    ApplicationActivationManager, IApplicationActivationManager, AO_NONE,
};

const INVALID_CONFIGURED_CODEX_PATH_MESSAGE: &str =
    "设置的 Codex 启动路径无效。请填写 Codex.exe 或 codex/codex.exe 的完整路径，或填写包含它们的安装目录。";
#[cfg(target_os = "windows")]
const WINDOWS_STORE_LAUNCH_TIMEOUT_MS: u64 = 8_000;
#[cfg(target_os = "windows")]
const WINDOWS_STORE_LAUNCH_POLL_MS: u64 = 250;

/// 构造可直接启动 Codex CLI 的命令。
///
/// 重点处理 GUI 进程 PATH 不完整的问题：
/// 先定位真实可执行路径，再把其父目录注入子进程 PATH。
pub(crate) fn new_codex_command(configured_path: Option<&str>) -> Result<Command, String> {
    let normalized_configured_path = normalize_configured_path(configured_path);
    let codex_path = find_configured_codex_cli_path(normalized_configured_path.as_deref())
        .or_else(find_codex_cli_path)
        .ok_or_else(|| {
            if normalized_configured_path.is_some() {
                INVALID_CONFIGURED_CODEX_PATH_MESSAGE.to_string()
            } else {
                "未找到 codex 可执行文件。请先安装 Codex CLI，或将其所在目录加入系统 PATH。"
                    .to_string()
            }
        })?;

    let mut cmd = new_background_command(&codex_path);

    if let Some(parent) = codex_path.parent() {
        let path_entries = if let Some(current_path) = env::var_os("PATH") {
            std::iter::once(parent.to_path_buf())
                .chain(env::split_paths(&current_path))
                .collect::<Vec<_>>()
        } else {
            vec![parent.to_path_buf()]
        };
        let merged = env::join_paths(path_entries).map_err(|e| format!("设置 PATH 失败: {e}"))?;
        cmd.env("PATH", merged);
    }

    Ok(cmd)
}

pub(crate) fn validate_configured_codex_path(configured_path: Option<&str>) -> Result<(), String> {
    let normalized = normalize_configured_path(configured_path);
    let Some(path) = normalized.as_deref() else {
        return Ok(());
    };

    #[cfg(target_os = "windows")]
    if is_windows_store_codex_path(path) {
        return if has_windows_store_codex_app() {
            Ok(())
        } else {
            Err(INVALID_CONFIGURED_CODEX_PATH_MESSAGE.to_string())
        };
    }

    if find_configured_codex_app_path_from_path(Some(path)).is_some()
        || find_configured_codex_cli_path(Some(path)).is_some()
        || is_macos_app_bundle(path)
    {
        Ok(())
    } else {
        Err(INVALID_CONFIGURED_CODEX_PATH_MESSAGE.to_string())
    }
}

pub(crate) fn find_configured_codex_app_path(configured_path: Option<&str>) -> Option<PathBuf> {
    let normalized = normalize_configured_path(configured_path)?;

    find_configured_codex_app_path_from_path(Some(&normalized))
}

#[cfg(target_os = "windows")]
pub(crate) fn is_windows_store_codex_path(path: &Path) -> bool {
    let normalized = path
        .to_string_lossy()
        .replace('/', "\\")
        .to_ascii_lowercase();
    normalized.contains("\\windowsapps\\openai.codex_")
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn is_windows_store_codex_path(_path: &Path) -> bool {
    false
}

#[cfg(target_os = "windows")]
pub(crate) fn has_windows_store_codex_app() -> bool {
    find_windows_codex_store_app_id().is_some()
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn has_windows_store_codex_app() -> bool {
    false
}

#[cfg(target_os = "windows")]
pub(crate) fn launch_windows_store_codex() -> Result<(), String> {
    let app_id = find_windows_codex_store_app_id()
        .ok_or_else(|| "未找到微软商店版 Codex 的启动标识（AUMID）。".to_string())?;
    let baseline_pids = list_running_windows_codex_process_ids();
    let process_id = activate_windows_store_codex_by_aumid(&app_id)?;
    if wait_for_windows_store_codex_process(process_id, &baseline_pids) {
        Ok(())
    } else {
        Err(format!(
            "微软商店版 Codex 激活后未检测到进程启动（{WINDOWS_STORE_LAUNCH_TIMEOUT_MS} ms）。"
        ))
    }
}

pub(crate) fn find_codex_app_path() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        find_windows_codex_app_path()
    }

    #[cfg(not(target_os = "windows"))]
    {
        let mut candidates = vec![
            PathBuf::from("/Applications/Codex.app"),
            PathBuf::from("/Applications/Codex Desktop.app"),
        ];

        if let Some(home) = dirs::home_dir() {
            candidates.push(home.join("Applications").join("Codex.app"));
            candidates.push(home.join("Applications").join("Codex Desktop.app"));
        }

        if let Some(found) = candidates.into_iter().find(|path| path.exists()) {
            return Some(found);
        }

        let spotlight_queries = [
            "kMDItemFSName == 'Codex.app'",
            "kMDItemCFBundleIdentifier == 'com.openai.codex'",
        ];

        for query in spotlight_queries {
            if let Some(path) = first_spotlight_match(query) {
                return Some(path);
            }
        }

        None
    }
}

fn find_codex_cli_path() -> Option<PathBuf> {
    let mut candidates = codex_cli_candidates();
    append_nvm_codex_candidates(&mut candidates);
    append_macos_app_bundle_codex_candidates(&mut candidates);

    let mut seen = HashSet::new();
    for candidate in candidates {
        if !seen.insert(candidate.clone()) {
            continue;
        }
        if is_executable_file(&candidate) {
            return Some(candidate);
        }
    }

    None
}

fn find_configured_codex_cli_path(configured_path: Option<&Path>) -> Option<PathBuf> {
    let configured_path = configured_path?;
    let mut candidates = Vec::new();
    append_configured_codex_candidates(&mut candidates, configured_path);

    let mut seen = HashSet::new();
    for candidate in candidates {
        if !seen.insert(candidate.clone()) {
            continue;
        }
        if is_executable_file(&candidate) {
            return Some(candidate);
        }
    }

    None
}

fn find_configured_codex_app_path_from_path(configured_path: Option<&Path>) -> Option<PathBuf> {
    let configured_path = configured_path?;

    #[cfg(target_os = "macos")]
    {
        if is_macos_app_bundle(configured_path) {
            return Some(configured_path.to_path_buf());
        }
    }

    #[cfg(target_os = "windows")]
    {
        if is_windows_store_codex_path(configured_path) {
            return if has_windows_store_codex_app() {
                Some(configured_path.to_path_buf())
            } else {
                None
            };
        }

        if configured_path.is_file() && is_windows_codex_app_file(configured_path) {
            return Some(configured_path.to_path_buf());
        }

        if configured_path.is_dir() {
            let mut candidates = Vec::new();
            append_windows_codex_app_candidates_from_dir(&mut candidates, configured_path);
            append_windows_codex_app_candidates_from_dir(
                &mut candidates,
                &configured_path.join("current"),
            );
            append_windows_codex_app_candidates_from_dir(
                &mut candidates,
                &configured_path.join("app"),
            );
            append_windows_codex_app_candidates_from_dir(
                &mut candidates,
                &configured_path.join("Application"),
            );
            return first_executable_candidate(candidates);
        }
    }

    None
}

fn codex_cli_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Some(path_os) = env::var_os("PATH") {
        for dir in env::split_paths(&path_os) {
            push_codex_candidates_from_dir(&mut candidates, &dir);
        }
    }

    #[cfg(target_os = "macos")]
    {
        for dir in [
            PathBuf::from("/opt/homebrew/bin"),
            PathBuf::from("/usr/local/bin"),
            PathBuf::from("/usr/bin"),
        ] {
            push_codex_candidates_from_dir(&mut candidates, &dir);
        }
    }

    if let Some(home) = dirs::home_dir() {
        for dir in [
            home.join(".local").join("bin"),
            home.join(".npm-global").join("bin"),
            home.join(".volta").join("bin"),
            home.join(".asdf").join("shims"),
            home.join(".pnpm"),
            home.join("Library").join("pnpm"),
            home.join("bin"),
            home.join("AppData")
                .join("Local")
                .join("Microsoft")
                .join("WindowsApps"),
            home.join("AppData")
                .join("Local")
                .join("Microsoft")
                .join("WinGet")
                .join("Links"),
        ] {
            push_codex_candidates_from_dir(&mut candidates, &dir);
        }
    }

    candidates
}

#[cfg(target_os = "windows")]
fn find_windows_codex_app_path() -> Option<PathBuf> {
    let mut candidates = Vec::new();

    if let Some(local_app_data) = env::var_os("LOCALAPPDATA").map(PathBuf::from) {
        append_windows_codex_app_candidates_from_dir(
            &mut candidates,
            &local_app_data.join("Microsoft").join("WindowsApps"),
        );
        append_windows_codex_app_candidates_from_dir(
            &mut candidates,
            &local_app_data.join("Programs").join("Codex"),
        );
        append_windows_codex_app_candidates_from_dir(
            &mut candidates,
            &local_app_data.join("Programs").join("OpenAI Codex"),
        );
    }

    if let Some(home) = dirs::home_dir() {
        append_windows_codex_app_candidates_from_dir(
            &mut candidates,
            &home
                .join("AppData")
                .join("Local")
                .join("Microsoft")
                .join("WindowsApps"),
        );
    }

    append_windows_store_package_candidates(&mut candidates);
    append_where_matches(&mut candidates, &["Codex.exe", "Codex Desktop.exe"]);

    first_executable_candidate(candidates)
}

fn append_nvm_codex_candidates(candidates: &mut Vec<PathBuf>) {
    let Some(home) = dirs::home_dir() else {
        return;
    };
    let nvm_versions_dir = home.join(".nvm").join("versions").join("node");
    let Ok(entries) = fs::read_dir(&nvm_versions_dir) else {
        return;
    };

    let mut version_dirs = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect::<Vec<_>>();
    version_dirs.sort();
    version_dirs.reverse();

    for version_dir in version_dirs {
        push_codex_candidates_from_dir(candidates, &version_dir.join("bin"));
    }
}

fn append_configured_codex_candidates(candidates: &mut Vec<PathBuf>, configured_path: &Path) {
    if configured_path.is_file() {
        if is_codex_cli_file(configured_path) {
            candidates.push(configured_path.to_path_buf());
        }
        return;
    }

    let mut search_dirs = vec![configured_path.to_path_buf()];

    if configured_path.is_dir() {
        search_dirs.push(configured_path.join("bin"));
        search_dirs.push(configured_path.join("resources"));
        search_dirs.push(configured_path.join("resources").join("bin"));
    }

    #[cfg(target_os = "macos")]
    if is_macos_app_bundle(configured_path) {
        candidates.push(
            configured_path
                .join("Contents")
                .join("Resources")
                .join("codex"),
        );
    }

    for dir in search_dirs {
        push_codex_candidates_from_dir(candidates, &dir);
    }
}

#[cfg(target_os = "macos")]
fn append_macos_app_bundle_codex_candidates(candidates: &mut Vec<PathBuf>) {
    let mut app_paths = vec![
        PathBuf::from("/Applications/Codex.app"),
        PathBuf::from("/Applications/Codex Desktop.app"),
    ];

    if let Some(home) = dirs::home_dir() {
        app_paths.push(home.join("Applications").join("Codex.app"));
        app_paths.push(home.join("Applications").join("Codex Desktop.app"));
    }

    if let Some(found) = find_codex_app_path() {
        app_paths.push(found);
    }

    for app_path in app_paths {
        candidates.push(app_path.join("Contents").join("Resources").join("codex"));
    }
}

#[cfg(not(target_os = "macos"))]
fn append_macos_app_bundle_codex_candidates(_candidates: &mut Vec<PathBuf>) {}

#[cfg(target_os = "windows")]
fn append_windows_store_package_candidates(candidates: &mut Vec<PathBuf>) {
    for root in [
        env::var_os("ProgramFiles").map(PathBuf::from),
        env::var_os("ProgramW6432").map(PathBuf::from),
        env::var_os("ProgramFiles(x86)").map(PathBuf::from),
    ]
    .into_iter()
    .flatten()
    {
        let windows_apps = root.join("WindowsApps");
        let Ok(entries) = fs::read_dir(&windows_apps) else {
            continue;
        };

        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let package_name = entry.file_name().to_string_lossy().to_ascii_lowercase();
            if !package_name.contains("codex") {
                continue;
            }

            append_windows_codex_app_candidates_from_dir(candidates, &path);
            append_windows_codex_app_candidates_from_dir(candidates, &path.join("app"));
            append_windows_codex_app_candidates_from_dir(candidates, &path.join("Application"));
        }
    }
}

#[cfg(target_os = "windows")]
fn append_where_matches(candidates: &mut Vec<PathBuf>, commands: &[&str]) {
    for command in commands {
        let Ok(output) = Command::new("where.exe").arg(command).output() else {
            continue;
        };
        if !output.status.success() {
            continue;
        }

        for line in String::from_utf8_lossy(&output.stdout).lines() {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                candidates.push(PathBuf::from(trimmed));
            }
        }
    }
}

#[cfg(target_os = "windows")]
fn find_windows_codex_store_app_id() -> Option<String> {
    let script = r#"
$ErrorActionPreference = 'Stop'
$pattern = '^OpenAI\.Codex_[^!]+![^!]+$'
$candidates = New-Object System.Collections.Generic.List[string]

@(
  Get-StartApps |
    Where-Object { $_.AppID -and $_.AppID -match $pattern } |
    Select-Object -ExpandProperty AppID -Unique
) | ForEach-Object {
  if ($_ -and -not $candidates.Contains($_)) {
    [void]$candidates.Add($_)
  }
}

try {
  @(
    (New-Object -ComObject Shell.Application).
      NameSpace('shell:::{4234d49b-0245-4df3-b780-3893943456e1}').
      Items() |
      Select-Object @{n='AUMID';e={$_.Path}} |
      Where-Object { $_.AUMID -and $_.AUMID -match $pattern } |
      Select-Object -ExpandProperty AUMID -Unique
  ) | ForEach-Object {
    if ($_ -and -not $candidates.Contains($_)) {
      [void]$candidates.Add($_)
    }
  }
} catch {}

try {
  $pkg = Get-AppxPackage -Name 'OpenAI.Codex' | Select-Object -First 1
  if ($null -ne $pkg) {
    $manifest = $pkg | Get-AppxPackageManifest
    foreach ($app in @($manifest.Package.Applications.Application)) {
      if ($null -ne $app -and $app.Id) {
        $aumid = '{0}!{1}' -f $pkg.PackageFamilyName, $app.Id
        if ($aumid -match $pattern -and -not $candidates.Contains($aumid)) {
          [void]$candidates.Add($aumid)
        }
      }
    }
  }
} catch {}

$match = $candidates |
  Sort-Object @{Expression = { $_ -notmatch '!App$' }}, @{Expression = { $_ }} |
  Select-Object -First 1
if ($match) {
  $match
  exit 0
}
"#;

    let output = new_resolved_command("powershell")
        .arg("-NoProfile")
        .arg("-NonInteractive")
        .arg("-ExecutionPolicy")
        .arg("Bypass")
        .arg("-Command")
        .arg(script)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(str::to_string)
}

#[cfg(target_os = "windows")]
fn activate_windows_store_codex_by_aumid(app_id: &str) -> Result<u32, String> {
    let _com_guard = WindowsComGuard::initialize()?;
    let activation_manager: IApplicationActivationManager =
        unsafe { CoCreateInstance(&ApplicationActivationManager, None, CLSCTX_LOCAL_SERVER) }
            .map_err(|error| format!("创建微软商店激活管理器失败: {error}"))?;

    let app_id = HSTRING::from(app_id);
    let arguments = HSTRING::new();
    unsafe { activation_manager.ActivateApplication(&app_id, &arguments, AO_NONE) }
        .map_err(|error| format!("通过 AUMID 激活 Codex 失败: {error}"))
}

#[cfg(target_os = "windows")]
fn wait_for_windows_store_codex_process(expected_pid: u32, baseline_pids: &[u32]) -> bool {
    let baseline = baseline_pids.iter().copied().collect::<HashSet<_>>();
    let deadline = Instant::now() + Duration::from_millis(WINDOWS_STORE_LAUNCH_TIMEOUT_MS);
    loop {
        if expected_pid > 0 && is_windows_process_running(expected_pid) {
            return true;
        }

        let current_pids = list_running_windows_codex_process_ids();
        if current_pids.iter().any(|pid| !baseline.contains(pid)) {
            return true;
        }

        if Instant::now() >= deadline {
            return false;
        }

        thread::sleep(Duration::from_millis(WINDOWS_STORE_LAUNCH_POLL_MS));
    }
}

#[cfg(target_os = "windows")]
fn list_running_windows_codex_process_ids() -> Vec<u32> {
    let mut pids = Vec::new();
    for image_name in ["Codex.exe", "Codex Desktop.exe"] {
        let filter = format!("IMAGENAME eq {image_name}");
        let Ok(output) = new_resolved_command("tasklist")
            .args(["/FO", "CSV", "/NH", "/FI", &filter])
            .output()
        else {
            continue;
        };

        if !output.status.success() {
            continue;
        }

        for line in String::from_utf8_lossy(&output.stdout).lines() {
            if let Some(pid) = parse_tasklist_csv_pid(line) {
                pids.push(pid);
            }
        }
    }

    pids.sort_unstable();
    pids.dedup();
    pids
}

#[cfg(target_os = "windows")]
fn is_windows_process_running(pid: u32) -> bool {
    let filter = format!("PID eq {pid}");
    let Ok(output) = new_resolved_command("tasklist")
        .args(["/FO", "CSV", "/NH", "/FI", &filter])
        .output()
    else {
        return false;
    };

    if !output.status.success() {
        return false;
    }

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .any(|line| parse_tasklist_csv_pid(line).is_some())
}

#[cfg(target_os = "windows")]
fn parse_tasklist_csv_pid(line: &str) -> Option<u32> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with("INFO:") || !trimmed.starts_with('"') {
        return None;
    }

    let mut parts = trimmed.trim_matches('"').split("\",\"");
    let _image_name = parts.next()?;
    parts.next()?.parse().ok()
}

#[cfg(target_os = "windows")]
struct WindowsComGuard {
    should_uninitialize: bool,
}

#[cfg(target_os = "windows")]
impl WindowsComGuard {
    fn initialize() -> Result<Self, String> {
        let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        if hr == RPC_E_CHANGED_MODE {
            return Ok(Self {
                should_uninitialize: false,
            });
        }
        if hr.is_ok() {
            return Ok(Self {
                should_uninitialize: true,
            });
        }
        Err(format!("初始化 Windows COM 失败: {hr}"))
    }
}

#[cfg(target_os = "windows")]
impl Drop for WindowsComGuard {
    fn drop(&mut self) {
        if self.should_uninitialize {
            unsafe {
                CoUninitialize();
            }
        }
    }
}

#[cfg(target_os = "windows")]
fn append_windows_codex_app_candidates_from_dir(candidates: &mut Vec<PathBuf>, dir: &Path) {
    for name in ["Codex.exe", "Codex Desktop.exe"] {
        candidates.push(dir.join(name));
    }
}

fn normalize_configured_path(configured_path: Option<&str>) -> Option<PathBuf> {
    let raw = configured_path?.trim();
    if raw.is_empty() {
        return None;
    }

    let unquoted = raw
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .or_else(|| {
            raw.strip_prefix('\'')
                .and_then(|value| value.strip_suffix('\''))
        })
        .unwrap_or(raw)
        .trim();

    if unquoted.is_empty() {
        None
    } else {
        Some(PathBuf::from(unquoted))
    }
}

fn push_codex_candidates_from_dir(candidates: &mut Vec<PathBuf>, dir: &Path) {
    #[cfg(windows)]
    let names = ["codex.exe", "codex.cmd", "codex.bat"];
    #[cfg(not(windows))]
    let names = ["codex"];

    for name in names {
        candidates.push(dir.join(name));
    }
}

#[cfg(target_os = "windows")]
fn first_executable_candidate(candidates: Vec<PathBuf>) -> Option<PathBuf> {
    let mut seen = HashSet::new();
    for candidate in candidates {
        if !seen.insert(candidate.clone()) {
            continue;
        }
        if is_executable_file(&candidate) && is_windows_codex_app_file(&candidate) {
            return Some(candidate);
        }
    }
    None
}

fn is_codex_cli_file(path: &Path) -> bool {
    let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
        return false;
    };

    #[cfg(windows)]
    {
        matches_ignore_ascii_case(file_name, &["codex.exe", "codex.cmd", "codex.bat"])
    }

    #[cfg(not(windows))]
    {
        file_name == "codex"
    }
}

#[cfg(target_os = "windows")]
fn is_windows_codex_app_file(path: &Path) -> bool {
    let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
        return false;
    };

    if !matches_ignore_ascii_case(file_name, &["codex.exe", "codex desktop.exe"]) {
        return false;
    }

    let normalized_path = path.to_string_lossy().to_ascii_lowercase();
    if normalized_path.contains("\\winget\\links\\")
        || normalized_path.contains("\\shims\\")
        || normalized_path.contains("\\resources\\")
        || normalized_path.contains("\\resources\\bin\\")
    {
        return false;
    }

    let parent_name = path
        .parent()
        .and_then(|parent| parent.file_name())
        .and_then(|value| value.to_str())
        .unwrap_or_default();

    !matches_ignore_ascii_case(parent_name, &["bin"])
}

#[cfg(windows)]
fn matches_ignore_ascii_case(value: &str, candidates: &[&str]) -> bool {
    candidates
        .iter()
        .any(|candidate| value.eq_ignore_ascii_case(candidate))
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

fn is_macos_app_bundle(path: &Path) -> bool {
    #[cfg(target_os = "macos")]
    {
        path.is_dir()
            && path
                .extension()
                .and_then(|value| value.to_str())
                .map(|value| value.eq_ignore_ascii_case("app"))
                .unwrap_or(false)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = path;
        false
    }
}

#[cfg(not(target_os = "windows"))]
fn first_spotlight_match(query: &str) -> Option<PathBuf> {
    let output = Command::new("mdfind").arg(query).output().ok()?;
    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(PathBuf::from)
        .find(|path| path.exists())
}
