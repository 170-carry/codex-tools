use tauri::AppHandle;
#[cfg(target_os = "macos")]
use tauri::Manager;

#[cfg(target_os = "macos")]
use crate::account_service::refresh_all_usage_coordinated;
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
#[cfg(target_os = "windows")]
use crate::models::WindowsTaskbarWidgetPlacement;
use crate::models::WindowsTrayIconStyle;
#[cfg(target_os = "macos")]
use crate::state::AppState;
use crate::store::load_store;
#[cfg(target_os = "macos")]
use crate::tray_visual::{
    render_tray_visual, tray_visual_dimensions, TrayVisualPlatform, TrayVisualStatus,
};
#[cfg(target_os = "windows")]
use crate::windows_taskbar_widget::WindowsTaskbarWidgetSnapshot;
#[cfg(target_os = "windows")]
use crate::windows_taskbar_widget::WindowsWidgetStatus;
#[cfg(target_os = "windows")]
use crate::windows_tray_icon::{render_windows_tray_icon, static_codex_tools_icon};
#[cfg(target_os = "macos")]
use std::time::Duration;

#[cfg(target_os = "macos")]
const REFRESH_INTERVAL_SECONDS: u64 = 30;
#[cfg(target_os = "windows")]
const WINDOWS_WIDGET_STALE_AFTER_SECONDS: i64 = 10 * 60;

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

fn should_show_usage_surface(mode: TrayUsageDisplayMode) -> bool {
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
fn read_macos_tray_icon_style(app: &AppHandle) -> WindowsTrayIconStyle {
    load_store(app)
        .map(|store| store.settings.windows_tray_icon_style)
        .unwrap_or_default()
}

#[cfg(target_os = "windows")]
#[derive(Debug, Clone, Copy)]
struct WindowsUsageSurfaceConfig {
    mode: TrayUsageDisplayMode,
    show_window_labels: bool,
    tray_icon_style: WindowsTrayIconStyle,
    widget_placement: WindowsTaskbarWidgetPlacement,
}

#[cfg(target_os = "windows")]
fn read_windows_usage_config(app: &AppHandle) -> WindowsUsageSurfaceConfig {
    load_store(app)
        .map(|store| WindowsUsageSurfaceConfig {
            mode: store.settings.tray_usage_display_mode,
            show_window_labels: store.settings.tray_usage_title_show_window_labels,
            tray_icon_style: store.settings.windows_tray_icon_style,
            widget_placement: store.settings.windows_taskbar_widget_placement,
        })
        .unwrap_or(WindowsUsageSurfaceConfig {
            mode: TrayUsageDisplayMode::default(),
            show_window_labels: false,
            tray_icon_style: WindowsTrayIconStyle::default(),
            widget_placement: WindowsTaskbarWidgetPlacement::default(),
        })
}

fn tray_icon_percent(accounts: &[AccountSummary], mode: TrayUsageDisplayMode) -> Option<f64> {
    if mode == TrayUsageDisplayMode::Hidden {
        return None;
    }
    let usage = accounts
        .iter()
        .find(|account| account.is_current)
        .and_then(|account| account.usage.as_ref())?;
    if only_show_single_window(mode) {
        return mode_percent(mode, single_window_for_mode(mode, Some(usage)));
    }

    let values = [usage.five_hour.as_ref(), usage.one_week.as_ref()]
        .into_iter()
        .filter_map(|window| mode_percent(mode, window));
    match mode {
        TrayUsageDisplayMode::Used => values.max_by(f64::total_cmp),
        TrayUsageDisplayMode::Remaining => values.min_by(f64::total_cmp),
        _ => None,
    }
}

#[cfg(target_os = "macos")]
fn macos_uses_native_tray_title(_style: WindowsTrayIconStyle) -> bool {
    false
}

#[cfg(target_os = "macos")]
fn macos_light_theme(app: &AppHandle) -> bool {
    app.get_webview_window("main")
        .and_then(|window| window.theme().ok())
        .map(|theme| theme != tauri::Theme::Dark)
        .unwrap_or(true)
}

#[cfg(target_os = "macos")]
fn render_macos_tray_icon(
    app: &AppHandle,
    style: WindowsTrayIconStyle,
    percent: Option<f64>,
) -> tauri::image::Image<'static> {
    const MACOS_SOURCE_SIZE: u32 = 64;
    let (width, height) =
        tray_visual_dimensions(style, TrayVisualPlatform::Macos, MACOS_SOURCE_SIZE);
    render_tray_visual(
        style,
        percent,
        TrayVisualStatus::Fresh,
        macos_light_theme(app),
        width,
        height,
    )
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

fn build_tray_usage_title(
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

#[cfg(target_os = "windows")]
fn cached_account_summaries(app: &AppHandle) -> Result<Vec<AccountSummary>, String> {
    let store = load_store(app)?;
    let current_account_key = crate::auth::current_auth_account_key();
    let current_variant_key = crate::auth::current_auth_variant_key();
    let mut summaries = store
        .accounts
        .iter()
        .map(|account| {
            account.to_summary(
                current_account_key.as_deref(),
                current_variant_key.as_deref(),
            )
        })
        .collect::<Vec<_>>();
    mark_current_account_summary(
        &mut summaries,
        current_account_key.as_deref(),
        store.settings.active_account_id.as_deref(),
    );
    Ok(summaries)
}

#[cfg(target_os = "windows")]
fn windows_widget_state_label(
    locale: crate::models::AppLocale,
    status: WindowsWidgetStatus,
) -> &'static str {
    use crate::models::AppLocale;
    match (locale, status) {
        (AppLocale::ZhCn, WindowsWidgetStatus::Fresh) => "额度数据已更新",
        (AppLocale::ZhCn, WindowsWidgetStatus::Stale) => "额度数据已过期",
        (AppLocale::ZhCn, WindowsWidgetStatus::Error) => "额度刷新失败",
        (AppLocale::ZhCn, WindowsWidgetStatus::Unavailable) => "额度数据不可用",
        (AppLocale::JaJp, WindowsWidgetStatus::Fresh) => "使用量データは最新です",
        (AppLocale::JaJp, WindowsWidgetStatus::Stale) => "使用量データが古くなっています",
        (AppLocale::JaJp, WindowsWidgetStatus::Error) => "使用量の更新に失敗しました",
        (AppLocale::JaJp, WindowsWidgetStatus::Unavailable) => "使用量データを利用できません",
        (AppLocale::KoKr, WindowsWidgetStatus::Fresh) => "사용량 데이터가 최신입니다",
        (AppLocale::KoKr, WindowsWidgetStatus::Stale) => "사용량 데이터가 오래되었습니다",
        (AppLocale::KoKr, WindowsWidgetStatus::Error) => "사용량 새로 고침 실패",
        (AppLocale::KoKr, WindowsWidgetStatus::Unavailable) => "사용량 데이터를 사용할 수 없음",
        (AppLocale::RuRu, WindowsWidgetStatus::Fresh) => "Данные квоты обновлены",
        (AppLocale::RuRu, WindowsWidgetStatus::Stale) => "Данные квоты устарели",
        (AppLocale::RuRu, WindowsWidgetStatus::Error) => "Не удалось обновить квоту",
        (AppLocale::RuRu, WindowsWidgetStatus::Unavailable) => "Данные квоты недоступны",
        (_, WindowsWidgetStatus::Fresh) => "Quota data is up to date",
        (_, WindowsWidgetStatus::Stale) => "Quota data is stale",
        (_, WindowsWidgetStatus::Error) => "Quota refresh failed",
        (_, WindowsWidgetStatus::Unavailable) => "Quota data is unavailable",
    }
}

#[cfg(target_os = "windows")]
fn build_windows_widget_snapshot(
    accounts: &[AccountSummary],
    mode: TrayUsageDisplayMode,
    show_window_labels: bool,
    placement: WindowsTaskbarWidgetPlacement,
    locale: crate::models::AppLocale,
    surface_error: Option<&str>,
) -> WindowsTaskbarWidgetSnapshot {
    let current = accounts.iter().find(|account| account.is_current);
    let title = build_tray_usage_title(accounts, mode, show_window_labels);
    let account_error = current.and_then(|account| {
        account
            .usage_error
            .as_deref()
            .or(account.auth_refresh_error.as_deref())
    });
    let error = surface_error.or(account_error);
    let fetched_at = current
        .and_then(|account| account.usage.as_ref())
        .map(|usage| usage.fetched_at);
    let stale = fetched_at.is_some_and(|timestamp| {
        timestamp <= 0
            || crate::utils::now_unix_seconds().saturating_sub(timestamp)
                > WINDOWS_WIDGET_STALE_AFTER_SECONDS
    });
    let status = if error.is_some() {
        WindowsWidgetStatus::Error
    } else if current.is_none() || current.and_then(|account| account.usage.as_ref()).is_none() {
        WindowsWidgetStatus::Unavailable
    } else if stale {
        WindowsWidgetStatus::Stale
    } else {
        WindowsWidgetStatus::Fresh
    };
    let text = match status {
        WindowsWidgetStatus::Stale => format!("~{title}"),
        _ => title.clone(),
    };
    let mut tooltip_lines = vec!["Codex Tools".to_string()];
    tooltip_lines.push(format!(
        "{}: {}",
        i18n::tray_usage_mode_label(locale, mode),
        title
    ));
    if let Some(account) = current {
        tooltip_lines.push(format!(
            "{}: {}",
            i18n::tray_current_label(locale),
            account.label
        ));
    } else {
        tooltip_lines.push(format!(
            "{}: {}",
            i18n::tray_current_label(locale),
            i18n::tray_no_current(locale)
        ));
    }
    tooltip_lines.push(windows_widget_state_label(locale, status).to_string());
    if let Some(error) = error {
        tooltip_lines.push(error.to_string());
    } else if let Some(timestamp) = fetched_at {
        tooltip_lines.push(format!("fetched_at: {timestamp}"));
    }

    WindowsTaskbarWidgetSnapshot {
        visible: should_show_usage_surface(mode)
            && placement != WindowsTaskbarWidgetPlacement::Hidden,
        placement,
        text,
        tooltip: tooltip_lines.join("\n"),
        status,
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
    let icon_style = read_macos_tray_icon_style(app);
    let locale = i18n::app_locale(app);
    let tray = app
        .tray_by_id(TRAY_ID)
        .ok_or_else(|| "状态栏尚未初始化".to_string())?;

    if !should_show_usage_surface(mode) {
        tray.set_visible(false)
            .map_err(|e| format!("隐藏状态栏失败: {e}"))?;
        return Ok(());
    }

    let menu = build_macos_tray_menu(app, accounts, mode)?;
    let title = build_tray_usage_title(accounts, mode, show_window_labels);
    #[cfg(debug_assertions)]
    log_macos_status_bar_render("update", accounts, &title);
    tray.set_menu(Some(menu))
        .map_err(|e| format!("更新状态栏菜单失败: {e}"))?;
    if macos_uses_native_tray_title(icon_style) {
        tray.set_icon(Some(STATUS_BAR_ICON))
            .map_err(|e| format!("更新状态栏图标失败: {e}"))?;
        tray.set_title(Some(title))
            .map_err(|e| format!("更新状态栏标题失败: {e}"))?;
    } else {
        let icon = render_macos_tray_icon(app, icon_style, tray_icon_percent(accounts, mode));
        tray.set_icon(Some(icon))
            .map_err(|e| format!("更新状态栏图标失败: {e}"))?;
        tray.set_title(None::<&str>)
            .map_err(|e| format!("清除状态栏原生标题失败: {e}"))?;
    }
    tray.set_icon_as_template(false)
        .map_err(|e| format!("设置状态栏彩色图标失败: {e}"))?;
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

#[cfg(target_os = "windows")]
fn update_windows_usage_snapshot(
    app: &AppHandle,
    accounts: &[AccountSummary],
    surface_error: Option<&str>,
) -> Result<(), String> {
    let config = read_windows_usage_config(app);
    let locale = i18n::app_locale(app);
    let snapshot = build_windows_widget_snapshot(
        accounts,
        config.mode,
        config.show_window_labels,
        config.widget_placement,
        locale,
        surface_error,
    );
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        tray.set_tooltip(Some(snapshot.tooltip.clone()))
            .map_err(|error| format!("更新 Windows 托盘提示失败: {error}"))?;
        let icon = if config.mode == TrayUsageDisplayMode::Hidden {
            static_codex_tools_icon()
        } else {
            render_windows_tray_icon(
                config.tray_icon_style,
                tray_icon_percent(accounts, config.mode),
                snapshot.status,
            )
        };
        tray.set_icon(Some(icon))
            .map_err(|error| format!("更新 Windows 托盘图标失败: {error}"))?;
    }
    crate::windows_taskbar_widget::update(snapshot)
}

#[cfg(target_os = "windows")]
fn refresh_windows_usage_snapshot(app: &AppHandle) -> Result<(), String> {
    let summaries = cached_account_summaries(app)?;
    update_windows_usage_snapshot(app, &summaries, None)
}

pub(crate) fn update_usage_surfaces_snapshot(
    app: &AppHandle,
    accounts: &[AccountSummary],
) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        return update_macos_tray_snapshot(app, accounts);
    }
    #[cfg(target_os = "windows")]
    {
        return update_windows_usage_snapshot(app, accounts, None);
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = (app, accounts);
        Ok(())
    }
}

pub(crate) fn refresh_usage_surfaces_snapshot(app: &AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        return refresh_macos_tray_snapshot(app);
    }
    #[cfg(target_os = "windows")]
    {
        return refresh_windows_usage_snapshot(app);
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = app;
        Ok(())
    }
}

pub(crate) fn update_usage_surfaces_error(app: &AppHandle, error: &str) {
    #[cfg(target_os = "windows")]
    match cached_account_summaries(app)
        .and_then(|summaries| update_windows_usage_snapshot(app, &summaries, Some(error)))
    {
        Ok(()) => {}
        Err(update_error) => {
            log::warn!("更新 Windows 额度组件错误状态失败: {update_error}");
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = (app, error);
    }
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
                if let Ok(summaries) =
                    refresh_all_usage_coordinated(&app, state.inner(), false, "macos-hidden").await
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
    let icon_style = read_macos_tray_icon_style(app);
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
    let title = build_tray_usage_title(&summaries, mode, show_window_labels);
    #[cfg(debug_assertions)]
    log_macos_status_bar_render("setup", &summaries, &title);

    let initial_icon = if macos_uses_native_tray_title(icon_style) {
        STATUS_BAR_ICON
    } else {
        render_macos_tray_icon(app, icon_style, tray_icon_percent(&summaries, mode))
    };
    let mut tray_builder = TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        .icon(initial_icon)
        .icon_as_template(false)
        .tooltip(build_macos_tray_tooltip(&summaries, mode, locale))
        .show_menu_on_left_click(true);
    if macos_uses_native_tray_title(icon_style) {
        tray_builder = tray_builder.title(title);
    }
    let tray = tray_builder
        .build(app)
        .map_err(|e| format!("创建 macOS 状态栏失败: {e}"))?;
    tray.set_visible(should_show_usage_surface(mode))
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
    let summaries = cached_account_summaries(app)?;
    let config = read_windows_usage_config(app);
    let initial_snapshot = build_windows_widget_snapshot(
        &summaries,
        config.mode,
        config.show_window_labels,
        config.widget_placement,
        i18n::app_locale(app),
        None,
    );
    let initial_icon = if config.mode == TrayUsageDisplayMode::Hidden {
        static_codex_tools_icon()
    } else {
        render_windows_tray_icon(
            config.tray_icon_style,
            tray_icon_percent(&summaries, config.mode),
            initial_snapshot.status,
        )
    };

    let tray = TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        .icon(initial_icon)
        .tooltip(initial_snapshot.tooltip.clone())
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

    if let Err(error) = crate::windows_taskbar_widget::setup(app, initial_snapshot) {
        log::warn!("Windows 任务栏额度组件启动失败，保留普通托盘入口: {error}");
        let _ = tray.set_tooltip(Some(format!(
            "Codex Tools\nWindows quota widget unavailable\n{error}"
        )));
    }

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
                refresh_all_usage_coordinated(&app_handle, state.inner(), true, "macos-tray-manual")
                    .await
            {
                let _ = update_macos_tray_snapshot(&app_handle, &summaries);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::build_tray_usage_title;
    #[cfg(target_os = "windows")]
    use super::build_windows_widget_snapshot;
    use super::should_show_usage_surface;
    #[cfg(target_os = "macos")]
    use super::tray_account_usage_line;
    #[cfg(target_os = "windows")]
    use super::tray_icon_percent;
    use crate::models::AccountSummary;
    use crate::models::AppLocale;
    use crate::models::TrayUsageDisplayMode;
    use crate::models::UsageSnapshot;
    use crate::models::UsageWindow;
    #[cfg(target_os = "windows")]
    use crate::models::WindowsTaskbarWidgetPlacement;

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
                fetched_at: crate::utils::now_unix_seconds(),
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
            build_tray_usage_title(
                std::slice::from_ref(&account),
                TrayUsageDisplayMode::OneWeekRemaining,
                false,
            ),
            "60%"
        );
        assert_eq!(
            build_tray_usage_title(
                std::slice::from_ref(&account),
                TrayUsageDisplayMode::OneWeekRemaining,
                true,
            ),
            "1w 60%"
        );

        #[cfg(target_os = "macos")]
        {
            let usage_line = tray_account_usage_line(
                &account,
                TrayUsageDisplayMode::OneWeekRemaining,
                AppLocale::EnUs,
            );
            assert!(usage_line.contains("60%"));
            assert!(!usage_line.contains("40%"));
            assert!(!usage_line.contains("5h"));
        }
    }

    #[test]
    fn one_week_remaining_mode_keeps_a_one_week_placeholder_without_current_account() {
        assert_eq!(
            build_tray_usage_title(&[], TrayUsageDisplayMode::OneWeekRemaining, false),
            "--"
        );
        assert_eq!(
            build_tray_usage_title(&[], TrayUsageDisplayMode::OneWeekRemaining, true),
            "1w --"
        );
    }

    #[test]
    fn window_labels_can_be_hidden_for_combined_usage_display() {
        let account = current_account_with_usage();

        assert_eq!(
            build_tray_usage_title(
                std::slice::from_ref(&account),
                TrayUsageDisplayMode::Remaining,
                false,
            ),
            "40% / 60%"
        );
        assert_eq!(
            build_tray_usage_title(&[account], TrayUsageDisplayMode::Remaining, true),
            "5h 40% / 1w 60%"
        );
    }

    #[test]
    fn hidden_mode_hides_the_usage_surface() {
        assert!(!should_show_usage_surface(TrayUsageDisplayMode::Hidden));
        assert!(should_show_usage_surface(
            TrayUsageDisplayMode::OneWeekRemaining
        ));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_widget_reuses_title_modes_and_exposes_health_states() {
        use crate::windows_taskbar_widget::WindowsWidgetStatus;

        let mut account = current_account_with_usage();
        let fresh = build_windows_widget_snapshot(
            std::slice::from_ref(&account),
            TrayUsageDisplayMode::OneWeekRemaining,
            false,
            WindowsTaskbarWidgetPlacement::Embedded,
            AppLocale::EnUs,
            None,
        );
        assert_eq!(fresh.text, "60%");
        assert_eq!(fresh.status, WindowsWidgetStatus::Fresh);
        assert!(fresh.visible);
        assert!(fresh.tooltip.contains("Quota data is up to date"));

        account.usage_error = Some("network unavailable".to_string());
        let error = build_windows_widget_snapshot(
            std::slice::from_ref(&account),
            TrayUsageDisplayMode::OneWeekRemaining,
            false,
            WindowsTaskbarWidgetPlacement::Embedded,
            AppLocale::EnUs,
            None,
        );
        assert_eq!(error.text, "60%");
        assert!(!error.text.contains('!'));
        assert_eq!(error.status, WindowsWidgetStatus::Error);
        assert!(error.tooltip.contains("network unavailable"));

        account.usage_error = None;
        account.usage.as_mut().expect("usage").fetched_at = 0;
        let stale = build_windows_widget_snapshot(
            std::slice::from_ref(&account),
            TrayUsageDisplayMode::OneWeekRemaining,
            false,
            WindowsTaskbarWidgetPlacement::Embedded,
            AppLocale::EnUs,
            None,
        );
        assert_eq!(stale.text, "~60%");
        assert_eq!(stale.status, WindowsWidgetStatus::Stale);
        assert!(stale.tooltip.contains("Quota data is stale"));

        let unavailable = build_windows_widget_snapshot(
            &[],
            TrayUsageDisplayMode::OneWeekRemaining,
            false,
            WindowsTaskbarWidgetPlacement::Embedded,
            AppLocale::EnUs,
            None,
        );
        assert_eq!(unavailable.text, "--");
        assert_eq!(unavailable.status, WindowsWidgetStatus::Unavailable);
        assert!(unavailable.tooltip.contains("Quota data is unavailable"));

        let hidden = build_windows_widget_snapshot(
            &[account],
            TrayUsageDisplayMode::Hidden,
            false,
            WindowsTaskbarWidgetPlacement::Embedded,
            AppLocale::EnUs,
            None,
        );
        assert!(!hidden.visible);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_tray_icon_uses_selected_or_most_constrained_window() {
        let account = current_account_with_usage();
        assert_eq!(
            tray_icon_percent(
                std::slice::from_ref(&account),
                TrayUsageDisplayMode::FiveHourRemaining,
            ),
            Some(40.0)
        );
        assert_eq!(
            tray_icon_percent(
                std::slice::from_ref(&account),
                TrayUsageDisplayMode::OneWeekRemaining,
            ),
            Some(60.0)
        );
        assert_eq!(
            tray_icon_percent(
                std::slice::from_ref(&account),
                TrayUsageDisplayMode::Remaining,
            ),
            Some(40.0)
        );
        assert_eq!(
            tray_icon_percent(&[account], TrayUsageDisplayMode::Used),
            Some(60.0)
        );
    }
}
