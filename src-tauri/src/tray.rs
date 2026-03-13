use std::time::Duration;

use tauri::AppHandle;
use tauri::Manager;

use crate::account_service::refresh_all_usage_internal;
use crate::auth::current_auth_account_id;
use crate::i18n;
use crate::models::AccountSummary;
use crate::models::TrayUsageDisplayMode;
use crate::models::UsageWindow;
use crate::state::AppState;
use crate::store::load_store;

const REFRESH_INTERVAL_SECONDS: u64 = 30;

const TRAY_MENU_OPEN_ID: &str = "tray_open_window";
const TRAY_MENU_QUIT_ID: &str = "tray_quit";

#[cfg(target_os = "macos")]
const TRAY_ID: &str = "codex_tools_status_bar";
#[cfg(target_os = "macos")]
const TRAY_MENU_REFRESH_ID: &str = "tray_refresh_usage";
#[cfg(target_os = "macos")]
const STATUS_BAR_ICON: tauri::image::Image<'_> =
    tauri::include_image!("./icons/codex-tools-statusbar-terminal.png");
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
        TrayUsageDisplayMode::Remaining => remaining_percent(window),
        TrayUsageDisplayMode::Hidden => None,
    }
}

fn read_tray_usage_mode(app: &AppHandle) -> TrayUsageDisplayMode {
    load_store(app)
        .map(|store| store.settings.tray_usage_display_mode)
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
fn build_macos_tray_title(accounts: &[AccountSummary], mode: TrayUsageDisplayMode) -> String {
    if mode == TrayUsageDisplayMode::Hidden {
        return String::new();
    }

    if let Some(current) = accounts.iter().find(|account| account.is_current) {
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
        return format!("5h {five_hour} / 1w {one_week}");
    }

    "5h -- / 1w --".to_string()
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
    let mode = read_tray_usage_mode(app);
    let locale = i18n::app_locale(app);
    let tray = app
        .tray_by_id(TRAY_ID)
        .ok_or_else(|| "状态栏尚未初始化".to_string())?;

    let menu = build_macos_tray_menu(app, accounts, mode)?;
    tray.set_menu(Some(menu))
        .map_err(|e| format!("更新状态栏菜单失败: {e}"))?;
    tray.set_title(Some(build_macos_tray_title(accounts, mode)))
        .map_err(|e| format!("更新状态栏标题失败: {e}"))?;
    tray.set_tooltip(Some(build_macos_tray_tooltip(accounts, mode, locale)))
        .map_err(|e| format!("更新状态栏提示失败: {e}"))?;
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
    let current_account_id = current_auth_account_id();
    let summaries: Vec<AccountSummary> = store
        .accounts
        .iter()
        .map(|account| account.to_summary(current_account_id.as_deref()))
        .collect();
    update_macos_tray_snapshot(app, &summaries)
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn refresh_macos_tray_snapshot(_app: &AppHandle) -> Result<(), String> {
    Ok(())
}

#[cfg(target_os = "macos")]
fn start_macos_tray_refresh_loop(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            let state = app.state::<AppState>();
            if let Ok(summaries) = refresh_all_usage_internal(&app, state.inner(), false).await {
                let _ = update_macos_tray_snapshot(&app, &summaries);
            }
            tokio::time::sleep(Duration::from_secs(REFRESH_INTERVAL_SECONDS)).await;
        }
    });
}

#[cfg(target_os = "macos")]
fn setup_macos_status_bar(app: &AppHandle) -> Result<(), String> {
    use tauri::tray::TrayIconBuilder;

    let mode = read_tray_usage_mode(app);
    let locale = i18n::app_locale(app);
    let store = load_store(app)?;
    let current_account_id = current_auth_account_id();
    let summaries: Vec<AccountSummary> = store
        .accounts
        .iter()
        .map(|account| account.to_summary(current_account_id.as_deref()))
        .collect();
    let menu = build_macos_tray_menu(app, &summaries, mode)?;

    TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        .icon(STATUS_BAR_ICON)
        .icon_as_template(true)
        .title(build_macos_tray_title(&summaries, mode))
        .tooltip(build_macos_tray_tooltip(&summaries, mode, locale))
        .show_menu_on_left_click(true)
        .build(app)
        .map_err(|e| format!("创建 macOS 状态栏失败: {e}"))?;

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
