use tauri::AppHandle;
#[cfg(target_os = "macos")]
use tauri::Manager;

#[cfg(target_os = "macos")]
use crate::account_service::refresh_all_usage_internal;
#[cfg(target_os = "macos")]
use crate::auth::current_auth_account_key;
#[cfg(target_os = "macos")]
use crate::auth::current_auth_variant_key;
use crate::i18n;
use crate::models::mark_current_account_summary;
use crate::models::AccountSummary;
use crate::models::TrayUsageDisplayMode;
use crate::models::UsageSnapshot;
use crate::models::UsageWindow;
#[cfg(target_os = "macos")]
use crate::state::AppState;
use crate::store::load_store;
#[cfg(target_os = "macos")]
use std::time::Duration;

const REFRESH_INTERVAL_SECONDS: u64 = 30;

const TRAY_MENU_OPEN_ID: &str = "tray_open_window";
const TRAY_MENU_QUIT_ID: &str = "tray_quit";

#[cfg(target_os = "macos")]
const TRAY_ID: &str = "codex_tools_status_bar";
#[cfg(target_os = "macos")]
const TRAY_MENU_REFRESH_ID: &str = "tray_refresh_usage";
#[cfg(target_os = "macos")]
const STATUS_BAR_ICON: tauri::image::Image<'_> = tauri::include_image!("./icons/icon.png");
#[cfg(target_os = "windows")]
const TRAY_ID: &str = "codex_tools_tray";
#[cfg(target_os = "windows")]
const WINDOWS_TRAY_ICON: tauri::image::Image<'_> = tauri::include_image!("./icons/32x32.png");

fn format_percent(value: Option<f64>) -> String {
    value
        .map(|percent| percent.clamp(0.0, 100.0).round() as i64)
        .map(|percent| format!("{percent}%"))
        .unwrap_or_else(|| "--".to_string())
}

fn remaining_percent(window: Option<&UsageWindow>) -> Option<f64> {
    window.map(|item| 100.0 - item.used_percent)
}

fn mode_percent(mode: TrayUsageDisplayMode, window: Option<&UsageWindow>) -> Option<f64> {
    match mode {
        TrayUsageDisplayMode::Used => window.map(|item| item.used_percent),
        TrayUsageDisplayMode::Remaining
        | TrayUsageDisplayMode::FiveHourRemaining
        | TrayUsageDisplayMode::OneWeekRemaining => remaining_percent(window),
        TrayUsageDisplayMode::Hidden => None,
    }
}

fn single_window_for_mode(
    mode: TrayUsageDisplayMode,
    usage: Option<&UsageSnapshot>,
) -> Option<&UsageWindow> {
    match mode {
        TrayUsageDisplayMode::FiveHourRemaining => usage.and_then(|usage| usage.five_hour.as_ref()),
        TrayUsageDisplayMode::OneWeekRemaining => usage.and_then(|usage| usage.one_week.as_ref()),
        _ => None,
    }
}

fn only_show_single_window(mode: TrayUsageDisplayMode) -> bool {
    matches!(
        mode,
        TrayUsageDisplayMode::FiveHourRemaining | TrayUsageDisplayMode::OneWeekRemaining
    )
}

fn should_show_macos_status_item(mode: TrayUsageDisplayMode) -> bool {
    mode != TrayUsageDisplayMode::Hidden
}

fn read_tray_title_config(app: &AppHandle) -> (TrayUsageDisplayMode, bool) {
    load_store(app)
        .map(|store| {
            (
                store.settings.tray_usage_display_mode,
                store.settings.tray_usage_title_show_window_labels,
            )
        })
        .unwrap_or_default()
}

#[cfg(target_os = "macos")]
fn tray_account_usage_line(
    account: &AccountSummary,
    mode: TrayUsageDisplayMode,
    locale: crate::models::AppLocale,
) -> String {
    let current_prefix = if account.is_current {
        i18n::tray_current_prefix(locale)
    } else {
        String::new()
    };
    if mode == TrayUsageDisplayMode::Hidden {
        return format!("{current_prefix}{}", account.label);
    }

    if only_show_single_window(mode) {
        let selected_window = format_percent(mode_percent(
            mode,
            single_window_for_mode(mode, account.usage.as_ref()),
        ));
        let remaining_label = i18n::tray_usage_mode_label(locale, TrayUsageDisplayMode::Remaining);
        return format!(
            "{current_prefix}{} | {remaining_label} {selected_window}",
            account.label
        );
    }

    let five_hour = format_percent(mode_percent(
        mode,
        account
            .usage
            .as_ref()
            .and_then(|usage| usage.five_hour.as_ref()),
    ));

    let one_week = format_percent(mode_percent(
        mode,
        account
            .usage
            .as_ref()
            .and_then(|usage| usage.one_week.as_ref()),
    ));

    let mode_label = i18n::tray_usage_mode_label(locale, mode);
    format!(
        "{current_prefix}{} | 5h{mode_label} {five_hour} | 1week{mode_label} {one_week}",
        account.label
    )
}

#[cfg(target_os = "macos")]
fn build_macos_tray_title(
    accounts: &[AccountSummary],
    mode: TrayUsageDisplayMode,
    show_window_labels: bool,
) -> String {
    if mode == TrayUsageDisplayMode::Hidden {
        return String::new();
    }

    if let Some(current) = accounts.iter().find(|account| account.is_current) {
        if only_show_single_window(mode) {
            let selected_window = format_percent(mode_percent(
                mode,
                single_window_for_mode(mode, current.usage.as_ref()),
            ));
            if !show_window_labels {
                return selected_window;
            }
            return match mode {
                TrayUsageDisplayMode::FiveHourRemaining => format!("5h {selected_window}"),
                TrayUsageDisplayMode::OneWeekRemaining => format!("1w {selected_window}"),
                _ => unreachable!("single-window modes are handled above"),
            };
        }

        let five_hour = format_percent(mode_percent(
            mode,
            current
                .usage
                .as_ref()
                .and_then(|usage| usage.five_hour.as_ref()),
        ));
        let one_week = format_percent(mode_percent(
            mode,
            current
                .usage
                .as_ref()
                .and_then(|usage| usage.one_week.as_ref()),
        ));
        return if show_window_labels {
            format!("5h {five_hour} / 1w {one_week}")
        } else {
            format!("{five_hour} / {one_week}")
        };
    }

    if only_show_single_window(mode) {
        if !show_window_labels {
            return "--".to_string();
        }
        return match mode {
            TrayUsageDisplayMode::FiveHourRemaining => "5h --".to_string(),
            TrayUsageDisplayMode::OneWeekRemaining => "1w --".to_string(),
            _ => unreachable!("single-window modes are handled above"),
        };
    }

    if show_window_labels {
        "5h -- / 1w --".to_string()
    } else {
        "-- / --".to_string()
    }
}

#[cfg(target_os = "macos")]
fn build_macos_tray_tooltip(
    accounts: &[AccountSummary],
    mode: TrayUsageDisplayMode,
    locale: crate::models::AppLocale,
) -> String {
    let mut lines = vec![i18n::tray_usage_heading(locale).to_string()];
    lines.push(format!(
        "{}: {}",
        i18n::tray_display_mode_label(locale),
        i18n::tray_usage_mode_label(locale, mode)
    ));

    if let Some(current) = accounts.iter().find(|account| account.is_current) {
        lines.push(format!(
            "{}: {}",
            i18n::tray_current_label(locale),
            tray_account_usage_line(current, mode, locale)
        ));
    } else {
        lines.push(format!(
            "{}: {}",
            i18n::tray_current_label(locale),
            i18n::tray_no_current(locale)
        ));
    }

    if accounts.is_empty() {
        lines.push(i18n::tray_no_accounts(locale).to_string());
        return lines.join("\n");
    }

    lines.push(i18n::tray_all_accounts(locale, accounts.len()));
    for account in accounts.iter().take(8) {
        lines.push(format!(
            "• {}",
            tray_account_usage_line(account, mode, locale)
        ));
    }
    if accounts.len() > 8 {
        lines.push(i18n::tray_more_accounts(locale, accounts.len() - 8));
    }

    lines.join("\n")
}

#[cfg(target_os = "macos")]
fn build_macos_tray_menu(
    app: &AppHandle,
    accounts: &[AccountSummary],
    mode: TrayUsageDisplayMode,
) -> Result<tauri::menu::Menu<tauri::Wry>, String> {
    use tauri::menu::Menu;
    use tauri::menu::MenuItem;
    use tauri::menu::PredefinedMenuItem;

    let locale = i18n::app_locale(app);
    let menu = Menu::new(app).map_err(|e| format!("创建状态栏菜单失败: {e}"))?;

    let header_text = format!(
        "{} ({})",
        i18n::tray_usage_heading(locale),
        i18n::tray_usage_mode_label(locale, mode)
    );
    let header = MenuItem::with_id(app, "tray_header", header_text, false, None::<&str>)
        .map_err(|e| format!("创建状态栏菜单项失败: {e}"))?;
    menu.append(&header)
        .map_err(|e| format!("写入状态栏菜单失败: {e}"))?;

    let current_line = if let Some(current) = accounts.iter().find(|account| account.is_current) {
        format!(
            "{}: {}",
            i18n::tray_current_account_label(locale),
            tray_account_usage_line(current, mode, locale)
        )
    } else {
        format!(
            "{}: {}",
            i18n::tray_current_account_label(locale),
            i18n::tray_no_current(locale)
        )
    };
    let current_item = MenuItem::with_id(
        app,
        "tray_current_summary",
        current_line,
        false,
        None::<&str>,
    )
    .map_err(|e| format!("创建状态栏菜单项失败: {e}"))?;
    menu.append(&current_item)
        .map_err(|e| format!("写入状态栏菜单失败: {e}"))?;

    let separator =
        PredefinedMenuItem::separator(app).map_err(|e| format!("创建状态栏分隔符失败: {e}"))?;
    menu.append(&separator)
        .map_err(|e| format!("写入状态栏菜单失败: {e}"))?;

    if accounts.is_empty() {
        let empty = MenuItem::with_id(
            app,
            "tray_accounts_empty",
            i18n::tray_empty_accounts(locale),
            false,
            None::<&str>,
        )
        .map_err(|e| format!("创建状态栏菜单项失败: {e}"))?;
        menu.append(&empty)
            .map_err(|e| format!("写入状态栏菜单失败: {e}"))?;
    } else {
        for (index, account) in accounts.iter().enumerate() {
            let id = format!("tray_account_{index}");
            let line_item = MenuItem::with_id(
                app,
                id,
                tray_account_usage_line(account, mode, locale),
                false,
                None::<&str>,
            )
            .map_err(|e| format!("创建状态栏菜单项失败: {e}"))?;
            menu.append(&line_item)
                .map_err(|e| format!("写入状态栏菜单失败: {e}"))?;
        }
    }

    let separator =
        PredefinedMenuItem::separator(app).map_err(|e| format!("创建状态栏分隔符失败: {e}"))?;
    menu.append(&separator)
        .map_err(|e| format!("写入状态栏菜单失败: {e}"))?;

    let refresh = MenuItem::with_id(
        app,
        TRAY_MENU_REFRESH_ID,
        i18n::tray_refresh_now(locale),
        true,
        None::<&str>,
    )
    .map_err(|e| format!("创建状态栏菜单项失败: {e}"))?;
    let open = MenuItem::with_id(
        app,
        TRAY_MENU_OPEN_ID,
        i18n::tray_open_app(locale),
        true,
        None::<&str>,
    )
    .map_err(|e| format!("创建状态栏菜单项失败: {e}"))?;
    let quit = MenuItem::with_id(
        app,
        TRAY_MENU_QUIT_ID,
        i18n::tray_quit(locale),
        true,
        None::<&str>,
    )
    .map_err(|e| format!("创建状态栏菜单项失败: {e}"))?;

    menu.append(&refresh)
        .map_err(|e| format!("写入状态栏菜单失败: {e}"))?;
    menu.append(&open)
        .map_err(|e| format!("写入状态栏菜单失败: {e}"))?;
    menu.append(&quit)
        .map_err(|e| format!("写入状态栏菜单失败: {e}"))?;

    Ok(menu)
}

#[cfg(target_os = "macos")]
pub(crate) fn update_macos_tray_snapshot(
    app: &AppHandle,
    accounts: &[AccountSummary],
) -> Result<(), String> {
    let (mode, show_window_labels) = read_tray_title_config(app);
    let locale = i18n::app_locale(app);
    let tray = app
        .tray_by_id(TRAY_ID)
        .ok_or_else(|| "状态栏尚未初始化".to_string())?;

    if !should_show_macos_status_item(mode) {
        tray.set_visible(false)
            .map_err(|e| format!("隐藏状态栏失败: {e}"))?;
        return Ok(());
    }

    let menu = build_macos_tray_menu(app, accounts, mode)?;
    let title = build_macos_tray_title(accounts, mode, show_window_labels);
    #[cfg(debug_assertions)]
    log_macos_status_bar_render("update", accounts, &title);
    tray.set_menu(Some(menu))
        .map_err(|e| format!("更新状态栏菜单失败: {e}"))?;
    tray.set_title(Some(title))
        .map_err(|e| format!("更新状态栏标题失败: {e}"))?;
    tray.set_tooltip(Some(build_macos_tray_tooltip(accounts, mode, locale)))
        .map_err(|e| format!("更新状态栏提示失败: {e}"))?;
    tray.set_visible(true)
        .map_err(|e| format!("显示状态栏失败: {e}"))?;
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn update_macos_tray_snapshot(
    _app: &AppHandle,
    _accounts: &[AccountSummary],
) -> Result<(), String> {
    Ok(())
}

#[cfg(target_os = "macos")]
pub(crate) fn refresh_macos_tray_snapshot(app: &AppHandle) -> Result<(), String> {
    let store = load_store(app)?;
    let current_account_key = current_auth_account_key();
    let current_variant_key = current_auth_variant_key();
    let mut summaries: Vec<AccountSummary> = store
        .accounts
        .iter()
        .map(|account| {
            account.to_summary(
                current_account_key.as_deref(),
                current_variant_key.as_deref(),
            )
        })
        .collect();
    mark_current_account_summary(
        &mut summaries,
        current_account_key.as_deref(),
        store.settings.active_account_id.as_deref(),
    );
    #[cfg(debug_assertions)]
    log_macos_status_bar_resolution(
        "refresh",
        &store,
        &summaries,
        current_account_key.as_deref(),
        current_variant_key.as_deref(),
    );
    update_macos_tray_snapshot(app, &summaries)
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn refresh_macos_tray_snapshot(_app: &AppHandle) -> Result<(), String> {
    Ok(())
}

#[cfg(target_os = "macos")]
fn main_window_is_visible(app: &AppHandle) -> bool {
    app.get_webview_window("main")
        .and_then(|window| window.is_visible().ok())
        .unwrap_or(false)
}

#[cfg(all(target_os = "macos", debug_assertions))]
fn log_macos_status_bar_resolution(
    context: &str,
    store: &crate::models::AccountsStore,
    summaries: &[AccountSummary],
    current_account_key: Option<&str>,
    current_variant_key: Option<&str>,
) {
    let matched_current = summaries.iter().any(|account| account.is_current);
    if context != "setup" && matched_current {
        return;
    }

    let active_account = store
        .settings
        .active_account_id
        .as_deref()
        .and_then(|active_id| {
            store
                .accounts
                .iter()
                .find(|account| account.id == active_id)
        });
    let active_usage_cached = active_account
        .map(|account| account.usage.is_some() && account.usage_error.is_none())
        .unwrap_or(false);
    let account_group_matches = current_account_key
        .map(|current_account_key| {
            store
                .accounts
                .iter()
                .filter(|account| account.account_key() == current_account_key)
                .count()
        })
        .unwrap_or(0);
    let account_variant_matches = current_variant_key
        .map(|current_variant_key| {
            store
                .accounts
                .iter()
                .filter(|account| account.variant_key() == current_variant_key)
                .count()
        })
        .unwrap_or(0);

    log::info!(
        "AUTH_DIAG tray context={context} stored_accounts={} auth_group_key_present={} auth_variant_key_present={} account_group_matches={} account_variant_matches={} matched_current={} active_id_present={} active_id_resolves={} active_usage_cached={}",
        store.accounts.len(),
        current_account_key.is_some(),
        current_variant_key.is_some(),
        account_group_matches,
        account_variant_matches,
        matched_current,
        store.settings.active_account_id.is_some(),
        active_account.is_some(),
        active_usage_cached,
    );
}

#[cfg(all(target_os = "macos", debug_assertions))]
fn log_macos_status_bar_render(context: &str, accounts: &[AccountSummary], title: &str) {
    let current = accounts.iter().find(|account| account.is_current);
    let current_usage_cached = current
        .map(|account| account.usage.is_some() && account.usage_error.is_none())
        .unwrap_or(false);
    let current_usage_error = current
        .and_then(|account| account.usage_error.as_deref())
        .is_some();

    log::info!(
        "AUTH_DIAG tray_render context={context} accounts={} current_present={} current_usage_cached={} current_usage_error={} title_has_placeholder={}",
        accounts.len(),
        current.is_some(),
        current_usage_cached,
        current_usage_error,
        title.contains("--"),
    );
}

#[cfg(target_os = "macos")]
fn start_macos_tray_refresh_loop(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            // 初始状态栏直接使用本地缓存，首轮等待一个周期，避免与前端首屏
            // 刷新及后台认证检查重复请求。
            tokio::time::sleep(Duration::from_secs(REFRESH_INTERVAL_SECONDS)).await;
            if !main_window_is_visible(&app) {
                let state = app.state::<AppState>();
                if let Ok(summaries) = refresh_all_usage_internal(&app, state.inner(), false).await
                {
                    let _ = update_macos_tray_snapshot(&app, &summaries);
                }
            }
        }
    });
}

#[cfg(target_os = "macos")]
fn setup_macos_status_bar(app: &AppHandle) -> Result<(), String> {
    use tauri::tray::TrayIconBuilder;

    let (mode, show_window_labels) = read_tray_title_config(app);
    let locale = i18n::app_locale(app);
    let store = load_store(app)?;
    let current_account_key = current_auth_account_key();
    let current_variant_key = current_auth_variant_key();
    let mut summaries: Vec<AccountSummary> = store
        .accounts
        .iter()
        .map(|account| {
            account.to_summary(
                current_account_key.as_deref(),
                current_variant_key.as_deref(),
            )
        })
        .collect();
    mark_current_account_summary(
        &mut summaries,
        current_account_key.as_deref(),
        store.settings.active_account_id.as_deref(),
    );
    #[cfg(debug_assertions)]
    log_macos_status_bar_resolution(
        "setup",
        &store,
        &summaries,
        current_account_key.as_deref(),
        current_variant_key.as_deref(),
    );
    let menu = build_macos_tray_menu(app, &summaries, mode)?;
    let title = build_macos_tray_title(&summaries, mode, show_window_labels);
    #[cfg(debug_assertions)]
    log_macos_status_bar_render("setup", &summaries, &title);

    let tray = TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        .icon(STATUS_BAR_ICON)
        .icon_as_template(false)
        .title(title)
        .tooltip(build_macos_tray_tooltip(&summaries, mode, locale))
        .show_menu_on_left_click(true)
        .build(app)
        .map_err(|e| format!("创建 macOS 状态栏失败: {e}"))?;
    tray.set_visible(should_show_macos_status_item(mode))
        .map_err(|e| format!("设置状态栏可见性失败: {e}"))?;

    start_macos_tray_refresh_loop(app.clone());
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn setup_macos_status_bar(_app: &AppHandle) -> Result<(), String> {
    Ok(())
}

#[cfg(target_os = "windows")]
fn build_windows_tray_menu(app: &AppHandle) -> Result<tauri::menu::Menu<tauri::Wry>, String> {
    use tauri::menu::Menu;
    use tauri::menu::MenuItem;
    use tauri::menu::PredefinedMenuItem;

    let locale = i18n::app_locale(app);
    let menu = Menu::new(app).map_err(|e| format!("创建系统托盘菜单失败: {e}"))?;
    let open = MenuItem::with_id(
        app,
        TRAY_MENU_OPEN_ID,
        i18n::tray_open_app(locale),
        true,
        None::<&str>,
    )
    .map_err(|e| format!("创建系统托盘菜单项失败: {e}"))?;
    let quit = MenuItem::with_id(
        app,
        TRAY_MENU_QUIT_ID,
        i18n::tray_quit(locale),
        true,
        None::<&str>,
    )
    .map_err(|e| format!("创建系统托盘菜单项失败: {e}"))?;
    let separator =
        PredefinedMenuItem::separator(app).map_err(|e| format!("创建系统托盘分隔符失败: {e}"))?;

    menu.append(&open)
        .map_err(|e| format!("写入系统托盘菜单失败: {e}"))?;
    menu.append(&separator)
        .map_err(|e| format!("写入系统托盘菜单失败: {e}"))?;
    menu.append(&quit)
        .map_err(|e| format!("写入系统托盘菜单失败: {e}"))?;

    Ok(menu)
}

#[cfg(target_os = "windows")]
fn setup_windows_tray(app: &AppHandle) -> Result<(), String> {
    use tauri::tray::MouseButton;
    use tauri::tray::TrayIconBuilder;
    use tauri::tray::TrayIconEvent;

    let menu = build_windows_tray_menu(app)?;

    TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        .icon(WINDOWS_TRAY_ICON)
        .tooltip("Codex Tools")
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| match event {
            TrayIconEvent::Click {
                button: MouseButton::Left,
                ..
            }
            | TrayIconEvent::DoubleClick {
                button: MouseButton::Left,
                ..
            } => crate::restore_main_window(tray.app_handle()),
            _ => {}
        })
        .build(app)
        .map_err(|e| format!("创建 Windows 系统托盘失败: {e}"))?;

    Ok(())
}

#[cfg(not(target_os = "windows"))]
#[allow(dead_code)]
fn setup_windows_tray(_app: &AppHandle) -> Result<(), String> {
    Ok(())
}

pub(crate) fn setup_system_tray(app: &AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        return setup_macos_status_bar(app);
    }

    #[cfg(target_os = "windows")]
    {
        return setup_windows_tray(app);
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = app;
        Ok(())
    }
}

pub(crate) fn handle_status_bar_menu_event(app: &AppHandle, event: tauri::menu::MenuEvent) {
    let id = event.id().as_ref();
    if id == TRAY_MENU_QUIT_ID {
        app.exit(0);
        return;
    }

    if id == TRAY_MENU_OPEN_ID {
        crate::restore_main_window(app);
        return;
    }

    #[cfg(target_os = "macos")]
    if id == TRAY_MENU_REFRESH_ID {
        let app_handle = app.clone();
        tauri::async_runtime::spawn(async move {
            let state = app_handle.state::<AppState>();
            if let Ok(summaries) =
                refresh_all_usage_internal(&app_handle, state.inner(), true).await
            {
                let _ = update_macos_tray_snapshot(&app_handle, &summaries);
            }
        });
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::build_macos_tray_title;
    use super::should_show_macos_status_item;
    use super::tray_account_usage_line;
    use crate::models::AccountSummary;
    use crate::models::AppLocale;
    use crate::models::TrayUsageDisplayMode;
    use crate::models::UsageSnapshot;
    use crate::models::UsageWindow;

    fn current_account_with_usage() -> AccountSummary {
        AccountSummary {
            id: "current".to_string(),
            label: "Current account".to_string(),
            source_kind: Default::default(),
            email: None,
            account_key: "account-key".to_string(),
            account_id: "account-id".to_string(),
            plan_type: Some("pro".to_string()),
            subscription_active_until: None,
            api_base_url: None,
            model_name: None,
            balance_text: None,
            profile_auth_ready: false,
            profile_config_ready: false,
            profile_integrity_error: None,
            profile_last_validated_at: None,
            profile_last_validation_error: None,
            added_at: 0,
            updated_at: 0,
            usage: Some(UsageSnapshot {
                fetched_at: 0,
                plan_type: Some("pro".to_string()),
                five_hour: Some(UsageWindow {
                    used_percent: 60.0,
                    window_seconds: 18_000,
                    reset_at: None,
                }),
                one_week: Some(UsageWindow {
                    used_percent: 40.0,
                    window_seconds: 604_800,
                    reset_at: None,
                }),
                credits: None,
                reset_credits: None,
            }),
            usage_error: None,
            auth_refresh_blocked: false,
            auth_refresh_error: None,
            api_proxy_enabled: false,
            is_current: true,
        }
    }

    #[test]
    fn one_week_remaining_mode_shows_only_the_one_week_value() {
        let account = current_account_with_usage();

        assert_eq!(
            build_macos_tray_title(
                std::slice::from_ref(&account),
                TrayUsageDisplayMode::OneWeekRemaining,
                false,
            ),
            "60%"
        );
        assert_eq!(
            build_macos_tray_title(
                std::slice::from_ref(&account),
                TrayUsageDisplayMode::OneWeekRemaining,
                true,
            ),
            "1w 60%"
        );

        let usage_line = tray_account_usage_line(
            &account,
            TrayUsageDisplayMode::OneWeekRemaining,
            AppLocale::EnUs,
        );
        assert!(usage_line.contains("60%"));
        assert!(!usage_line.contains("40%"));
        assert!(!usage_line.contains("5h"));
    }

    #[test]
    fn one_week_remaining_mode_keeps_a_one_week_placeholder_without_current_account() {
        assert_eq!(
            build_macos_tray_title(&[], TrayUsageDisplayMode::OneWeekRemaining, false),
            "--"
        );
        assert_eq!(
            build_macos_tray_title(&[], TrayUsageDisplayMode::OneWeekRemaining, true),
            "1w --"
        );
    }

    #[test]
    fn window_labels_can_be_hidden_for_combined_usage_display() {
        let account = current_account_with_usage();

        assert_eq!(
            build_macos_tray_title(
                std::slice::from_ref(&account),
                TrayUsageDisplayMode::Remaining,
                false,
            ),
            "40% / 60%"
        );
        assert_eq!(
            build_macos_tray_title(&[account], TrayUsageDisplayMode::Remaining, true),
            "5h 40% / 1w 60%"
        );
    }

    #[test]
    fn hidden_mode_hides_the_entire_macos_status_item() {
        assert!(!should_show_macos_status_item(TrayUsageDisplayMode::Hidden));
        assert!(should_show_macos_status_item(
            TrayUsageDisplayMode::OneWeekRemaining
        ));
    }
}
