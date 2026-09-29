use super::process::{is_macos_app_running, is_macos_app_running_with_home};
use crate::app_paths;
use std::{process::Command, thread, time::Duration};

#[cfg(target_os = "macos")]
pub(crate) fn macos_codex_open_args(
    path: &std::path::Path,
    workspace_path: Option<&str>,
    codex_home: &std::path::Path,
) -> Vec<std::ffi::OsString> {
    let mut codex_home_env = std::ffi::OsString::from("CODEX_HOME=");
    codex_home_env.push(codex_home.as_os_str());

    let mut args = vec![
        std::ffi::OsString::from("-n"),
        std::ffi::OsString::from("--env"),
        codex_home_env,
        std::ffi::OsString::from("-a"),
        path.as_os_str().to_os_string(),
    ];
    if let Some(workspace) = workspace_path {
        args.push(std::ffi::OsString::from(workspace));
    }
    args
}

#[cfg(target_os = "macos")]
const MACOS_CODEX_LAUNCH_STABLE_OBSERVATIONS: usize = 3;
#[cfg(target_os = "macos")]
const MACOS_CODEX_LAUNCH_ATTEMPTS: usize = 3;

#[cfg(target_os = "macos")]
pub(crate) fn wait_for_macos_codex_launch_with_probe<F>(
    timeout: Duration,
    poll_interval: Duration,
    mut is_running: F,
) -> bool
where
    F: FnMut() -> bool,
{
    let deadline = std::time::Instant::now() + timeout;
    let mut stable_observations = 0;
    loop {
        if is_running() {
            stable_observations += 1;
            if stable_observations >= MACOS_CODEX_LAUNCH_STABLE_OBSERVATIONS {
                return true;
            }
        } else {
            stable_observations = 0;
        }

        if std::time::Instant::now() >= deadline {
            return false;
        }
        thread::sleep(poll_interval);
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn run_macos_codex_open_command(
    path: &std::path::Path,
    workspace_path: Option<&str>,
    codex_home: &std::path::Path,
) -> Result<(), String> {
    let output = Command::new("open")
        .args(macos_codex_open_args(path, workspace_path, codex_home))
        .output()
        .map_err(|error| format!("调用 macOS open 失败: {error}"))?;
    if output.status.success() {
        return Ok(());
    }

    let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if detail.is_empty() {
        Err(format!("macOS open 退出状态为 {}", output.status))
    } else {
        Err(format!("macOS open 失败: {detail}"))
    }
}

pub(crate) fn launch(path: &std::path::Path, workspace_path: Option<&str>) -> Result<(), String> {
    let codex_home = app_paths::codex_dir()?;
    // The desktop process must read the same auth/profile directory that was just switched.
    // This is also what keeps debug/preview account stores isolated from ~/.codex.
    let mut failures = Vec::new();
    for attempt in 1..=MACOS_CODEX_LAUNCH_ATTEMPTS {
        let open_succeeded = match run_macos_codex_open_command(path, workspace_path, &codex_home) {
            Ok(()) => {
                let launched = wait_for_macos_codex_launch_with_probe(
                    Duration::from_secs(4),
                    Duration::from_millis(150),
                    || is_macos_app_running_with_home(path, &codex_home),
                );
                if launched {
                    log::info!(
                        "MACOS_CODEX_LAUNCH action=verified attempt={} path={}",
                        attempt,
                        path.display()
                    );
                    return Ok(());
                }
                true
            }
            Err(error) => {
                failures.push(format!("第 {attempt} 次启动失败: {error}"));
                false
            }
        };

        if is_macos_app_running_with_home(path, &codex_home) {
            let launched_late = wait_for_macos_codex_launch_with_probe(
                Duration::from_secs(4),
                Duration::from_millis(150),
                || is_macos_app_running_with_home(path, &codex_home),
            );
            if launched_late {
                log::info!(
                    "MACOS_CODEX_LAUNCH action=verified-late attempt={} path={}",
                    attempt,
                    path.display()
                );
                return Ok(());
            }
        }

        if is_macos_app_running(path) {
            failures.push(format!(
                    "第 {attempt} 次 open 后检测到目标应用进程，但未能确认其稳定使用目标 CODEX_HOME；为避免重复实例已停止重试"
                ));
            break;
        }
        if open_succeeded {
            failures.push(format!(
                    "第 {attempt} 次 open 返回成功，但 4 秒内未检测到使用目标 CODEX_HOME 的稳定 Codex 进程"
                ));
        }

        if attempt < MACOS_CODEX_LAUNCH_ATTEMPTS {
            thread::sleep(Duration::from_millis(900));
        }
    }

    Err(format!(
        "启动 Codex 应用失败（已重试 {} 次）：{}",
        MACOS_CODEX_LAUNCH_ATTEMPTS,
        failures.join("；")
    ))
}
