//! Native lifecycle; tests inject a fake and never call this adapter.
use super::*;
pub(crate) struct NativeLifecycle {
    #[cfg(target_os = "macos")]
    configured: Option<String>,
    #[cfg(target_os = "windows")]
    plan: Option<crate::cli::WindowsCodexLaunchPlan>,
}
impl NativeLifecycle {
    pub(crate) fn new(
        paths: &SwitchPaths,
        configured: Option<&str>,
        as_admin: bool,
        launch: bool,
    ) -> Result<Self, String> {
        #[cfg(target_os = "macos")]
        {
            let _ = (paths, as_admin, launch);
            Ok(Self {
                configured: configured.map(str::to_owned),
            })
        }
        #[cfg(target_os = "windows")]
        {
            if launch && paths.codex_home != crate::app_paths::codex_dir()? {
                return Err("PROJECT_SHARING_UNSUPPORTED_HOME".into());
            }
            Ok(Self {
                plan: Some(
                    crate::cli::prepare_windows_codex_launch(configured, as_admin)
                        .map_err(|_| "PROJECT_SHARING_DESKTOP_UNAVAILABLE")?,
                ),
            })
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            let _ = (paths, configured, as_admin, launch);
            Err("PROJECT_SHARING_UNSUPPORTED_PLATFORM".into())
        }
    }
    pub(crate) fn launch(
        &mut self,
        paths: &SwitchPaths,
        workspace: Option<&str>,
    ) -> Result<(Option<String>, bool), String> {
        #[cfg(target_os = "macos")]
        {
            let path = crate::cli::find_configured_codex_app_path(self.configured.as_deref())
                .or_else(crate::cli::find_codex_app_path)
                .ok_or("PROJECT_SHARING_LAUNCH_FAILED")?;
            crate::macos_desktop::launch::run_macos_codex_open_command(
                &path,
                workspace,
                &paths.codex_home,
            )
            .map_err(|_| "PROJECT_SHARING_LAUNCH_FAILED")?;
            if !crate::macos_desktop::launch::wait_for_macos_codex_launch_with_probe(
                std::time::Duration::from_secs(4),
                std::time::Duration::from_millis(150),
                || {
                    crate::macos_desktop::process::is_macos_app_running_with_home(
                        &path,
                        &paths.codex_home,
                    )
                },
            ) {
                return Err("PROJECT_SHARING_LAUNCH_FAILED".into());
            }
            Ok((Some(path.to_string_lossy().into_owned()), false))
        }
        #[cfg(target_os = "windows")]
        {
            let _ = paths;
            self.plan
                .take()
                .ok_or("PROJECT_SHARING_LAUNCH_FAILED")?
                .launch(workspace)
                .map_err(|_| "PROJECT_SHARING_LAUNCH_FAILED".into())
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            let _ = (paths, workspace);
            Err("PROJECT_SHARING_UNSUPPORTED_PLATFORM".into())
        }
    }
}
impl DesktopLifecycle for NativeLifecycle {
    fn stop_and_verify(&mut self) -> Result<(), String> {
        #[cfg(target_os = "macos")]
        crate::macos_desktop::process::stop_running_macos_codex_processes()
            .map_err(|_| "PROJECT_SHARING_DESKTOP_RUNNING")?;
        #[cfg(target_os = "windows")]
        self.plan
            .as_ref()
            .ok_or("PROJECT_SHARING_DESKTOP_UNAVAILABLE")?
            .stop_desktop()
            .map_err(|_| "PROJECT_SHARING_DESKTOP_RUNNING")?;
        self.assert_stopped()
    }
    fn assert_stopped(&mut self) -> Result<(), String> {
        #[cfg(target_os = "macos")]
        {
            let system = sysinfo::System::new_all();
            let pid = sysinfo::get_current_pid().map_err(|_| "PROJECT_SHARING_DESKTOP_RUNNING")?;
            let user = system
                .process(pid)
                .and_then(|p| p.user_id())
                .ok_or("PROJECT_SHARING_DESKTOP_RUNNING")?;
            if !crate::macos_desktop::process::running_macos_codex_desktop_process_ids(
                &system, user,
            )
            .is_empty()
            {
                return Err("PROJECT_SHARING_DESKTOP_RUNNING".into());
            }
            Ok(())
        }
        #[cfg(target_os = "windows")]
        {
            self.plan
                .as_ref()
                .ok_or("PROJECT_SHARING_DESKTOP_UNAVAILABLE")?
                .assert_stopped()
                .map_err(|_| "PROJECT_SHARING_DESKTOP_RUNNING".into())
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            Err("PROJECT_SHARING_UNSUPPORTED_PLATFORM".into())
        }
    }
}
