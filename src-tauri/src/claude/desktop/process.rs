use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use sysinfo::{Signal, System};

pub fn stop() -> Result<Option<PathBuf>, String> {
    let mut system = System::new_all();
    let processes = system
        .processes()
        .iter()
        .filter(|(_, p)| {
            p.exe().is_some_and(|path| {
                let s = path.to_string_lossy();
                s.ends_with("/Claude.app/Contents/MacOS/Claude")
                    || s.ends_with("\\Claude.exe")
                    || s.ends_with("/claude-desktop")
            })
        })
        .map(|(pid, p)| (*pid, p.exe().unwrap().to_path_buf()))
        .collect::<Vec<_>>();
    let helpers = system
        .processes()
        .iter()
        .filter(|(_, p)| {
            p.exe().is_some_and(|path| {
                path.to_string_lossy()
                    .contains("/Claude.app/Contents/Frameworks/Claude Helper")
            })
        })
        .map(|(pid, _)| *pid)
        .collect::<Vec<_>>();
    for (pid, _) in &processes {
        if let Some(process) = system.process(*pid) {
            if !process.kill_with(Signal::Term).unwrap_or(false) {
                return Err("无法正常退出 Claude 桌面 App，请先手动退出再切换".into());
            }
        }
    }
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        system.refresh_processes();
        if processes
            .iter()
            .all(|(pid, _)| system.process(*pid).is_none())
            && helpers.iter().all(|pid| system.process(*pid).is_none())
        {
            return Ok(processes.first().map(|(_, path)| path.clone()));
        }
        if Instant::now() >= deadline {
            return Err("Claude 桌面 App 尚未退出，请结束当前操作后重试".into());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

pub fn launch(executable: Option<PathBuf>) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let result = std::process::Command::new("open")
        .args(["-a", "Claude"])
        .status()
        .and_then(|status| {
            if status.success() {
                Ok(())
            } else {
                Err(std::io::Error::other("macOS 无法打开 Claude"))
            }
        });
    #[cfg(not(target_os = "macos"))]
    let result = std::process::Command::new(executable.ok_or("未找到 Claude 桌面 App 启动路径")?)
        .spawn()
        .map(|_| ());
    #[cfg(target_os = "macos")]
    let _ = executable;
    result
        .map(|_| ())
        .map_err(|e| format!("启动 Claude 桌面 App 失败: {e}"))
}
