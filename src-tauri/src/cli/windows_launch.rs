//! Resolve and validate the launch destination before switching credentials.
use super::*;

#[cfg(target_os = "windows")]
#[derive(Debug)]
pub(super) enum WindowsDesktopTarget {
    Store(WindowsStoreCodexTarget),
    Executable(PathBuf),
}

#[cfg(target_os = "windows")]
impl WindowsDesktopTarget {
    pub(super) fn executable(&self) -> &Path {
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
    pub(super) desktop: Option<WindowsDesktopTarget>,
    pub(super) selected_desktop_path: Option<PathBuf>,
    pub(super) fallback_cli: Result<Command, String>,
    pub(super) discovery_error: Option<String>,
    pub(super) as_admin: bool,
}

#[cfg(target_os = "windows")]
pub(crate) fn prepare_windows_codex_launch(
    configured_path: Option<&str>,
    as_admin: bool,
) -> Result<WindowsCodexLaunchPlan, String> {
    let _timing = crate::switch_timing::Phase::start("desktop_discovery");
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
    let selected_desktop_path = desktop
        .as_ref()
        .map(|target| target.executable().to_path_buf());
    if as_admin && matches!(desktop, Some(WindowsDesktopTarget::Store(_))) {
        log::warn!("微软商店版 ChatGPT/Codex 不支持以管理员身份启动，自动使用普通权限启动。");
    }
    let plan = WindowsCodexLaunchPlan {
        desktop,
        selected_desktop_path,
        fallback_cli: new_codex_command(configured_path),
        discovery_error,
        as_admin,
    };
    plan.validate()?;
    Ok(plan)
}

#[cfg(target_os = "windows")]
pub(super) fn windows_launch_failure(desktop_error: Option<&str>, cli_error: &str) -> String {
    match desktop_error {
        Some(error) => format!("ChatGPT/Codex 桌面启动失败: {error}；CLI 回退失败: {cli_error}"),
        None => format!("未找到可用的 ChatGPT/Codex 桌面启动目标；CLI 回退失败: {cli_error}"),
    }
}

#[cfg(target_os = "windows")]
impl WindowsCodexLaunchPlan {
    pub(super) fn validate(&self) -> Result<(), String> {
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
        super::windows_stop::stop_desktop(self.selected_desktop_path.as_deref())
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
