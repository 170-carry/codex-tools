use tauri::AppHandle;
use tauri_plugin_autostart::ManagerExt as _;

use crate::cli;
use crate::models::AppSettings;
use crate::models::AppSettingsPatch;
use crate::state::AppState;
use crate::store::load_store;
use crate::store::save_store;

/// 读取应用设置（前端设置页使用）。
pub(crate) async fn get_app_settings_internal(
    app: &AppHandle,
    state: &AppState,
) -> Result<AppSettings, String> {
    let _guard = state.store_lock.lock().await;
    let store = load_store(app)?;
    Ok(store.settings)
}

/// 更新应用设置并持久化：
/// - 存储到 `accounts.json.settings`
/// - 若涉及开机启动开关，立即同步到系统。
pub(crate) async fn update_app_settings_internal(
    app: &AppHandle,
    state: &AppState,
    patch: AppSettingsPatch,
) -> Result<AppSettings, String> {
    let mut launch_at_startup_to_apply = None;
    let settings = {
        let _guard = state.store_lock.lock().await;
        let mut store = load_store(app)?;

        if let Some(value) = patch.launch_at_startup {
            store.settings.launch_at_startup = value;
            launch_at_startup_to_apply = Some(value);
        }
        if let Some(value) = patch.tray_usage_display_mode {
            store.settings.tray_usage_display_mode = value;
        }
        if let Some(value) = patch.launch_codex_after_switch {
            store.settings.launch_codex_after_switch = value;
        }
        if let Some(value) = patch.codex_launch_path {
            let normalized = normalize_codex_launch_path(value);
            cli::validate_configured_codex_path(normalized.as_deref())?;
            store.settings.codex_launch_path = normalized;
        }
        if let Some(value) = patch.sync_opencode_openai_auth {
            store.settings.sync_opencode_openai_auth = value;
        }
        if let Some(value) = patch.restart_opencode_desktop_on_switch {
            store.settings.restart_opencode_desktop_on_switch = value;
        }
        if let Some(value) = patch.restart_editors_on_switch {
            store.settings.restart_editors_on_switch = value;
        }
        if let Some(value) = patch.restart_editor_targets {
            store.settings.restart_editor_targets = value;
        }
        if let Some(value) = patch.auto_start_api_proxy {
            store.settings.auto_start_api_proxy = value;
        }
        if let Some(value) = patch.api_proxy_port {
            store.settings.api_proxy_port = value;
        }
        if let Some(value) = patch.remote_servers {
            store.settings.remote_servers = value;
        }
        if let Some(value) = patch.locale {
            store.settings.locale = value;
        }
        let settings = store.settings.clone();
        save_store(app, &store)?;
        settings
    };

    if let Some(value) = launch_at_startup_to_apply {
        set_system_autostart(app, value)?;
    }

    Ok(settings)
}

/// 启动时根据本地设置校准系统开机启动状态，避免“设置与系统实际状态不一致”。
pub(crate) fn sync_autostart_from_store(app: &AppHandle) -> Result<(), String> {
    let settings = load_store(app)?.settings;
    let current_enabled = app
        .autolaunch()
        .is_enabled()
        .map_err(|e| format!("读取开机启动状态失败: {e}"))?;

    if current_enabled != settings.launch_at_startup {
        set_system_autostart(app, settings.launch_at_startup)?;
    }

    Ok(())
}

fn set_system_autostart(app: &AppHandle, enabled: bool) -> Result<(), String> {
    if enabled {
        app.autolaunch()
            .enable()
            .map_err(|e| format!("开启开机启动失败: {e}"))
    } else {
        app.autolaunch()
            .disable()
            .map_err(|e| format!("关闭开机启动失败: {e}"))
    }
}

fn normalize_codex_launch_path(value: Option<String>) -> Option<String> {
    value.and_then(|raw| {
        let trimmed = raw.trim();
        let unquoted = trimmed
            .strip_prefix('"')
            .and_then(|item| item.strip_suffix('"'))
            .or_else(|| {
                trimmed
                    .strip_prefix('\'')
                    .and_then(|item| item.strip_suffix('\''))
            })
            .unwrap_or(trimmed)
            .trim();

        if unquoted.is_empty() {
            None
        } else {
            Some(unquoted.to_string())
        }
    })
}
