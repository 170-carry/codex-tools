//! Stop every verified running Codex desktop before writing account files.
use super::*;

pub(super) fn stop_desktop(executable: Option<&Path>) -> Result<(), String> {
    let _timing = crate::switch_timing::Phase::start("desktop_stop");
    let expected = executable
        .map(Path::to_path_buf)
        .into_iter()
        .collect::<Vec<_>>();
    let mut system = sysinfo::System::new();
    refresh_windows_switch_processes(&mut system);
    let current_pid = sysinfo::get_current_pid().map_err(|error| error.to_string())?;
    let current = system
        .process(current_pid)
        .ok_or_else(|| "无法识别当前进程，未停止应用。".to_string())?;
    let user = current
        .user_id()
        .cloned()
        .ok_or_else(|| "无法识别当前用户，未停止应用。".to_string())?;
    let session = current
        .session_id()
        .ok_or("无法识别当前会话，未停止应用。")?;
    let roots = running_windows_desktop_process_ids(&system, &expected)?;
    let parents = system
        .processes()
        .iter()
        .filter(|(_, process)| {
            process.user_id() == Some(&user) && process.session_id() == Some(session)
        })
        .map(|(pid, process)| (*pid, process.parent()))
        .collect::<Vec<_>>();
    let targets = windows_switch_stop_targets(roots, &parents, current_pid);
    // Acquire handles before taking the authoritative snapshot. Holding each
    // handle pins process identity while user/session/ancestry are checked.
    let mut handles = Vec::new();
    for pid in targets {
        match crate::windows_desktop_lifecycle::ProcessHandle::open(pid) {
            Ok(handle) => handles.push(handle),
            Err(error) => {
                refresh_windows_switch_processes(&mut system);
                if system.process(pid).is_some() {
                    return Err(error);
                }
            }
        }
    }
    system = sysinfo::System::new();
    refresh_windows_switch_processes(&mut system);
    let roots = running_windows_desktop_process_ids(&system, &expected)?;
    let parents = system
        .processes()
        .iter()
        .filter(|(_, p)| p.user_id() == Some(&user) && p.session_id() == Some(session))
        .map(|(pid, p)| (*pid, p.parent()))
        .collect::<Vec<_>>();
    let verified = windows_switch_stop_targets(roots.clone(), &parents, current_pid);
    for handle in &handles {
        if handle.exited()? {
            continue;
        }
        let process = system
            .process(handle.pid)
            .ok_or("进程核验状态变化，未更改当前账号。")?;
        if !verified.contains(&handle.pid)
            || !process.exe().is_some_and(|path| {
                handle
                    .executable()
                    .is_ok_and(|actual| windows_paths_equal(path, &actual))
            })
        {
            return Err("进程身份或归属发生变化，未更改当前账号。".into());
        }
    }
    if verified
        .iter()
        .any(|pid| !handles.iter().any(|h| h.pid == *pid))
    {
        return Err("停止前出现新的桌面子进程，未更改当前账号，请重试。".into());
    }
    crate::windows_desktop_lifecycle::stop(&handles, &roots)?;
    refresh_windows_switch_processes(&mut system);
    let parents = system
        .processes()
        .iter()
        .filter(|(_, p)| p.user_id() == Some(&user) && p.session_id() == Some(session))
        .map(|(pid, p)| (*pid, p.parent()))
        .collect::<Vec<_>>();
    let descendants = windows_switch_stop_targets(
        handles.iter().map(|h| h.pid).collect(),
        &parents,
        current_pid,
    );
    if descendants.iter().any(|pid| system.process(*pid).is_some()) {
        return Err("停止期间出现残留子进程，未更改当前账号，请重试。".into());
    }
    if !running_windows_desktop_process_ids(&system, &expected)?.is_empty() {
        return Err(
            "ChatGPT/Codex 在停止期间重新启动，未更改当前账号，请关闭该桌面后重试。".to_string(),
        );
    }
    cleanup_thread_writer_locks();
    Ok(())
}

pub(crate) fn cleanup_thread_writer_locks() {
    if let Ok(dir) = crate::app_paths::codex_dir() {
        let locks_dir = dir.join("thread-writer-locks");
        if let Ok(entries) = std::fs::read_dir(&locks_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("lock") {
                    let _ = std::fs::remove_file(path);
                }
            }
        }
    }
}
