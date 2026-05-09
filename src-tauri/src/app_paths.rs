use std::path::PathBuf;

#[cfg(feature = "desktop")]
use tauri::AppHandle;
#[cfg(feature = "desktop")]
use tauri::Manager;

const DEV_APP_DATA_DIR_ENV: &str = "CODEX_TOOLS_DEV_DATA_DIR";
const DEV_CODEX_DIR_ENV: &str = "CODEX_TOOLS_DEV_CODEX_DIR";

fn env_path(name: &str) -> Option<PathBuf> {
    let value = std::env::var(name).ok()?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(PathBuf::from(trimmed))
}

#[cfg(feature = "desktop")]
pub(crate) fn app_data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    if cfg!(debug_assertions) {
        if let Some(path) = env_path(DEV_APP_DATA_DIR_ENV) {
            return Ok(path);
        }
    }

    app.path()
        .app_data_dir()
        .map_err(|error| format!("无法获取应用数据目录: {error}"))
}

pub(crate) fn codex_dir() -> Result<PathBuf, String> {
    if cfg!(debug_assertions) {
        if let Some(path) = env_path(DEV_CODEX_DIR_ENV) {
            return Ok(path);
        }
    }

    let home = dirs::home_dir().ok_or_else(|| "无法读取 HOME 目录".to_string())?;
    Ok(home.join(".codex"))
}

pub(crate) fn codex_auth_path() -> Result<PathBuf, String> {
    Ok(codex_dir()?.join("auth.json"))
}

pub(crate) fn codex_config_path() -> Result<PathBuf, String> {
    Ok(codex_dir()?.join("config.toml"))
}
