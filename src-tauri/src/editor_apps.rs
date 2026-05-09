use std::path::PathBuf;
use std::process::Command;
use std::thread;
use std::time::Duration;

use crate::models::EditorAppId;
use crate::models::InstalledEditorApp;
#[cfg(target_os = "windows")]
use crate::utils::new_background_command;

const RESTART_SETTLE_MS: u64 = 220;

struct EditorSpec {
    id: EditorAppId,
    label: &'static str,
    bundle_names: &'static [&'static str],
    process_names: &'static [&'static str],
}

const EDITOR_SPECS: &[EditorSpec] = &[
    EditorSpec {
        id: EditorAppId::Vscode,
        label: "VS Code",
        bundle_names: &["Visual Studio Code.app", "Code.app"],
        process_names: &["Code", "Visual Studio Code"],
    },
    EditorSpec {
        id: EditorAppId::VscodeInsiders,
        label: "Visual Studio Code - Insiders",
        bundle_names: &["Visual Studio Code - Insiders.app", "Code - Insiders.app"],
        process_names: &["Code - Insiders", "Visual Studio Code - Insiders"],
    },
    EditorSpec {
        id: EditorAppId::Cursor,
        label: "Cursor",
        bundle_names: &["Cursor.app"],
        process_names: &["Cursor"],
    },
    EditorSpec {
        id: EditorAppId::Antigravity,
        label: "Antigravity",
        bundle_names: &["Antigravity.app", "Antigravity IDE.app"],
        process_names: &["Antigravity", "Antigravity IDE"],
    },
    EditorSpec {
        id: EditorAppId::Kiro,
        label: "Kiro",
        bundle_names: &["Kiro.app"],
        process_names: &["Kiro"],
    },
    EditorSpec {
        id: EditorAppId::Trae,
        label: "Trae",
        bundle_names: &["Trae.app"],
        process_names: &["Trae"],
    },
    EditorSpec {
        id: EditorAppId::Qoder,
        label: "Qoder",
        bundle_names: &["Qoder.app"],
        process_names: &["Qoder"],
    },
];

/// 返回当前机器上已安装且可重启的编辑器列表（用于设置页下拉选择）。
pub(crate) fn list_installed_editor_apps() -> Vec<InstalledEditorApp> {
    EDITOR_SPECS
        .iter()
        .filter_map(|spec| {
            detect_editor_bundle_path(spec).map(|_| InstalledEditorApp {
                id: spec.id,
                label: spec.label.to_string(),
            })
        })
        .collect()
}

/// 对选中的编辑器执行“强制关闭并重启”。
pub(crate) fn restart_selected_editor_apps(
    targets: &[EditorAppId],
) -> (Vec<EditorAppId>, Option<String>) {
    if targets.is_empty() {
        return (Vec::new(), None);
    }

    let mut restarted = Vec::new();
    let mut errors = Vec::new();

    for target in targets {
        let Some(spec) = find_spec(*target) else {
            errors.push(format!("未知编辑器标识: {:?}", target));
            continue;
        };

        match restart_editor(spec) {
            Ok(()) => restarted.push(spec.id),
            Err(err) => errors.push(format!("{}: {}", spec.label, err)),
        }
    }

    if errors.is_empty() {
        (restarted, None)
    } else {
        (restarted, Some(errors.join(" | ")))
    }
}

fn restart_editor(spec: &EditorSpec) -> Result<(), String> {
    let app_path = detect_editor_bundle_path(spec).ok_or_else(|| "未检测到安装路径".to_string())?;

    force_kill_processes(spec.process_names);
    thread::sleep(Duration::from_millis(RESTART_SETTLE_MS));
    reopen_app(&app_path)
}

fn find_spec(id: EditorAppId) -> Option<&'static EditorSpec> {
    EDITOR_SPECS.iter().find(|spec| spec.id == id)
}

fn detect_editor_bundle_path(spec: &EditorSpec) -> Option<PathBuf> {
    #[cfg(not(target_os = "macos"))]
    let _ = spec;

    #[cfg(target_os = "macos")]
    {
        let mut candidates = Vec::<PathBuf>::new();

        for bundle_name in spec.bundle_names {
            candidates.push(PathBuf::from("/Applications").join(bundle_name));
            if let Some(home) = dirs::home_dir() {
                candidates.push(home.join("Applications").join(bundle_name));
            }
        }

        if let Some(found) = candidates.into_iter().find(|path| path.exists()) {
            return Some(found);
        }
    }

    None
}

fn force_kill_processes(process_names: &[&str]) {
    #[cfg(target_os = "macos")]
    {
        for name in process_names {
            let _ = Command::new("pkill").args(["-9", "-x", name]).status();
        }
    }

    #[cfg(target_os = "windows")]
    {
        for name in process_names {
            let image_name = if name.to_ascii_lowercase().ends_with(".exe") {
                (*name).to_string()
            } else {
                format!("{name}.exe")
            };
            let _ = new_background_command("taskkill")
                .args(["/F", "/IM", &image_name, "/T"])
                .status();
        }
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        for name in process_names {
            let _ = Command::new("pkill").args(["-9", "-x", name]).status();
        }
    }
}

fn reopen_app(app_path: &PathBuf) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let status = Command::new("open")
            .arg("-na")
            .arg(app_path)
            .status()
            .map_err(|e| format!("重启应用失败: {e}"))?;
        if !status.success() {
            return Err("open 命令返回非零状态".to_string());
        }
        return Ok(());
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = app_path;
        Err("当前平台暂不支持编辑器自动重启".to_string())
    }
}
