use crate::utils::new_background_command;
#[cfg(target_os = "windows")]
use crate::utils::new_resolved_command;
use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
#[cfg(target_os = "windows")]
use std::{
    thread,
    time::{Duration, Instant},
};

mod discovery;
#[cfg(any(target_os = "windows", test))]
mod windows_identity;
use discovery::*;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub(crate) use macos::is_macos_codex_app_bundle;
#[cfg(target_os = "macos")]
use macos::{
    append_macos_app_bundle_codex_candidates, first_spotlight_codex_app_match,
    macos_codex_app_candidates, macos_codex_cli_candidates_for_bundle,
};
#[cfg(target_os = "windows")]
mod windows_elevated;
#[cfg(target_os = "windows")]
pub(crate) use windows_elevated::launch_elevated_process;
#[cfg(target_os = "windows")]
mod windows_launch;
#[cfg(target_os = "windows")]
pub(crate) use windows_launch::prepare_windows_codex_launch;
#[cfg(all(test, target_os = "windows"))]
use windows_launch::{windows_launch_failure, WindowsCodexLaunchPlan, WindowsDesktopTarget};
#[cfg(target_os = "windows")]
mod windows_paths;
#[cfg(target_os = "windows")]
pub(crate) use windows_paths::is_windows_store_codex_path;
#[cfg(target_os = "windows")]
use windows_paths::{
    append_windows_codex_app_candidates_from_dir, find_windows_codex_app_path,
    first_executable_candidate, is_windows_codex_app_file,
};
#[cfg(target_os = "windows")]
mod windows_process;
#[cfg(target_os = "windows")]
use windows_process::*;
#[cfg(target_os = "windows")]
mod windows_stop;
#[cfg(target_os = "windows")]
mod windows_store;
#[cfg(target_os = "windows")]
use windows_store::{
    find_windows_codex_store_app_id, find_windows_codex_store_target, launch_windows_store_target,
    WindowsStoreCodexTarget,
};
#[cfg(all(test, target_os = "windows"))]
mod windows_tests;

const INVALID_CONFIGURED_CODEX_PATH_MESSAGE: &str =
    "设置的 Codex 启动路径无效。请填写 Codex 桌面程序（Codex.exe 或 OpenAI.Codex 包内的 ChatGPT.exe）、codex/codex.exe 的完整路径，或包含它们的安装目录。";
#[cfg(target_os = "windows")]
const WINDOWS_STORE_LAUNCH_TIMEOUT_MS: u64 = 8_000;
#[cfg(target_os = "windows")]
const WINDOWS_STORE_LAUNCH_POLL_MS: u64 = 250;

/// 构造可直接启动 Codex CLI 的命令。
///
/// 重点处理 GUI 进程 PATH 不完整的问题：
/// 先定位真实可执行路径，再把其父目录注入子进程 PATH。
pub(crate) fn new_codex_command(configured_path: Option<&str>) -> Result<Command, String> {
    new_codex_command_with_builder(configured_path, |path| new_background_command(path))
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
