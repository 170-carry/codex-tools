use crate::cli;
use std::{collections::HashSet, process::Command, thread, time::Duration};

#[cfg(target_os = "macos")]
pub(crate) fn macos_app_bundle_for_main_executable(
    executable: &std::path::Path,
) -> Option<&std::path::Path> {
    let macos_dir = executable.parent()?;
    if !macos_dir
        .file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("MacOS"))
    {
        return None;
    }

    let contents_dir = macos_dir.parent()?;
    if !contents_dir
        .file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("Contents"))
    {
        return None;
    }

    let app_bundle = contents_dir.parent()?;
    app_bundle
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("app"))
        .then_some(app_bundle)
}

#[cfg(target_os = "macos")]
pub(crate) fn macos_codex_main_app_bundle_for_executable(
    executable: &std::path::Path,
) -> Option<&std::path::Path> {
    let app_bundle = macos_app_bundle_for_main_executable(executable)?;
    if !cli::is_macos_codex_app_bundle(app_bundle) {
        return None;
    }

    let expected_executable_name = app_bundle.file_stem()?.to_str()?;
    let executable_name = executable.file_name()?.to_str()?;
    executable_name
        .eq_ignore_ascii_case(expected_executable_name)
        .then_some(app_bundle)
}

#[cfg(target_os = "macos")]
pub(crate) fn collect_descendant_process_ids(
    mut targets: HashSet<sysinfo::Pid>,
    process_parents: &[(sysinfo::Pid, Option<sysinfo::Pid>)],
) -> HashSet<sysinfo::Pid> {
    loop {
        let previous_count = targets.len();
        for (pid, parent) in process_parents {
            if parent.is_some_and(|parent| targets.contains(&parent)) {
                targets.insert(*pid);
            }
        }
        if targets.len() == previous_count {
            return targets;
        }
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn effective_codex_home_from_environment<'a>(
    environment: impl IntoIterator<Item = &'a String>,
    default_home: &std::path::Path,
) -> std::path::PathBuf {
    environment
        .into_iter()
        .find_map(|entry| {
            entry
                .strip_prefix("CODEX_HOME=")
                .filter(|value| !value.is_empty())
                .map(std::path::PathBuf::from)
        })
        .unwrap_or_else(|| default_home.to_path_buf())
}

#[cfg(target_os = "macos")]
pub(crate) fn running_macos_codex_desktop_root_process_ids(
    system: &sysinfo::System,
    current_user_id: &sysinfo::Uid,
) -> HashSet<sysinfo::Pid> {
    system
        .processes()
        .iter()
        .filter(|(_, process)| process.user_id() == Some(current_user_id))
        .filter_map(|(pid, process)| {
            process.exe().and_then(|executable| {
                macos_codex_main_app_bundle_for_executable(executable).map(|_| *pid)
            })
        })
        .collect()
}

#[cfg(target_os = "macos")]
pub(crate) fn running_macos_codex_desktop_process_ids(
    system: &sysinfo::System,
    current_user_id: &sysinfo::Uid,
) -> HashSet<sysinfo::Pid> {
    let desktop_roots = running_macos_codex_desktop_root_process_ids(system, current_user_id);
    let same_user_processes = system
        .processes()
        .iter()
        .filter(|(_, process)| process.user_id() == Some(current_user_id));
    let process_parents = same_user_processes
        .map(|(pid, process)| (*pid, process.parent()))
        .collect::<Vec<_>>();

    collect_descendant_process_ids(desktop_roots, &process_parents)
}

#[cfg(target_os = "macos")]
pub(crate) fn stop_running_macos_codex_processes() -> Result<(), String> {
    let mut system = sysinfo::System::new_all();
    let current_pid = sysinfo::get_current_pid().map_err(|error| error.to_string())?;
    let current_user_id = system
        .process(current_pid)
        .and_then(|process| process.user_id())
        .cloned()
        .ok_or_else(|| "无法识别当前用户，未结束 ChatGPT/Codex 应用进程".to_string())?;
    let mut remaining = running_macos_codex_desktop_process_ids(&system, &current_user_id);
    if remaining.is_empty() {
        return Ok(());
    }

    let app_names = running_macos_codex_desktop_root_process_ids(&system, &current_user_id)
        .into_iter()
        .filter_map(|pid| system.process(pid))
        .filter_map(|process| process.exe())
        .filter_map(macos_codex_main_app_bundle_for_executable)
        .filter_map(|bundle| bundle.file_stem())
        .filter_map(|name| name.to_str())
        .map(str::to_string)
        .collect::<HashSet<_>>();
    for app_name in app_names {
        // Bundle names have already passed the fixed Codex/ChatGPT allowlist in
        // is_macos_codex_app_bundle, so they are safe to embed in this script.
        let script = format!("tell application \"{app_name}\" to quit");
        match Command::new("osascript").args(["-e", &script]).output() {
            Ok(output) if output.status.success() => {
                log::info!("MACOS_CODEX_STOP action=request-quit app={app_name}");
            }
            Ok(output) => {
                let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
                log::warn!(
                    "请求 Codex 正常退出失败 app={} status={} detail={}",
                    app_name,
                    output.status,
                    detail
                );
            }
            Err(error) => log::warn!("调用 osascript 请求 Codex 正常退出失败: {error}"),
        }
    }

    let graceful_deadline = std::time::Instant::now() + Duration::from_secs(4);
    loop {
        thread::sleep(Duration::from_millis(50));
        system.refresh_processes();
        remaining = running_macos_codex_desktop_process_ids(&system, &current_user_id);
        if remaining.is_empty() {
            // LaunchServices can briefly retain the just-terminated instance even
            // after its processes disappear. Give it time to accept a fresh launch.
            thread::sleep(Duration::from_millis(800));
            return Ok(());
        }
        if std::time::Instant::now() >= graceful_deadline {
            break;
        }
    }

    log::warn!("Codex 未在 4 秒内正常退出，开始结束剩余进程");
    let forced_deadline = std::time::Instant::now() + Duration::from_secs(2);
    loop {
        // 按已验证 App bundle 的可执行路径结束整个进程树，避免裸进程名误杀普通 ChatGPT。
        for pid in &remaining {
            if let Some(process) = system.process(*pid) {
                let _ = process.kill();
            }
        }

        thread::sleep(Duration::from_millis(50));
        system.refresh_processes();
        remaining = running_macos_codex_desktop_process_ids(&system, &current_user_id);
        if remaining.is_empty() {
            thread::sleep(Duration::from_millis(1_200));
            return Ok(());
        }
        if std::time::Instant::now() >= forced_deadline {
            let pids = remaining
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            return Err(format!("无法结束正在运行的 ChatGPT/Codex 应用进程: {pids}"));
        }
    }
}

#[cfg(not(target_os = "windows"))]
#[cfg(target_os = "macos")]
pub(crate) fn macos_paths_refer_to_same_bundle(
    left: &std::path::Path,
    right: &std::path::Path,
) -> bool {
    if left == right {
        return true;
    }
    match (left.canonicalize(), right.canonicalize()) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn current_user_has_macos_app_process(
    path: &std::path::Path,
    mut matches_process: impl FnMut(&sysinfo::Process) -> bool,
) -> bool {
    let system = sysinfo::System::new_all();
    let Ok(current_pid) = sysinfo::get_current_pid() else {
        return false;
    };
    let Some(current_user_id) = system
        .process(current_pid)
        .and_then(|process| process.user_id())
    else {
        return false;
    };

    system.processes().values().any(|process| {
        process.user_id() == Some(current_user_id)
            && process
                .exe()
                .and_then(macos_app_bundle_for_main_executable)
                .is_some_and(|bundle| macos_paths_refer_to_same_bundle(bundle, path))
            && matches_process(process)
    })
}

#[cfg(target_os = "macos")]
pub(crate) fn is_macos_app_running(path: &std::path::Path) -> bool {
    current_user_has_macos_app_process(path, |_| true)
}

#[cfg(target_os = "macos")]
pub(crate) fn is_macos_app_running_with_home(
    path: &std::path::Path,
    expected_home: &std::path::Path,
) -> bool {
    let Some(default_home) = dirs::home_dir().map(|home| home.join(".codex")) else {
        return false;
    };

    current_user_has_macos_app_process(path, |process| {
        macos_paths_refer_to_same_bundle(
            &effective_codex_home_from_environment(process.environ(), &default_home),
            expected_home,
        )
    })
}
