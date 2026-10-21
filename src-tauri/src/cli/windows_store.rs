//! Discover and activate the registered OpenAI.Codex Store application.
use super::*;
use windows::core::HSTRING;
use windows::Win32::Foundation::RPC_E_CHANGED_MODE;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_LOCAL_SERVER, COINIT_MULTITHREADED,
};
use windows::Win32::UI::Shell::{
    ApplicationActivationManager, IApplicationActivationManager, ACTIVATEOPTIONS, AO_NOERRORUI,
};

const STORE_ACTIVATION_OPTIONS: ACTIVATEOPTIONS = AO_NOERRORUI;

#[cfg(target_os = "windows")]
pub(super) fn find_windows_codex_store_app_id() -> Option<String> {
    find_windows_codex_store_target()
        .ok()
        .flatten()
        .map(|target| target.aumid)
}

#[cfg(target_os = "windows")]
#[derive(Debug, Clone, serde::Deserialize)]
pub(super) struct WindowsStoreCodexTarget {
    pub(super) aumid: String,
    pub(super) executable: PathBuf,
}

#[cfg(target_os = "windows")]
pub(super) fn find_windows_codex_store_target() -> Result<Option<WindowsStoreCodexTarget>, String> {
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
pub(super) fn activate_windows_store_codex_by_aumid(app_id: &str) -> Result<u32, String> {
    let _com_guard = WindowsComGuard::initialize()?;
    let activation_manager: IApplicationActivationManager =
        unsafe { CoCreateInstance(&ApplicationActivationManager, None, CLSCTX_LOCAL_SERVER) }
            .map_err(|error| format!("创建微软商店激活管理器失败: {error}"))?;

    let app_id = HSTRING::from(app_id);
    let arguments = HSTRING::new();
    unsafe { activation_manager.ActivateApplication(&app_id, &arguments, STORE_ACTIVATION_OPTIONS) }
        .map_err(|error| format!("通过 AUMID 激活 Codex 失败: {error}"))
}

#[cfg(target_os = "windows")]
pub(super) fn launch_windows_store_target(target: &WindowsStoreCodexTarget) -> Result<(), String> {
    launch_windows_store_target_with(
        target,
        process_is_running,
        activate_windows_store_codex_by_aumid,
        wait_for_windows_codex_process,
    )
}

#[cfg(target_os = "windows")]
fn launch_windows_store_target_with(
    target: &WindowsStoreCodexTarget,
    is_running: impl FnOnce(&Path) -> bool,
    activate: impl FnOnce(&str) -> Result<u32, String>,
    wait_for_process: impl FnOnce(&Path) -> bool,
) -> Result<(), String> {
    if is_running(&target.executable) {
        log::info!("CODEX_DESKTOP_LAUNCH action=already_running");
        // Reactivate an existing window, but a focus failure does not make the
        // current-session desktop stop being available. Never open a shell URI.
        if let Err(error) = activate(&target.aumid) {
            log::warn!("CODEX_DESKTOP_LAUNCH action=reactivation_failed error={error}");
        }
        return Ok(());
    }

    // Shell namespace dispatch via `cmd /c start` can show an unowned
    // "Class not registered" dialog, and process creation cannot report that
    // activation failure. Call the registered App contract directly instead.
    // AO_NOERRORUI keeps errors in our Result/log/status rather than a Shell dialog.
    log::info!("CODEX_DESKTOP_LAUNCH action=activating method=aumid aumid={}", target.aumid);
    let process_id = activate(&target.aumid).map_err(|error| {
        let detail = format!("启动微软商店版 Codex 失败（AUMID={}）：{error}", target.aumid);
        log::warn!("CODEX_DESKTOP_LAUNCH action=activation_failed method=aumid error={detail}");
        detail
    })?;
    log::info!(
        "CODEX_DESKTOP_LAUNCH method=aumid aumid={} executable={} activation_pid={process_id}",
        target.aumid,
        target.executable.display()
    );
    if !wait_for_process(&target.executable) {
        return Err(format!(
            "微软商店版 Codex 激活后未检测到当前用户的桌面进程（AUMID={}，程序={}，激活 PID={process_id}，等待 {WINDOWS_STORE_LAUNCH_TIMEOUT_MS} ms）。",
            target.aumid, target.executable.display()
        ));
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn process_is_running(executable: &Path) -> bool {
    let mut system = sysinfo::System::new();
    refresh_windows_switch_processes(&mut system);
    verified_windows_desktop_process_ids(&system, &[executable.to_path_buf()])
        .is_ok_and(|pids| !pids.is_empty())
}

#[cfg(target_os = "windows")]
struct WindowsComGuard {
    should_uninitialize: bool,
}

#[cfg(target_os = "windows")]
impl WindowsComGuard {
    fn initialize() -> Result<Self, String> {
        // 调用方可能位于 Tauri 的 async 线程；MTA 更宽松，避免 RPC_E_CHANGED_MODE。
        let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    fn target() -> WindowsStoreCodexTarget {
        WindowsStoreCodexTarget {
            aumid: "OpenAI.Codex_publisher!App".to_string(),
            executable: PathBuf::from(r"C:\package\app\ChatGPT.exe"),
        }
    }

    #[test]
    fn cold_launch_activates_registered_app_before_process_verification() {
        let target = target();
        let stage = Cell::new(0);
        launch_windows_store_target_with(&target,
            |path| { assert_eq!(path, target.executable); assert_eq!(stage.replace(1), 0); false },
            |aumid| { assert_eq!(aumid, target.aumid); assert_eq!(stage.replace(2), 1); Ok(42) },
            |path| { assert_eq!(path, target.executable); assert_eq!(stage.replace(3), 2); true },
        ).unwrap();
        assert_eq!(stage.get(), 3);
        assert_eq!(STORE_ACTIVATION_OPTIONS, AO_NOERRORUI);
    }

    #[test]
    fn failed_activation_preserves_hresult_and_cannot_be_confirmed_by_a_later_manual_launch() {
        let target = target();
        let error = launch_windows_store_target_with(&target,
            |_| false,
            |_| Err("Class not registered (0x80040154)".to_string()),
            |_| panic!("process polling must not turn failed activation into success"),
        ).unwrap_err();
        assert!(error.contains("0x80040154"));
        assert!(error.contains(&target.aumid));
    }

    #[test]
    fn activation_broker_pid_without_desktop_is_not_a_success() {
        let error = launch_windows_store_target_with(&target(),
            |_| false, |_| Ok(42), |_| false,
        ).unwrap_err();
        assert!(error.contains("未检测到当前用户的桌面进程"));
        assert!(error.contains("42"));
    }

    #[test]
    fn existing_verified_desktop_remains_available_if_refocusing_fails() {
        launch_windows_store_target_with(&target(),
            |_| true,
            |_| Err("Unable to refocus".to_string()),
            |_| panic!("an already verified desktop does not need another launch wait"),
        ).unwrap();
    }
}
