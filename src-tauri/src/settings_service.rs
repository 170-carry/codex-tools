use tauri::AppHandle;
use tauri_plugin_autostart::ManagerExt as _;

use crate::cli;
use crate::models::normalize_api_proxy_sequential_five_hour_limit_percent;
use crate::models::AppSettings;
use crate::models::AppSettingsPatch;
use crate::proxy_service::sanitize_api_proxy_disabled_models_for_settings;
use crate::state::AppState;
use crate::store::load_store;
use crate::store::save_store;

static APPLIED_PROXY_URL: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

/// 读取应用设置（前端设置页使用）。
pub(crate) async fn get_app_settings_internal(
    app: &AppHandle,
    state: &AppState,
) -> Result<AppSettings, String> {
    let _guard = state.store_lock.lock().await;
    let mut store = load_store(app)?;
    if store
        .settings
        .codex_launch_path
        .as_deref()
        .is_some_and(should_discard_codex_launch_path)
    {
        store.settings.codex_launch_path = None;
        save_store(app, &store)?;
    }
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
    let mut proxy_url_to_apply = None;
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
        if let Some(value) = patch.smart_switch_include_api {
            store.settings.smart_switch_include_api = value;
        }
        if let Some(value) = patch.codex_launch_path {
            store.settings.codex_launch_path = normalize_codex_launch_path_for_storage(value)?;
        }
        if let Some(value) = patch.proxy_url {
            store.settings.proxy_url = normalize_proxy_url_for_storage(value)?;
            proxy_url_to_apply = Some(store.settings.proxy_url.clone());
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
        if let Some(value) = patch.api_proxy_load_balance_mode {
            store.settings.api_proxy_load_balance_mode = value;
        }
        if let Some(value) = patch.api_proxy_sequential_five_hour_limit_percent {
            store.settings.api_proxy_sequential_five_hour_limit_percent =
                normalize_api_proxy_sequential_five_hour_limit_percent(value);
        }
        if let Some(value) = patch.api_proxy_disabled_models {
            store.settings.api_proxy_disabled_models =
                sanitize_api_proxy_disabled_models_for_settings(value);
        }
        if let Some(value) = patch.remote_servers {
            store.settings.remote_servers = value;
        }
        if let Some(value) = patch.locale {
            store.settings.locale = value;
        }
        if let Some(value) = patch.skipped_update_version {
            store.settings.skipped_update_version = value;
        }

        let settings = store.settings.clone();
        save_store(app, &store)?;
        settings
    };

    if let Some(value) = launch_at_startup_to_apply {
        set_system_autostart(app, value)?;
    }
    if let Some(value) = proxy_url_to_apply {
        apply_proxy_environment(value.as_deref());
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

/// 启动时把保存的代理同步到当前进程环境，供后续 HTTP 客户端继承。
pub(crate) fn sync_proxy_environment_from_store(app: &AppHandle) -> Result<(), String> {
    let settings = load_store(app)?.settings;
    apply_proxy_environment(settings.proxy_url.as_deref());
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

fn normalize_codex_launch_path_for_storage(
    value: Option<String>,
) -> Result<Option<String>, String> {
    let normalized = normalize_codex_launch_path(value);
    if normalized
        .as_deref()
        .is_some_and(should_discard_codex_launch_path)
    {
        return Ok(None);
    }

    cli::validate_configured_codex_path(normalized.as_deref())?;
    Ok(normalized)
}

fn normalize_proxy_url(value: Option<String>) -> Option<String> {
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

fn normalize_proxy_url_for_storage(value: Option<String>) -> Result<Option<String>, String> {
    let normalized = normalize_proxy_url(value);
    let Some(proxy_url) = normalized else {
        return Ok(None);
    };

    let scheme = proxy_url
        .split_once("://")
        .map(|(scheme, _)| scheme.to_ascii_lowercase())
        .ok_or_else(|| "代理地址需要包含协议，例如 http://127.0.0.1:7890".to_string())?;
    if !matches!(
        scheme.as_str(),
        "http" | "https" | "socks4" | "socks4a" | "socks5" | "socks5h"
    ) {
        return Err("代理地址仅支持 HTTP、HTTPS 或 SOCKS 协议".to_string());
    }

    Ok(Some(proxy_url))
}

fn apply_proxy_environment(proxy_url: Option<&str>) {
    const PROXY_ENV_NAMES: [&str; 6] = [
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "ALL_PROXY",
        "http_proxy",
        "https_proxy",
        "all_proxy",
    ];

    let Ok(mut applied) = APPLIED_PROXY_URL.lock() else {
        return;
    };

    match proxy_url {
        Some(proxy_url) => {
            for name in PROXY_ENV_NAMES {
                std::env::set_var(name, proxy_url);
            }
            *applied = Some(proxy_url.to_string());
        }
        None => {
            if let Some(previous) = applied.take() {
                for name in PROXY_ENV_NAMES {
                    if std::env::var(name).as_deref() == Ok(previous.as_str()) {
                        std::env::remove_var(name);
                    }
                }
            }
        }
    }
}

fn should_discard_codex_launch_path(path: &str) -> bool {
    cli::is_windows_store_codex_path(std::path::Path::new(path))
        && cli::has_windows_store_codex_app()
}

#[cfg(test)]
mod tests {
    use super::apply_proxy_environment;
    use super::normalize_proxy_url_for_storage;
    use std::sync::Mutex;

    static ENV_TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn normalize_proxy_url_trims_and_unquotes_http_proxy() {
        assert_eq!(
            normalize_proxy_url_for_storage(Some(" \"http://127.0.0.1:7890\" ".to_string()))
                .expect("normalize proxy"),
            Some("http://127.0.0.1:7890".to_string())
        );
    }

    #[test]
    fn normalize_proxy_url_treats_blank_as_cleared() {
        assert_eq!(
            normalize_proxy_url_for_storage(Some("   ".to_string()))
                .expect("normalize proxy"),
            None
        );
        assert_eq!(
            normalize_proxy_url_for_storage(None).expect("normalize proxy"),
            None
        );
    }

    #[test]
    fn normalize_proxy_url_accepts_socks_proxy() {
        assert_eq!(
            normalize_proxy_url_for_storage(Some("socks5://127.0.0.1:1080".to_string()))
                .expect("normalize proxy"),
            Some("socks5://127.0.0.1:1080".to_string())
        );
    }

    #[test]
    fn normalize_proxy_url_rejects_unsupported_scheme() {
        let error =
            normalize_proxy_url_for_storage(Some("ftp://127.0.0.1:21".to_string()))
                .expect_err("reject proxy");

        assert!(error.contains("HTTP、HTTPS 或 SOCKS"));
    }

    #[test]
    fn apply_proxy_environment_sets_and_clears_only_its_own_values() {
        let _guard = ENV_TEST_LOCK.lock().expect("lock env test");
        const NAMES: [&str; 6] = [
            "HTTP_PROXY",
            "HTTPS_PROXY",
            "ALL_PROXY",
            "http_proxy",
            "https_proxy",
            "all_proxy",
        ];
        let originals: Vec<_> = NAMES
            .iter()
            .map(|name| (*name, std::env::var_os(name)))
            .collect();

        for name in NAMES {
            std::env::remove_var(name);
        }

        let result = std::panic::catch_unwind(|| {
            apply_proxy_environment(Some("http://127.0.0.1:7890"));
            assert_eq!(
                std::env::var("HTTP_PROXY").as_deref(),
                Ok("http://127.0.0.1:7890")
            );
            assert_eq!(
                std::env::var("HTTPS_PROXY").as_deref(),
                Ok("http://127.0.0.1:7890")
            );

            apply_proxy_environment(None);
            assert!(std::env::var_os("HTTP_PROXY").is_none());
            assert!(std::env::var_os("HTTPS_PROXY").is_none());
        });

        for (name, value) in originals {
            if let Some(value) = value {
                std::env::set_var(name, value);
            } else {
                std::env::remove_var(name);
            }
        }

        if let Err(error) = result {
            std::panic::resume_unwind(error);
        }
    }
}
