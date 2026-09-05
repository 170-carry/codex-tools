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
use windows::core::{HSTRING, PCWSTR};
#[cfg(target_os = "windows")]
use windows::Win32::Foundation::RPC_E_CHANGED_MODE;
#[cfg(target_os = "windows")]
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_LOCAL_SERVER, COINIT_APARTMENTTHREADED,
};
#[cfg(target_os = "windows")]
use windows::Win32::UI::Shell::{
    ApplicationActivationManager, IApplicationActivationManager, ShellExecuteW, AO_NONE,
};
#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

const INVALID_CONFIGURED_CODEX_PATH_MESSAGE: &str =
    "设置的 Codex 启动路径无效。请填写 Codex 桌面程序（Codex.exe 或 OpenAI.Codex 包内的 ChatGPT.exe）、codex/codex.exe 的完整路径，或包含它们的安装目录。";
#[cfg(target_os = "windows")]
const WINDOWS_STORE_LAUNCH_TIMEOUT_MS: u64 = 8_000;
#[cfg(target_os = "windows")]
const WINDOWS_STORE_LAUNCH_POLL_MS: u64 = 250;
#[cfg(target_os = "macos")]
const MACOS_CODEX_APP_NAMES: [&str; 3] = ["ChatGPT.app", "Codex.app", "Codex Desktop.app"];

/// 构造可直接启动 Codex CLI 的命令。
///
/// 重点处理 GUI 进程 PATH 不完整的问题：
/// 先定位真实可执行路径，再把其父目录注入子进程 PATH。
pub(crate) fn new_codex_command(configured_path: Option<&str>) -> Result<Command, String> {
    new_codex_command_with_builder(configured_path, |path| new_background_command(path))
}

#[cfg(target_os = "windows")]
#[derive(Debug)]
enum WindowsDesktopTarget {
    Store(WindowsStoreCodexTarget),
    Executable(PathBuf),
}

#[cfg(target_os = "windows")]
impl WindowsDesktopTarget {
    fn executable(&self) -> &Path {
        match self {
            Self::Store(target) => &target.executable,
            Self::Executable(path) => path,
        }
    }
}

/// Resolve once before stopping the desktop or replacing its auth snapshot.
/// Keep the CLI failure alongside the desktop failure rather than losing the
/// original cause when a fallback is unavailable.
#[cfg(target_os = "windows")]
#[derive(Debug)]
pub(crate) struct WindowsCodexLaunchPlan {
    desktop: Option<WindowsDesktopTarget>,
    desktop_to_stop: Option<PathBuf>,
    fallback_cli: Result<Command, String>,
    discovery_error: Option<String>,
    as_admin: bool,
}

#[cfg(target_os = "windows")]
pub(crate) fn prepare_windows_codex_launch(
    configured_path: Option<&str>,
    as_admin: bool,
) -> Result<WindowsCodexLaunchPlan, String> {
    let (store_target, mut discovery_error) = match find_windows_codex_store_target() {
        Ok(target) => (target, None),
        Err(error) => (None, Some(error)),
    };
    let configured = normalize_configured_path(configured_path);
    let configured_desktop = configured
        .as_deref()
        .filter(|path| !is_windows_store_codex_path(path))
        .and_then(|path| find_configured_codex_app_path_from_path(Some(path)))
        .map(WindowsDesktopTarget::Executable);
    let mut desktop = configured_desktop.or_else(|| store_target.map(WindowsDesktopTarget::Store));
    if desktop.is_none() {
        // Legacy unpackaged Codex remains supported. Do not treat a stale Store
        // directory as a standalone executable when package discovery failed.
        desktop = find_windows_codex_app_path()
            .filter(|path| !is_windows_store_codex_path(path))
            .map(WindowsDesktopTarget::Executable);
    }
    let desktop_to_stop = desktop
        .as_ref()
        .map(|target| target.executable().to_path_buf());
    if as_admin && matches!(desktop, Some(WindowsDesktopTarget::Store(_))) {
        discovery_error = Some("微软商店版 ChatGPT/Codex 不支持以管理员身份启动，请关闭管理员启动或指定可用的桌面版/CLI。".to_string());
        desktop = None;
    }
    let plan = WindowsCodexLaunchPlan {
        desktop,
        desktop_to_stop,
        fallback_cli: new_codex_command(configured_path),
        discovery_error,
        as_admin,
    };
    plan.validate()?;
    Ok(plan)
}

#[cfg(target_os = "windows")]
fn windows_launch_failure(desktop_error: Option<&str>, cli_error: &str) -> String {
    match desktop_error {
        Some(error) => format!("ChatGPT/Codex 桌面启动失败: {error}；CLI 回退失败: {cli_error}"),
        None => format!("未找到可用的 ChatGPT/Codex 桌面启动目标；CLI 回退失败: {cli_error}"),
    }
}

#[cfg(target_os = "windows")]
impl WindowsCodexLaunchPlan {
    fn validate(&self) -> Result<(), String> {
        if self.desktop.is_none() {
            if let Err(error) = &self.fallback_cli {
                return Err(format!(
                    "切换前检查失败，未停止应用或更改当前账号：{}",
                    windows_launch_failure(self.discovery_error.as_deref(), error)
                ));
            }
        }
        Ok(())
    }

    pub(crate) fn stop_desktop(&self) -> Result<(), String> {
        let Some(executable) = &self.desktop_to_stop else {
            return Ok(());
        };
        let mut system = sysinfo::System::new_all();
        let current_pid = sysinfo::get_current_pid().map_err(|error| error.to_string())?;
        let current = system
            .process(current_pid)
            .ok_or_else(|| "无法识别当前进程，未停止应用。".to_string())?;
        let user = current
            .user_id()
            .cloned()
            .ok_or_else(|| "无法识别当前用户，未停止应用。".to_string())?;
        let session = current.session_id();
        let roots = verified_windows_desktop_process_ids(&system, &[executable.clone()])?;
        let parents = system
            .processes()
            .iter()
            .filter(|(_, process)| {
                process.user_id() == Some(&user) && process.session_id() == session
            })
            .map(|(pid, process)| (*pid, process.parent()))
            .collect::<Vec<_>>();
        let targets = windows_switch_stop_targets(roots, &parents, current_pid);
        let mut remaining = targets
            .into_iter()
            .filter_map(|pid| {
                system
                    .process(pid)
                    .map(|process| (pid, process.start_time()))
            })
            .collect::<Vec<_>>();
        log::info!(
            "CODEX_DESKTOP_STOP executable={} pids={remaining:?}",
            executable.display()
        );
        let deadline = Instant::now() + Duration::from_secs(3);
        while !remaining.is_empty() {
            for (pid, start_time) in &remaining {
                if let Some(process) = system.process(*pid) {
                    // PID reuse must not turn a validated target into an unrelated kill.
                    if process.start_time() == *start_time && process.user_id() == Some(&user) {
                        process.kill();
                    }
                }
            }
            thread::sleep(Duration::from_millis(50));
            system.refresh_processes();
            remaining.retain(|(pid, start_time)| {
                system
                    .process(*pid)
                    .is_some_and(|process| process.start_time() == *start_time)
            });
            if Instant::now() >= deadline && !remaining.is_empty() {
                return Err(format!(
                    "未能结束已验证的 ChatGPT/Codex 进程，未更改当前账号: {remaining:?}"
                ));
            }
        }
        if !verified_windows_desktop_process_ids(&system, &[executable.clone()])?.is_empty() {
            return Err(
                "ChatGPT/Codex 在停止期间重新启动，未更改当前账号，请关闭该桌面后重试。"
                    .to_string(),
            );
        }
        Ok(())
    }

    pub(crate) fn launch(
        mut self,
        workspace: Option<&str>,
    ) -> Result<(Option<String>, bool), String> {
        let mut desktop_error = self.discovery_error;
        if let Some(target) = self.desktop {
            let result = match &target {
                WindowsDesktopTarget::Store(store) => launch_windows_store_target(store),
                WindowsDesktopTarget::Executable(path) => if self.as_admin {
                    let args = workspace
                        .map(|value| vec![value.to_string()])
                        .unwrap_or_default();
                    launch_elevated_process(path, &args)
                } else {
                    let mut command = new_background_command(path);
                    if let Some(workspace) = workspace {
                        command.arg(workspace);
                    }
                    command
                        .spawn()
                        .map(|_| ())
                        .map_err(|error| error.to_string())
                }
                .and_then(|()| {
                    if wait_for_windows_codex_process(path) {
                        Ok(())
                    } else {
                        Err(format!("启动后桌面进程未保持运行: {}", path.display()))
                    }
                }),
            };
            match result {
                Ok(()) => {
                    return Ok((
                        Some(target.executable().to_string_lossy().into_owned()),
                        false,
                    ))
                }
                Err(error) => {
                    log::warn!("CODEX_DESKTOP_LAUNCH failed: {error}");
                    desktop_error = Some(error);
                }
            }
        }
        let command = self
            .fallback_cli
            .as_mut()
            .map_err(|error| windows_launch_failure(desktop_error.as_deref(), error))?;
        let mut args = vec!["app".to_string()];
        if let Some(workspace) = workspace {
            args.push(workspace.to_string());
        }
        let result = if self.as_admin {
            launch_elevated_process(Path::new(command.get_program()), &args)
        } else {
            command
                .args(&args)
                .spawn()
                .map(|_| ())
                .map_err(|error| error.to_string())
        };
        result.map_err(|error| windows_launch_failure(desktop_error.as_deref(), &error))?;
        Ok((None, true))
    }
}

#[cfg(target_os = "windows")]
fn windows_desktop_descendants(
    mut targets: HashSet<sysinfo::Pid>,
    same_user_parents: &[(sysinfo::Pid, Option<sysinfo::Pid>)],
) -> HashSet<sysinfo::Pid> {
    loop {
        let previous = targets.len();
        for (pid, parent) in same_user_parents {
            if parent.is_some_and(|parent| targets.contains(&parent)) {
                targets.insert(*pid);
            }
        }
        if targets.len() == previous {
            return targets;
        }
    }
}

pub(crate) fn new_codex_foreground_command(
    configured_path: Option<&str>,
) -> Result<Command, String> {
    new_codex_command_with_builder(configured_path, |path| Command::new(path))
}

fn new_codex_command_with_builder(
    configured_path: Option<&str>,
    build_command: impl FnOnce(&Path) -> Command,
) -> Result<Command, String> {
    let codex_path = resolve_codex_cli_path(configured_path)?;
    let mut cmd = build_command(&codex_path);

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

#[cfg(target_os = "windows")]
pub(crate) fn launch_codex_command_elevated(
    configured_path: Option<&str>,
    args: &[String],
) -> Result<(), String> {
    let codex_path = resolve_codex_cli_path(configured_path)?;
    launch_elevated_process(&codex_path, args)
}

#[cfg(target_os = "windows")]
pub(crate) fn launch_elevated_process(program: &Path, args: &[String]) -> Result<(), String> {
    let operation = wide_null("runas");
    let file = wide_os_null(program.as_os_str());
    let parameters_text = args
        .iter()
        .map(|arg| quote_windows_arg(arg))
        .collect::<Vec<_>>()
        .join(" ");
    let parameters = wide_null(&parameters_text);
    let directory = program
        .parent()
        .map(|parent| wide_os_null(parent.as_os_str()));

    let parameters_ptr = if parameters_text.is_empty() {
        PCWSTR::null()
    } else {
        PCWSTR(parameters.as_ptr())
    };
    let directory_ptr = directory
        .as_ref()
        .map(|value| PCWSTR(value.as_ptr()))
        .unwrap_or_else(PCWSTR::null);

    let result = unsafe {
        ShellExecuteW(
            None,
            PCWSTR(operation.as_ptr()),
            PCWSTR(file.as_ptr()),
            parameters_ptr,
            directory_ptr,
            SW_SHOWNORMAL,
        )
    };
    let code = result.0 as isize;
    if code <= 32 {
        Err(format!(
            "以管理员身份启动 Codex 失败 {}，ShellExecuteW 返回码 {code}",
            program.display()
        ))
    } else {
        Ok(())
    }
}

fn resolve_codex_cli_path(configured_path: Option<&str>) -> Result<PathBuf, String> {
    let normalized_configured_path = normalize_configured_path(configured_path);
    find_configured_codex_cli_path(normalized_configured_path.as_deref())
        .or_else(find_codex_cli_path)
        .ok_or_else(|| {
            if normalized_configured_path.is_some() {
                INVALID_CONFIGURED_CODEX_PATH_MESSAGE.to_string()
            } else {
                "未找到 codex 可执行文件。请先安装 Codex CLI，或将其所在目录加入系统 PATH。"
                    .to_string()
            }
        })
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
    let target = find_windows_codex_store_target()?
        .ok_or_else(|| "未找到微软商店版 Codex 的启动标识（AUMID）。".to_string())?;
    launch_windows_store_target(&target)
}

pub(crate) fn find_codex_app_path() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        find_windows_codex_app_path()
    }

    #[cfg(target_os = "macos")]
    {
        let home = dirs::home_dir();
        let candidates = macos_codex_app_candidates(home.as_deref());

        if let Some(found) = candidates
            .into_iter()
            .find(|path| is_macos_codex_app_bundle(path))
        {
            return Some(found);
        }

        let spotlight_queries = [
            "kMDItemFSName == 'ChatGPT.app'",
            "kMDItemFSName == 'Codex.app'",
            "kMDItemFSName == 'Codex Desktop.app'",
            "kMDItemCFBundleIdentifier == 'com.openai.codex'",
        ];

        for query in spotlight_queries {
            if let Some(path) = first_spotlight_codex_app_match(query) {
                return Some(path);
            }
        }

        None
    }

    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        None
    }
}

#[cfg(target_os = "macos")]
fn macos_codex_app_candidates(home: Option<&Path>) -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    // 新版 Codex 桌面端使用 ChatGPT.app 名称；旧名称继续作为兼容回退。
    for app_name in MACOS_CODEX_APP_NAMES {
        candidates.push(Path::new("/Applications").join(app_name));
        if let Some(home) = home {
            candidates.push(home.join("Applications").join(app_name));
        }
    }

    candidates
}

#[cfg(target_os = "macos")]
pub(crate) fn is_macos_codex_app_bundle(path: &Path) -> bool {
    if !is_macos_app_bundle(path) {
        return false;
    }

    let Some(app_name) = path.file_name().and_then(|value| value.to_str()) else {
        return false;
    };

    if matches_ignore_ascii_case(app_name, &["Codex.app", "Codex Desktop.app"]) {
        return true;
    }
    if !app_name.eq_ignore_ascii_case("ChatGPT.app") {
        return false;
    }

    // ChatGPT.app 历史上也可能是普通聊天客户端，内置 codex 才能证明它支持当前启动协议。
    is_executable_file(&path.join("Contents").join("Resources").join("codex"))
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
        if is_executable_file(&candidate) && is_codex_cli_file(&candidate) {
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
        if is_executable_file(&candidate) && is_codex_cli_file(&candidate) {
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
    // Resolve the registered package before scanning directories: Store updates
    // can leave old package directories behind, and the desktop was renamed.
    if let Ok(Some(target)) = find_windows_codex_store_target() {
        return Some(target.executable);
    }
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
    let home = dirs::home_dir();
    let mut app_paths = macos_codex_app_candidates(home.as_deref());

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
    find_windows_codex_store_target()
        .ok()
        .flatten()
        .map(|target| target.aumid)
}

#[cfg(target_os = "windows")]
#[derive(Debug, Clone, serde::Deserialize)]
struct WindowsStoreCodexTarget {
    aumid: String,
    executable: PathBuf,
}

#[cfg(target_os = "windows")]
fn find_windows_codex_store_target() -> Result<Option<WindowsStoreCodexTarget>, String> {
    // The displayed name is now ChatGPT, but the registered product identity is
    // still OpenAI.Codex. Never discover the ordinary ChatGPT package by name.
    let script = r#"
$ErrorActionPreference = 'Stop'
$pkg = Get-AppxPackage -Name 'OpenAI.Codex' | Sort-Object Version -Descending | Select-Object -First 1
if ($null -eq $pkg) { exit 0 }
$manifest = $pkg | Get-AppxPackageManifest
foreach ($app in @($manifest.Package.Applications.Application) | Sort-Object { $_.Id -ne 'App' }) {
    $relative = ([string]$app.Executable).Replace('/', '\')
    if ($app.Id -and $relative -match '^app\\(ChatGPT|Codex|Codex Desktop)\.exe$') {
        [pscustomobject]@{
            aumid = '{0}!{1}' -f $pkg.PackageFamilyName, $app.Id
            executable = Join-Path $pkg.InstallLocation $relative
        } | ConvertTo-Json -Compress
        exit 0
    }
}
throw 'OpenAI.Codex is installed but its manifest has no supported desktop executable.'
"#;

    // Store-launched GUIs need not inherit a terminal's PATH. Windows PowerShell
    // is an OS component; resolve its absolute path without editing system PATH.
    let powershell = env::var_os("SystemRoot")
        .map(PathBuf::from)
        .map(|root| {
            root.join("System32")
                .join("WindowsPowerShell")
                .join("v1.0")
                .join("powershell.exe")
        })
        .filter(|path| path.is_file());
    let mut command = powershell
        .as_ref()
        .map(new_background_command)
        .unwrap_or_else(|| new_resolved_command("powershell"));
    let output = command
        .arg("-NoProfile")
        .arg("-NonInteractive")
        .arg("-ExecutionPolicy")
        .arg("Bypass")
        .arg("-Command")
        .arg(script)
        .output()
        .map_err(|error| format!("查询 OpenAI.Codex 安装包失败: {error}"))?;

    if !output.status.success() {
        return Err(format!(
            "查询 OpenAI.Codex 安装包失败: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    if text.trim().is_empty() {
        return Ok(None);
    }
    let target: WindowsStoreCodexTarget = serde_json::from_str(text.trim())
        .map_err(|error| format!("解析 OpenAI.Codex 安装包信息失败: {error}"))?;
    if !target.aumid.starts_with("OpenAI.Codex_")
        || !is_windows_store_codex_path(&target.executable)
        || !is_windows_codex_app_file(&target.executable)
        || !target.executable.is_file()
    {
        return Err(
            "OpenAI.Codex 安装包的启动目标无效，未尝试启动其他 ChatGPT 客户端。".to_string(),
        );
    }
    Ok(Some(target))
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
fn launch_windows_store_target(target: &WindowsStoreCodexTarget) -> Result<(), String> {
    let process_id = activate_windows_store_codex_by_aumid(&target.aumid)?;
    log::info!(
        "CODEX_DESKTOP_LAUNCH aumid={} executable={} activation_pid={process_id}",
        target.aumid,
        target.executable.display()
    );
    if wait_for_windows_codex_process(&target.executable) {
        Ok(())
    } else {
        Err(format!(
            "微软商店版 Codex 激活后未检测到当前用户的桌面进程（AUMID={}，程序={}，激活 PID={process_id}，等待 {WINDOWS_STORE_LAUNCH_TIMEOUT_MS} ms）。",
            target.aumid, target.executable.display()
        ))
    }
}

#[cfg(target_os = "windows")]
fn wait_for_windows_codex_process(executable: &Path) -> bool {
    let deadline = Instant::now() + Duration::from_millis(WINDOWS_STORE_LAUNCH_TIMEOUT_MS);
    let mut confirmed_since = None;
    loop {
        let system = sysinfo::System::new_all();
        let pids = verified_windows_desktop_process_ids(&system, &[executable.to_path_buf()])
            .unwrap_or_default();
        // Activation can return a broker PID. Require the installed GUI path to
        // stay alive instead of accepting any PID or a matching process name.
        if !pids.is_empty() {
            let start = confirmed_since.get_or_insert_with(Instant::now);
            if start.elapsed() >= Duration::from_millis(500) {
                log::info!("CODEX_DESKTOP_LAUNCH verified_pids={pids:?}");
                return true;
            }
        } else {
            confirmed_since = None;
        }

        if Instant::now() >= deadline {
            return false;
        }

        thread::sleep(Duration::from_millis(WINDOWS_STORE_LAUNCH_POLL_MS));
    }
}

#[cfg(target_os = "windows")]
fn verified_windows_desktop_process_ids(
    system: &sysinfo::System,
    executables: &[PathBuf],
) -> Result<HashSet<sysinfo::Pid>, String> {
    let current_pid = sysinfo::get_current_pid().map_err(|error| error.to_string())?;
    let current = system
        .process(current_pid)
        .ok_or_else(|| "无法识别当前进程，未操作桌面进程。".to_string())?;
    let user = current
        .user_id()
        .ok_or_else(|| "无法识别当前用户，未操作桌面进程。".to_string())?;
    Ok(system
        .processes()
        .iter()
        .filter_map(|(pid, process)| {
            (process.user_id() == Some(user)
                && process.session_id() == current.session_id()
                && process.exe().is_some_and(|path| {
                    executables
                        .iter()
                        .any(|expected| windows_paths_equal(path, expected))
                }))
            .then_some(*pid)
        })
        .collect())
}

#[cfg(target_os = "windows")]
fn windows_paths_equal(left: &Path, right: &Path) -> bool {
    left.to_string_lossy()
        .replace('/', "\\")
        .eq_ignore_ascii_case(&right.to_string_lossy().replace('/', "\\"))
}

#[cfg(target_os = "windows")]
struct WindowsComGuard {
    should_uninitialize: bool,
}

#[cfg(target_os = "windows")]
fn windows_switch_stop_targets(
    roots: HashSet<sysinfo::Pid>,
    same_user_parents: &[(sysinfo::Pid, Option<sysinfo::Pid>)],
    current_pid: sysinfo::Pid,
) -> HashSet<sysinfo::Pid> {
    // Tools may itself have been opened from a Codex terminal. Its own process
    // and helpers must survive long enough to apply the profile and relaunch.
    let protected = windows_desktop_descendants(HashSet::from([current_pid]), same_user_parents);
    windows_desktop_descendants(roots, same_user_parents)
        .difference(&protected)
        .copied()
        .collect()
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
    for name in ["ChatGPT.exe", "Codex.exe", "Codex Desktop.exe"] {
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

#[cfg(target_os = "windows")]
fn wide_null(value: &str) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;

    std::ffi::OsStr::new(value)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

#[cfg(target_os = "windows")]
fn wide_os_null(value: &std::ffi::OsStr) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;

    value.encode_wide().chain(std::iter::once(0)).collect()
}

#[cfg(target_os = "windows")]
fn quote_windows_arg(value: &str) -> String {
    if value.is_empty() {
        return "\"\"".to_string();
    }

    if !value
        .chars()
        .any(|item| item.is_whitespace() || item == '"')
    {
        return value.to_string();
    }

    let mut quoted = String::from("\"");
    let mut backslashes = 0usize;
    for item in value.chars() {
        match item {
            '\\' => backslashes += 1,
            '"' => {
                quoted.push_str(&"\\".repeat(backslashes * 2 + 1));
                quoted.push('"');
                backslashes = 0;
            }
            _ => {
                quoted.push_str(&"\\".repeat(backslashes));
                backslashes = 0;
                quoted.push(item);
            }
        }
    }
    quoted.push_str(&"\\".repeat(backslashes * 2));
    quoted.push('"');
    quoted
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
            && !is_windows_codex_app_file(path)
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

    if !matches_ignore_ascii_case(
        file_name,
        &["chatgpt.exe", "codex.exe", "codex desktop.exe"],
    ) {
        return false;
    }

    let normalized_path = path
        .to_string_lossy()
        .replace('/', "\\")
        .to_ascii_lowercase();
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

    if matches_ignore_ascii_case(parent_name, &["bin"]) {
        return false;
    }
    if is_windows_store_codex_path(path) {
        return parent_name.eq_ignore_ascii_case("app");
    }
    // An unrelated ChatGPT client is not a Codex launcher. Unpackaged legacy
    // Codex needs its Electron resources to distinguish it from a bare CLI.
    !file_name.eq_ignore_ascii_case("chatgpt.exe")
        && path
            .parent()
            .is_some_and(|dir| dir.join("resources").join("app.asar").is_file())
}

#[cfg(any(windows, target_os = "macos"))]
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

#[cfg(target_os = "macos")]
fn first_spotlight_codex_app_match(query: &str) -> Option<PathBuf> {
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
        .find(|path| is_macos_codex_app_bundle(path))
}

#[cfg(all(test, target_os = "windows"))]
mod windows_tests {
    use super::*;

    #[test]
    fn registered_chatgpt_desktop_is_not_a_cli_or_regular_chatgpt() {
        let package = Path::new(
            r"C:\Program Files\WindowsApps\OpenAI.Codex_26.901.5280.0_x64__2p2nqsd0c76g0\app",
        );
        assert!(is_windows_codex_app_file(&package.join("ChatGPT.exe")));
        assert!(is_windows_codex_app_file(&package.join("Codex.exe")));
        assert!(!is_codex_cli_file(&package.join("Codex.exe")));
        assert!(is_codex_cli_file(
            &package.join("resources").join("codex.exe")
        ));
        assert!(!is_windows_codex_app_file(
            &package.join("resources").join("codex.exe")
        ));
        assert!(!is_windows_codex_app_file(Path::new(
            r"C:\Users\tester\AppData\Local\OpenAI\Codex\bin\version\codex.exe"
        )));
        assert!(!is_windows_codex_app_file(Path::new(
            r"C:\Program Files\WindowsApps\OpenAI.ChatGPT_1_x64__publisher\app\ChatGPT.exe"
        )));
        assert!(!is_windows_codex_app_file(Path::new(
            r"C:\Tools\ChatGPT.exe"
        )));
        assert!(!is_windows_codex_app_file(Path::new(r"C:\Tools\codex.exe")));
    }

    #[test]
    fn windows_paths_are_case_and_separator_insensitive() {
        assert!(windows_paths_equal(
            Path::new(r"C:\Apps\ChatGPT.exe"),
            Path::new("c:/apps/chatgpt.exe")
        ));
        assert!(!windows_paths_equal(
            Path::new(r"C:\Apps\ChatGPT.exe"),
            Path::new(r"C:\Other\ChatGPT.exe")
        ));
    }

    #[test]
    fn desktop_descendants_leave_unrelated_cli_and_chat_clients_alone() {
        let pid = sysinfo::Pid::from_u32;
        let targets = windows_desktop_descendants(
            HashSet::from([pid(10)]),
            &[
                (pid(10), Some(pid(1))),
                (pid(11), Some(pid(10))),
                (pid(12), Some(pid(11))), // desktop-owned CLI
                (pid(20), Some(pid(1))),  // independent CLI
                (pid(21), Some(pid(20))),
                (pid(30), Some(pid(1))), // ordinary ChatGPT
            ],
        );
        assert_eq!(targets, HashSet::from([pid(10), pid(11), pid(12)]));
    }

    #[test]
    fn switching_tool_survives_when_launched_from_codex_terminal() {
        let pid = sysinfo::Pid::from_u32;
        let targets = windows_switch_stop_targets(
            HashSet::from([pid(10)]),
            &[
                (pid(10), Some(pid(1))),
                (pid(11), Some(pid(10))),
                (pid(12), Some(pid(11))), // Codex Tools
                (pid(13), Some(pid(12))), // its WebView/helper
                (pid(20), Some(pid(1))),  // unrelated CLI
            ],
            pid(12),
        );
        assert_eq!(targets, HashSet::from([pid(10), pid(11)]));
    }

    #[test]
    fn missing_launch_targets_fail_before_switch_with_both_causes() {
        let plan = WindowsCodexLaunchPlan {
            desktop: None,
            desktop_to_stop: None,
            fallback_cli: Err("CLI not found".to_string()),
            discovery_error: Some("Store query failed".to_string()),
            as_admin: false,
        };
        let error = plan.validate().unwrap_err();
        assert!(error.contains("未停止应用或更改当前账号"));
        assert!(error.contains("Store query failed"));
        assert!(error.contains("CLI not found"));
    }

    #[test]
    fn desktop_does_not_require_a_separate_cli_installation() {
        let plan = WindowsCodexLaunchPlan {
            desktop: Some(WindowsDesktopTarget::Store(WindowsStoreCodexTarget {
                aumid: "OpenAI.Codex_publisher!App".to_string(),
                executable: PathBuf::from(r"C:\package\app\ChatGPT.exe"),
            })),
            desktop_to_stop: None,
            fallback_cli: Err("CLI not found".to_string()),
            discovery_error: None,
            as_admin: false,
        };
        assert!(plan.validate().is_ok());
        let error =
            windows_launch_failure(Some("AUMID activation failed HRESULT"), "CLI not found");
        assert!(error.contains("AUMID activation failed HRESULT"));
        assert!(error.contains("CLI not found"));
    }

    #[test]
    #[ignore = "read-only installed Windows desktop diagnostic; run explicitly on a real host"]
    fn installed_windows_desktop_diagnostic() {
        let plan = prepare_windows_codex_launch(None, false).expect("resolve installed desktop");
        let target = plan.desktop.as_ref().expect("installed desktop target");
        let system = sysinfo::System::new_all();
        let pids =
            verified_windows_desktop_process_ids(&system, &[target.executable().to_path_buf()])
                .expect("read current-user desktop process identity");
        println!(
            "desktop={target:?}; verified_pids={pids:?}; standalone_cli_available={}",
            plan.fallback_cli.is_ok()
        );
        assert!(target.executable().is_file());
        // Intentionally do not activate, stop, or switch anything in this diagnostic.
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::is_macos_codex_app_bundle;
    use super::macos_codex_app_candidates;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn macos_candidates_prioritize_chatgpt_and_keep_legacy_names() {
        let candidates = macos_codex_app_candidates(Some(Path::new("/Users/tester")));

        assert_eq!(
            candidates,
            vec![
                Path::new("/Applications/ChatGPT.app").to_path_buf(),
                Path::new("/Users/tester/Applications/ChatGPT.app").to_path_buf(),
                Path::new("/Applications/Codex.app").to_path_buf(),
                Path::new("/Users/tester/Applications/Codex.app").to_path_buf(),
                Path::new("/Applications/Codex Desktop.app").to_path_buf(),
                Path::new("/Users/tester/Applications/Codex Desktop.app").to_path_buf(),
            ]
        );
    }

    #[test]
    fn chatgpt_candidate_requires_embedded_codex_but_legacy_bundle_stays_compatible() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after unix epoch")
            .as_nanos();
        let sandbox = std::env::temp_dir().join(format!(
            "codex-tools-cli-test-{}-{nonce}",
            std::process::id()
        ));
        let chatgpt_app = sandbox.join("ChatGPT.app");
        let legacy_app = sandbox.join("Codex.app");
        fs::create_dir_all(chatgpt_app.join("Contents").join("Resources"))
            .expect("create ChatGPT test bundle");
        fs::create_dir_all(&legacy_app).expect("create legacy Codex test bundle");

        assert!(!is_macos_codex_app_bundle(&chatgpt_app));
        assert!(is_macos_codex_app_bundle(&legacy_app));

        let embedded_codex = chatgpt_app.join("Contents").join("Resources").join("codex");
        fs::write(&embedded_codex, b"test").expect("write embedded codex marker");
        let mut permissions = fs::metadata(&embedded_codex)
            .expect("read marker metadata")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&embedded_codex, permissions).expect("make marker executable");

        assert!(is_macos_codex_app_bundle(&chatgpt_app));
        let _ = fs::remove_dir_all(sandbox);
    }
}
