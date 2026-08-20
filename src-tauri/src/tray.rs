#[cfg(target_os = "macos")]
use std::cell::RefCell;
use tauri::AppHandle;
#[cfg(target_os = "macos")]
use tauri::Emitter;
#[cfg(any(target_os = "macos", target_os = "windows"))]
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
#[cfg(target_os = "macos")]
use crate::models::MacosTrayTextIconStyle;
use crate::models::TrayUsageDisplayMode;
use crate::models::UsageSnapshot;
use crate::models::UsageWindow;
#[cfg(target_os = "windows")]
use crate::models::WindowsTaskbarWidgetPlacement;
use crate::models::WindowsTrayIconStyle;
#[cfg(any(target_os = "macos", target_os = "windows"))]
use crate::state::AppState;
use crate::store::load_store;
#[cfg(target_os = "macos")]
use crate::tray_visual::{
    render_native_macos_tray_visual, tray_visual_dimensions, TrayVisualPlatform, TrayVisualStatus,
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
const REFRESH_INTERVAL_SECONDS: u64 = 60;
#[cfg(target_os = "macos")]
const MACOS_ONBOARDING_PREVIEW_PERCENT: f64 = 100.0;
#[cfg(target_os = "macos")]
const MACOS_STATUS_BAR_USAGE_REFRESHED_EVENT: &str = "macos-status-bar-usage-refreshed";
#[cfg(target_os = "macos")]
const MACOS_TEXT_STATUS_AUTOSAVE_NAME: &str = "com.carry.codex-tools.status-item.text";
#[cfg(target_os = "macos")]
const MACOS_QUOTA_STATUS_AUTOSAVE_NAME: &str = "com.carry.codex-tools.status-item.quota";
#[cfg(target_os = "windows")]
const WINDOWS_WIDGET_STALE_AFTER_SECONDS: i64 = 10 * 60;

const TRAY_MENU_OPEN_ID: &str = "tray_open_window";
const TRAY_MENU_QUIT_ID: &str = "tray_quit";

#[cfg(target_os = "macos")]
const TRAY_MENU_REFRESH_ID: &str = "tray_refresh_usage";
#[cfg(target_os = "macos")]
const MACOS_LEGACY_STATUS_ICON: tauri::image::Image<'_> = tauri::include_image!("./icons/icon.png");
#[cfg(target_os = "windows")]
const TRAY_ID: &str = "codex_tools_tray";

#[cfg(target_os = "macos")]
thread_local! {
    static MACOS_QUOTA_TRAY: RefCell<Option<tray_icon::TrayIcon>> = const { RefCell::new(None) };
    static MACOS_TEXT_CODEX_TRAY: RefCell<Option<tray_icon::TrayIcon>> = const { RefCell::new(None) };
    static MACOS_TEXT_PROGRESS_TRAY: RefCell<Option<tray_icon::TrayIcon>> = const { RefCell::new(None) };
}
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

#[cfg(target_os = "macos")]
fn macos_text_tray_visibility(
    mode: TrayUsageDisplayMode,
    style: MacosTrayTextIconStyle,
) -> (bool, bool) {
    if !should_show_usage_surface(mode) {
        return (false, false);
    }

    match style {
        MacosTrayTextIconStyle::CodexTools => (true, false),
        MacosTrayTextIconStyle::ProgressRing => (false, true),
    }
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

#[cfg(target_os = "macos")]
fn read_macos_tray_text_icon_style(app: &AppHandle) -> MacosTrayTextIconStyle {
    load_store(app)
        .map(|store| store.settings.macos_tray_text_icon_style)
        .unwrap_or_default()
}

#[cfg(target_os = "macos")]
fn read_tray_quota_icon_visible(app: &AppHandle) -> bool {
    load_store(app)
        .map(|store| store.settings.tray_quota_icon_visible)
        .unwrap_or(true)
}

#[cfg(target_os = "macos")]
fn read_macos_tray_logo_ring_show_percentage(app: &AppHandle) -> bool {
    load_store(app)
        .map(|store| store.settings.macos_tray_logo_ring_show_percentage)
        .unwrap_or(true)
}

#[cfg(target_os = "windows")]
#[derive(Debug, Clone, Copy)]
struct WindowsUsageSurfaceConfig {
    mode: TrayUsageDisplayMode,
    show_window_labels: bool,
    tray_icon_style: WindowsTrayIconStyle,
    tray_quota_icon_visible: bool,
    widget_placement: WindowsTaskbarWidgetPlacement,
}

#[cfg(target_os = "windows")]
fn read_windows_usage_config(app: &AppHandle) -> WindowsUsageSurfaceConfig {
    load_store(app)
        .map(|store| WindowsUsageSurfaceConfig {
            mode: effective_windows_usage_display_mode(store.settings.tray_usage_display_mode),
            show_window_labels: store.settings.tray_usage_title_show_window_labels,
            tray_icon_style: store.settings.windows_tray_icon_style,
            tray_quota_icon_visible: store.settings.tray_quota_icon_visible,
            widget_placement: store.settings.windows_taskbar_widget_placement,
        })
        .unwrap_or(WindowsUsageSurfaceConfig {
            mode: TrayUsageDisplayMode::default(),
            show_window_labels: false,
            tray_icon_style: WindowsTrayIconStyle::default(),
            tray_quota_icon_visible: true,
            widget_placement: WindowsTaskbarWidgetPlacement::default(),
        })
}

#[cfg(any(target_os = "windows", test))]
fn effective_windows_usage_display_mode(mode: TrayUsageDisplayMode) -> TrayUsageDisplayMode {
    if mode == TrayUsageDisplayMode::Hidden {
        // Windows has a dedicated taskbar placement switch. Older or imported
        // settings may still contain the macOS-only hidden text mode; treating
        // that value as the Windows default prevents a selected taskbar
        // placement from remaining invisibly disabled.
        TrayUsageDisplayMode::OneWeekRemaining
    } else {
        mode
    }
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

fn quota_icon_percent(accounts: &[AccountSummary]) -> Option<f64> {
    tray_icon_percent(accounts, TrayUsageDisplayMode::Remaining)
}

#[cfg(target_os = "macos")]
fn macos_onboarding_preview_percent(
    onboarding_completed: bool,
    accounts: &[AccountSummary],
) -> Option<f64> {
    (!onboarding_completed && !accounts.iter().any(|account| account.is_current))
        .then_some(MACOS_ONBOARDING_PREVIEW_PERCENT)
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
    render_native_macos_tray_visual(
        style,
        percent,
        TrayVisualStatus::Fresh,
        macos_light_theme(app),
        width,
        height,
    )
}

#[cfg(target_os = "macos")]
fn native_macos_tray_icon(
    app: &AppHandle,
    style: WindowsTrayIconStyle,
    percent: Option<f64>,
) -> Result<tray_icon::Icon, String> {
    let image = render_macos_tray_icon(app, style, percent);
    tray_icon::Icon::from_rgba(image.rgba().to_vec(), image.width(), image.height())
        .map_err(|error| format!("创建原生状态栏图标失败: {error}"))
}

#[cfg(target_os = "macos")]
fn native_macos_legacy_status_icon() -> Result<tray_icon::Icon, String> {
    tray_icon::Icon::from_rgba(
        MACOS_LEGACY_STATUS_ICON.rgba().to_vec(),
        MACOS_LEGACY_STATUS_ICON.width(),
        MACOS_LEGACY_STATUS_ICON.height(),
    )
    .map_err(|error| format!("创建经典状态栏图标失败: {error}"))
}

#[cfg(target_os = "macos")]
fn native_macos_text_status_icon(
    app: &AppHandle,
    style: MacosTrayTextIconStyle,
    percent: Option<f64>,
) -> Result<tray_icon::Icon, String> {
    match style {
        MacosTrayTextIconStyle::CodexTools => native_macos_legacy_status_icon(),
        MacosTrayTextIconStyle::ProgressRing => {
            native_macos_tray_icon(app, WindowsTrayIconStyle::LogoProgressRing, percent)
        }
    }
}

#[cfg(target_os = "macos")]
fn macos_quota_icon_title(
    style: WindowsTrayIconStyle,
    percent: Option<f64>,
    show_percentage: bool,
) -> Option<String> {
    (style == WindowsTrayIconStyle::LogoProgressRing && show_percentage)
        .then(|| format_percent(percent))
}

#[cfg(target_os = "macos")]
fn current_macos_text_tray(style: MacosTrayTextIconStyle) -> Option<tray_icon::TrayIcon> {
    match style {
        MacosTrayTextIconStyle::CodexTools => {
            MACOS_TEXT_CODEX_TRAY.with(|slot| slot.borrow().clone())
        }
        MacosTrayTextIconStyle::ProgressRing => {
            MACOS_TEXT_PROGRESS_TRAY.with(|slot| slot.borrow().clone())
        }
    }
}

#[cfg(target_os = "macos")]
fn store_macos_text_tray(style: MacosTrayTextIconStyle, tray: tray_icon::TrayIcon) {
    match style {
        MacosTrayTextIconStyle::CodexTools => {
            MACOS_TEXT_CODEX_TRAY.with(|slot| {
                let previous = slot.replace(Some(tray));
                drop(previous);
            });
        }
        MacosTrayTextIconStyle::ProgressRing => {
            MACOS_TEXT_PROGRESS_TRAY.with(|slot| {
                let previous = slot.replace(Some(tray));
                drop(previous);
            });
        }
    }
}

#[cfg(target_os = "macos")]
fn remove_macos_text_tray(style: MacosTrayTextIconStyle) {
    match style {
        MacosTrayTextIconStyle::CodexTools => {
            MACOS_TEXT_CODEX_TRAY.with(|slot| drop(slot.borrow_mut().take()));
        }
        MacosTrayTextIconStyle::ProgressRing => {
            MACOS_TEXT_PROGRESS_TRAY.with(|slot| drop(slot.borrow_mut().take()));
        }
    }
}

#[cfg(target_os = "macos")]
fn remove_all_macos_status_bar_trays() {
    MACOS_QUOTA_TRAY.with(|slot| drop(slot.borrow_mut().take()));
    remove_macos_text_tray(MacosTrayTextIconStyle::CodexTools);
    remove_macos_text_tray(MacosTrayTextIconStyle::ProgressRing);
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

#[cfg(target_os = "macos")]
fn build_macos_onboarding_preview_title(
    mode: TrayUsageDisplayMode,
    show_window_labels: bool,
    percent: f64,
) -> String {
    if mode == TrayUsageDisplayMode::Hidden {
        return String::new();
    }

    let percent = format_percent(Some(percent));
    if only_show_single_window(mode) {
        if !show_window_labels {
            return percent;
        }
        return match mode {
            TrayUsageDisplayMode::FiveHourRemaining => format!("5h {percent}"),
            TrayUsageDisplayMode::OneWeekRemaining => format!("1w {percent}"),
            _ => unreachable!("single-window modes are handled above"),
        };
    }

    if show_window_labels {
        format!("5h {percent} / 1w {percent}")
    } else {
        format!("{percent} / {percent}")
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
) -> Result<tray_icon::menu::Menu, String> {
    use tray_icon::menu::{Menu, MenuItem, PredefinedMenuItem};

    let locale = i18n::app_locale(app);
    let menu = Menu::new();

    let header_text = format!(
        "{} ({})",
        i18n::tray_usage_heading(locale),
        i18n::tray_usage_mode_label(locale, mode)
    );
    let header = MenuItem::with_id("tray_header", header_text, false, None);
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
    let current_item = MenuItem::with_id("tray_current_summary", current_line, false, None);
    menu.append(&current_item)
        .map_err(|e| format!("写入状态栏菜单失败: {e}"))?;

    let separator = PredefinedMenuItem::separator();
    menu.append(&separator)
        .map_err(|e| format!("写入状态栏菜单失败: {e}"))?;

    if accounts.is_empty() {
        let empty = MenuItem::with_id(
            "tray_accounts_empty",
            i18n::tray_empty_accounts(locale),
            false,
            None,
        );
        menu.append(&empty)
            .map_err(|e| format!("写入状态栏菜单失败: {e}"))?;
    } else {
        for (index, account) in accounts.iter().enumerate() {
            let id = format!("tray_account_{index}");
            let line_item = MenuItem::with_id(
                id,
                tray_account_usage_line(account, mode, locale),
                false,
                None,
            );
            menu.append(&line_item)
                .map_err(|e| format!("写入状态栏菜单失败: {e}"))?;
        }
    }

    let separator = PredefinedMenuItem::separator();
    menu.append(&separator)
        .map_err(|e| format!("写入状态栏菜单失败: {e}"))?;

    let refresh = MenuItem::with_id(
        TRAY_MENU_REFRESH_ID,
        i18n::tray_refresh_now(locale),
        true,
        None,
    );
    let open = MenuItem::with_id(TRAY_MENU_OPEN_ID, i18n::tray_open_app(locale), true, None);
    let quit = MenuItem::with_id(TRAY_MENU_QUIT_ID, i18n::tray_quit(locale), true, None);

    menu.append(&refresh)
        .map_err(|e| format!("写入状态栏菜单失败: {e}"))?;
    menu.append(&open)
        .map_err(|e| format!("写入状态栏菜单失败: {e}"))?;
    menu.append(&quit)
        .map_err(|e| format!("写入状态栏菜单失败: {e}"))?;

    Ok(menu)
}

#[cfg(target_os = "macos")]
fn build_native_macos_status_bar_tray(
    app: &AppHandle,
    id: &str,
    autosave_name: &str,
    accounts: &[AccountSummary],
    mode: TrayUsageDisplayMode,
    icon: tray_icon::Icon,
    title: &str,
    tooltip: &str,
    description: &str,
) -> Result<tray_icon::TrayIcon, String> {
    use tray_icon::TrayIconBuilder;

    let tray = TrayIconBuilder::new()
        .with_id(id)
        .with_menu(Box::new(build_macos_tray_menu(app, accounts, mode)?))
        .with_icon(icon)
        .with_icon_as_template(false)
        .with_title(title)
        .with_tooltip(tooltip)
        .with_menu_on_left_click(true)
        .build()
        .map_err(|error| format!("创建 {description} macOS 状态栏失败: {error}"))?;
    let status_item = tray
        .ns_status_item()
        .ok_or_else(|| format!("读取 {description} macOS 状态项失败"))?;
    let autosave_name = objc2_foundation::NSString::from_str(autosave_name);
    status_item.setAutosaveName(Some(&autosave_name));
    Ok(tray)
}

#[cfg(target_os = "macos")]
fn text_tray_id(style: MacosTrayTextIconStyle) -> &'static str {
    match style {
        MacosTrayTextIconStyle::CodexTools => "codex_tools_legacy_status_bar",
        MacosTrayTextIconStyle::ProgressRing => "codex_tools_progress_text_status_bar",
    }
}

#[cfg(target_os = "macos")]
fn text_tray_description(style: MacosTrayTextIconStyle) -> &'static str {
    match style {
        MacosTrayTextIconStyle::CodexTools => "Codex Tools 文字",
        MacosTrayTextIconStyle::ProgressRing => "进度环文字",
    }
}

#[cfg(target_os = "macos")]
fn update_macos_tray_snapshot_on_main_thread(
    app: &AppHandle,
    accounts: &[AccountSummary],
) -> Result<(), String> {
    let (mode, show_window_labels) = read_tray_title_config(app);
    let text_icon_style = read_macos_tray_text_icon_style(app);
    let icon_style = read_macos_tray_icon_style(app);
    let quota_icon_visible = read_tray_quota_icon_visible(app);
    let logo_ring_show_percentage = read_macos_tray_logo_ring_show_percentage(app);
    let locale = i18n::app_locale(app);
    let onboarding_preview_percent = load_store(app).ok().and_then(|store| {
        macos_onboarding_preview_percent(store.settings.macos_quota_onboarding_completed, accounts)
    });
    let percent = quota_icon_percent(accounts).or(onboarding_preview_percent);

    if should_show_usage_surface(mode) {
        let title = onboarding_preview_percent
            .map(|percent| build_macos_onboarding_preview_title(mode, show_window_labels, percent))
            .unwrap_or_else(|| build_tray_usage_title(accounts, mode, show_window_labels));
        let tooltip = build_macos_tray_tooltip(accounts, mode, locale);
        #[cfg(debug_assertions)]
        log_macos_status_bar_render("update", accounts, &title);

        let inactive_text_style = match text_icon_style {
            MacosTrayTextIconStyle::CodexTools => MacosTrayTextIconStyle::ProgressRing,
            MacosTrayTextIconStyle::ProgressRing => MacosTrayTextIconStyle::CodexTools,
        };
        // tray-icon 0.21 在 macOS 上将隐藏项重新设为可见时，偶尔只会得到
        // 位于屏幕底边的离屏窗口。切换样式时先销毁旧项，再创建新项，既保留
        // 原生弹入动画，也保证文字栏与额度图标能稳定地同时留在菜单栏。
        remove_macos_text_tray(inactive_text_style);
        let text_tray = if let Some(tray) = current_macos_text_tray(text_icon_style) {
            tray
        } else {
            let tray = build_native_macos_status_bar_tray(
                app,
                text_tray_id(text_icon_style),
                MACOS_TEXT_STATUS_AUTOSAVE_NAME,
                accounts,
                mode,
                native_macos_text_status_icon(app, text_icon_style, percent)?,
                &title,
                &tooltip,
                text_tray_description(text_icon_style),
            )?;
            store_macos_text_tray(text_icon_style, tray.clone());
            tray
        };
        text_tray.set_menu(Some(Box::new(build_macos_tray_menu(app, accounts, mode)?)));
        text_tray
            .set_icon_with_as_template(
                Some(native_macos_text_status_icon(
                    app,
                    text_icon_style,
                    percent,
                )?),
                false,
            )
            .map_err(|error| format!("更新文字状态栏图标失败: {error}"))?;
        text_tray.set_title(Some(title.as_str()));
        text_tray
            .set_tooltip(Some(tooltip.as_str()))
            .map_err(|error| format!("更新文字状态栏提示失败: {error}"))?;
    } else {
        remove_macos_text_tray(MacosTrayTextIconStyle::CodexTools);
        remove_macos_text_tray(MacosTrayTextIconStyle::ProgressRing);
    }

    if !quota_icon_visible {
        MACOS_QUOTA_TRAY.with(|slot| drop(slot.borrow_mut().take()));
        #[cfg(debug_assertions)]
        log_current_macos_status_bar_rects("update");
        return Ok(());
    }

    let quota_mode = TrayUsageDisplayMode::Remaining;
    let quota_title = macos_quota_icon_title(icon_style, percent, logo_ring_show_percentage);
    let quota_tooltip = build_macos_tray_tooltip(accounts, quota_mode, locale);
    let quota_tray = if let Some(tray) = MACOS_QUOTA_TRAY.with(|slot| slot.borrow().clone()) {
        tray
    } else {
        let tray = build_native_macos_status_bar_tray(
            app,
            "codex_tools_native_status_bar",
            MACOS_QUOTA_STATUS_AUTOSAVE_NAME,
            accounts,
            quota_mode,
            native_macos_tray_icon(app, icon_style, percent)?,
            quota_title.as_deref().unwrap_or(""),
            &quota_tooltip,
            "额度",
        )?;
        MACOS_QUOTA_TRAY.with(|slot| {
            let previous = slot.replace(Some(tray.clone()));
            drop(previous);
        });
        tray
    };
    quota_tray.set_menu(Some(Box::new(build_macos_tray_menu(
        app, accounts, quota_mode,
    )?)));
    let icon = native_macos_tray_icon(app, icon_style, percent)?;
    quota_tray
        .set_icon_with_as_template(Some(icon), false)
        .map_err(|error| format!("更新额度状态栏图标失败: {error}"))?;
    quota_tray.set_title(Some(quota_title.as_deref().unwrap_or("")));
    quota_tray
        .set_tooltip(Some(quota_tooltip.as_str()))
        .map_err(|error| format!("更新额度状态栏提示失败: {error}"))?;
    #[cfg(debug_assertions)]
    log_current_macos_status_bar_rects("update");
    Ok(())
}

#[cfg(target_os = "macos")]
pub(crate) fn update_macos_tray_snapshot(
    app: &AppHandle,
    accounts: &[AccountSummary],
) -> Result<(), String> {
    let app = app.clone();
    let update_app = app.clone();
    let accounts = accounts.to_vec();
    let (sender, receiver) = std::sync::mpsc::channel();
    app.run_on_main_thread(move || {
        let result = update_macos_tray_snapshot_on_main_thread(&update_app, &accounts);
        let _ = sender.send(result);
    })
    .map_err(|error| format!("调度状态栏更新失败: {error}"))?;
    receiver
        .recv()
        .map_err(|error| format!("接收状态栏更新结果失败: {error}"))?
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
        let icon = if !config.tray_quota_icon_visible {
            static_codex_tools_icon()
        } else {
            render_windows_tray_icon(
                config.tray_icon_style,
                quota_icon_percent(accounts),
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
    let state = app.state::<AppState>();
    let surface_error = state
        .usage_surface_error
        .lock()
        .map_err(|_| "Windows quota widget error state lock is poisoned".to_string())?
        .clone();
    update_windows_usage_snapshot(app, &summaries, surface_error.as_deref())
}

pub(crate) fn update_usage_surfaces_snapshot(
    app: &AppHandle,
    accounts: &[AccountSummary],
) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        update_macos_tray_snapshot(app, accounts)
    }
    #[cfg(target_os = "windows")]
    {
        update_windows_usage_snapshot(app, accounts, None)?;
        let state = app.state::<AppState>();
        *state
            .usage_surface_error
            .lock()
            .map_err(|_| "Windows quota widget error state lock is poisoned".to_string())? = None;
        Ok(())
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
        refresh_macos_tray_snapshot(app)
    }
    #[cfg(target_os = "windows")]
    {
        refresh_windows_usage_snapshot(app)
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = app;
        Ok(())
    }
}

pub(crate) fn rebuild_usage_surfaces_snapshot(app: &AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let app = app.clone();
        let rebuild_app = app.clone();
        let (sender, receiver) = std::sync::mpsc::channel();
        app.run_on_main_thread(move || {
            let result = replace_macos_status_bar_trays(&rebuild_app, "settings-rebuild");
            let _ = sender.send(result);
        })
        .map_err(|error| format!("调度 macOS 状态栏重建失败: {error}"))?;
        receiver
            .recv()
            .map_err(|error| format!("接收 macOS 状态栏重建结果失败: {error}"))?
    }
    #[cfg(not(target_os = "macos"))]
    {
        refresh_usage_surfaces_snapshot(app)
    }
}

pub(crate) fn update_usage_surfaces_error(app: &AppHandle, error: &str) {
    #[cfg(target_os = "windows")]
    let result = {
        let state = app.state::<AppState>();
        match state.usage_surface_error.lock() {
            Ok(mut stored_error) => {
                *stored_error = Some(error.to_string());
            }
            Err(_) => {
                log::warn!("Windows quota widget error state lock is poisoned");
            }
        }
        cached_account_summaries(app)
            .and_then(|summaries| update_windows_usage_snapshot(app, &summaries, Some(error)))
    };

    #[cfg(target_os = "windows")]
    match result {
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

#[cfg(all(target_os = "macos", debug_assertions))]
fn log_macos_status_bar_rects(
    context: &str,
    quota_tray: Option<&tray_icon::TrayIcon>,
    text_codex_tray: Option<&tray_icon::TrayIcon>,
    text_progress_tray: Option<&tray_icon::TrayIcon>,
) {
    log::info!(
        "MACOS_TRAY_RECTS context={context} quota={:?} text_codex={:?} text_progress={:?}",
        quota_tray.and_then(tray_icon::TrayIcon::rect),
        text_codex_tray.and_then(tray_icon::TrayIcon::rect),
        text_progress_tray.and_then(tray_icon::TrayIcon::rect),
    );
}

#[cfg(all(target_os = "macos", debug_assertions))]
fn log_current_macos_status_bar_rects(context: &str) {
    let quota_tray = MACOS_QUOTA_TRAY.with(|slot| slot.borrow().clone());
    let text_codex_tray = MACOS_TEXT_CODEX_TRAY.with(|slot| slot.borrow().clone());
    let text_progress_tray = MACOS_TEXT_PROGRESS_TRAY.with(|slot| slot.borrow().clone());
    log_macos_status_bar_rects(
        context,
        quota_tray.as_ref(),
        text_codex_tray.as_ref(),
        text_progress_tray.as_ref(),
    );
}

#[cfg(target_os = "macos")]
fn start_macos_tray_refresh_loop(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            // 状态栏刷新不依赖窗口可见性：最小化、被遮挡或留在其他空间的
            // 主窗口仍可能被系统报告为可见。协调器会与前端和手动刷新共享
            // in-flight 请求，并短时复用刚完成的结果，避免重复网络访问。
            tokio::time::sleep(Duration::from_secs(REFRESH_INTERVAL_SECONDS)).await;
            let state = app.state::<AppState>();
            if let Ok(summaries) =
                refresh_all_usage_coordinated(&app, state.inner(), false, "macos-status-bar").await
            {
                let _ = update_macos_tray_snapshot(&app, &summaries);
                let _ = app.emit(MACOS_STATUS_BAR_USAGE_REFRESHED_EVENT, &summaries);
            }
        }
    });
}

#[cfg(target_os = "macos")]
fn create_macos_status_bar_trays(
    app: &AppHandle,
    _log_context: &str,
) -> Result<
    (
        Option<tray_icon::TrayIcon>,
        Option<tray_icon::TrayIcon>,
        Option<tray_icon::TrayIcon>,
    ),
    String,
> {
    let (mode, show_window_labels) = read_tray_title_config(app);
    let text_icon_style = read_macos_tray_text_icon_style(app);
    let icon_style = read_macos_tray_icon_style(app);
    let quota_icon_visible = read_tray_quota_icon_visible(app);
    let logo_ring_show_percentage = read_macos_tray_logo_ring_show_percentage(app);
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
        _log_context,
        &store,
        &summaries,
        current_account_key.as_deref(),
        current_variant_key.as_deref(),
    );
    let onboarding_preview_percent = macos_onboarding_preview_percent(
        store.settings.macos_quota_onboarding_completed,
        &summaries,
    );
    let title = onboarding_preview_percent
        .map(|percent| build_macos_onboarding_preview_title(mode, show_window_labels, percent))
        .unwrap_or_else(|| build_tray_usage_title(&summaries, mode, show_window_labels));
    let tooltip = build_macos_tray_tooltip(&summaries, mode, locale);
    #[cfg(debug_assertions)]
    log_macos_status_bar_render(_log_context, &summaries, &title);

    let quota_mode = TrayUsageDisplayMode::Remaining;
    let percent = quota_icon_percent(&summaries).or(onboarding_preview_percent);
    let quota_title = macos_quota_icon_title(icon_style, percent, logo_ring_show_percentage);
    let quota_tooltip = build_macos_tray_tooltip(&summaries, quota_mode, locale);
    let quota_tray = if quota_icon_visible {
        Some(build_native_macos_status_bar_tray(
            app,
            "codex_tools_native_status_bar",
            MACOS_QUOTA_STATUS_AUTOSAVE_NAME,
            &summaries,
            quota_mode,
            native_macos_tray_icon(app, icon_style, percent)?,
            quota_title.as_deref().unwrap_or(""),
            &quota_tooltip,
            "额度",
        )?)
    } else {
        None
    };

    let (show_text_codex, show_text_progress) = macos_text_tray_visibility(mode, text_icon_style);
    let text_codex_tray = show_text_codex
        .then(|| {
            build_native_macos_status_bar_tray(
                app,
                text_tray_id(MacosTrayTextIconStyle::CodexTools),
                MACOS_TEXT_STATUS_AUTOSAVE_NAME,
                &summaries,
                mode,
                native_macos_text_status_icon(app, MacosTrayTextIconStyle::CodexTools, percent)?,
                &title,
                &tooltip,
                text_tray_description(MacosTrayTextIconStyle::CodexTools),
            )
        })
        .transpose()?;
    let text_progress_tray = show_text_progress
        .then(|| {
            build_native_macos_status_bar_tray(
                app,
                text_tray_id(MacosTrayTextIconStyle::ProgressRing),
                MACOS_TEXT_STATUS_AUTOSAVE_NAME,
                &summaries,
                mode,
                native_macos_text_status_icon(app, MacosTrayTextIconStyle::ProgressRing, percent)?,
                &title,
                &tooltip,
                text_tray_description(MacosTrayTextIconStyle::ProgressRing),
            )
        })
        .transpose()?;

    #[cfg(debug_assertions)]
    log_macos_status_bar_rects(
        _log_context,
        quota_tray.as_ref(),
        text_codex_tray.as_ref(),
        text_progress_tray.as_ref(),
    );

    Ok((quota_tray, text_codex_tray, text_progress_tray))
}

#[cfg(target_os = "macos")]
fn replace_macos_status_bar_trays(app: &AppHandle, log_context: &str) -> Result<(), String> {
    // Drop every old NSStatusItem before creating replacements. Creating a new item
    // with the same ID before the old wrapper is dropped lets the old Drop remove
    // the new registration and was the source of the earlier disappearing icons.
    remove_all_macos_status_bar_trays();
    let (quota_tray, text_codex_tray, text_progress_tray) =
        create_macos_status_bar_trays(app, log_context)?;
    MACOS_QUOTA_TRAY.with(|slot| {
        let previous = slot.replace(quota_tray);
        drop(previous);
    });
    MACOS_TEXT_CODEX_TRAY.with(|slot| {
        let previous = slot.replace(text_codex_tray);
        drop(previous);
    });
    MACOS_TEXT_PROGRESS_TRAY.with(|slot| {
        let previous = slot.replace(text_progress_tray);
        drop(previous);
    });
    Ok(())
}

#[cfg(target_os = "macos")]
fn setup_macos_status_bar(app: &AppHandle) -> Result<(), String> {
    replace_macos_status_bar_trays(app, "setup")?;

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
    let initial_icon = if !config.tray_quota_icon_visible {
        static_codex_tools_icon()
    } else {
        render_windows_tray_icon(
            config.tray_icon_style,
            quota_icon_percent(&summaries),
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
        setup_windows_tray(app)
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
    #[cfg(target_os = "macos")]
    use super::build_macos_onboarding_preview_title;
    use super::build_tray_usage_title;
    #[cfg(target_os = "windows")]
    use super::build_windows_widget_snapshot;
    use super::effective_windows_usage_display_mode;
    #[cfg(target_os = "macos")]
    use super::macos_onboarding_preview_percent;
    #[cfg(target_os = "macos")]
    use super::macos_quota_icon_title;
    #[cfg(target_os = "macos")]
    use super::macos_text_tray_visibility;
    use super::quota_icon_percent;
    use super::should_show_usage_surface;
    #[cfg(target_os = "macos")]
    use super::tray_account_usage_line;
    #[cfg(target_os = "windows")]
    use super::tray_icon_percent;
    #[cfg(target_os = "macos")]
    use super::MACOS_QUOTA_STATUS_AUTOSAVE_NAME;
    #[cfg(target_os = "macos")]
    use super::MACOS_TEXT_STATUS_AUTOSAVE_NAME;
    #[cfg(target_os = "macos")]
    use super::REFRESH_INTERVAL_SECONDS;
    use crate::models::AccountSummary;
    use crate::models::AppLocale;
    #[cfg(target_os = "macos")]
    use crate::models::MacosTrayTextIconStyle;
    use crate::models::TrayUsageDisplayMode;
    use crate::models::UsageSnapshot;
    use crate::models::UsageWindow;
    #[cfg(target_os = "windows")]
    use crate::models::WindowsTaskbarWidgetPlacement;
    #[cfg(target_os = "macos")]
    use crate::models::WindowsTrayIconStyle;

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_status_bar_refreshes_once_per_minute() {
        assert_eq!(REFRESH_INTERVAL_SECONDS, 60);
    }

    #[test]
    fn windows_legacy_hidden_usage_mode_falls_back_to_one_week_remaining() {
        assert_eq!(
            effective_windows_usage_display_mode(TrayUsageDisplayMode::Hidden),
            TrayUsageDisplayMode::OneWeekRemaining
        );
        assert_eq!(
            effective_windows_usage_display_mode(TrayUsageDisplayMode::FiveHourRemaining),
            TrayUsageDisplayMode::FiveHourRemaining
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_text_and_quota_status_items_have_distinct_persistent_names() {
        assert!(!MACOS_TEXT_STATUS_AUTOSAVE_NAME.is_empty());
        assert!(!MACOS_QUOTA_STATUS_AUTOSAVE_NAME.is_empty());
        assert_ne!(
            MACOS_TEXT_STATUS_AUTOSAVE_NAME,
            MACOS_QUOTA_STATUS_AUTOSAVE_NAME
        );
    }

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

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_new_user_without_an_account_gets_a_one_hundred_percent_preview() {
        assert_eq!(macos_onboarding_preview_percent(false, &[]), Some(100.0));
        assert_eq!(
            build_macos_onboarding_preview_title(
                TrayUsageDisplayMode::OneWeekRemaining,
                false,
                100.0,
            ),
            "100%"
        );
        assert_eq!(
            macos_onboarding_preview_percent(false, &[current_account_with_usage()]),
            None
        );
        assert_eq!(macos_onboarding_preview_percent(true, &[]), None);
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

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_text_icon_choices_use_separate_mutually_exclusive_status_items() {
        assert_eq!(
            macos_text_tray_visibility(
                TrayUsageDisplayMode::OneWeekRemaining,
                MacosTrayTextIconStyle::CodexTools,
            ),
            (true, false)
        );
        assert_eq!(
            macos_text_tray_visibility(
                TrayUsageDisplayMode::OneWeekRemaining,
                MacosTrayTextIconStyle::ProgressRing,
            ),
            (false, true)
        );
        assert_eq!(
            macos_text_tray_visibility(
                TrayUsageDisplayMode::Hidden,
                MacosTrayTextIconStyle::CodexTools,
            ),
            (false, false)
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn only_logo_progress_ring_adds_a_compact_percentage_title() {
        assert_eq!(
            macos_quota_icon_title(WindowsTrayIconStyle::LogoProgressRing, Some(88.4), true),
            Some("88%".to_string())
        );
        assert_eq!(
            macos_quota_icon_title(WindowsTrayIconStyle::GradientNumberPlate, Some(88.4), true,),
            None
        );
        assert_eq!(
            macos_quota_icon_title(WindowsTrayIconStyle::LogoProgressRing, None, true),
            Some("--".to_string())
        );
        assert_eq!(
            macos_quota_icon_title(WindowsTrayIconStyle::LogoProgressRing, Some(88.4), false),
            None
        );
    }

    #[test]
    fn quota_icon_always_uses_the_most_constrained_remaining_window() {
        assert_eq!(
            quota_icon_percent(&[current_account_with_usage()]),
            Some(40.0)
        );
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
    fn status_bar_modes_do_not_change_the_quota_icon_percentage() {
        let account = current_account_with_usage();
        assert_eq!(
            tray_icon_percent(std::slice::from_ref(&account), TrayUsageDisplayMode::Used,),
            Some(60.0)
        );
        assert_eq!(
            tray_icon_percent(
                std::slice::from_ref(&account),
                TrayUsageDisplayMode::OneWeekRemaining,
            ),
            Some(60.0)
        );
        assert_eq!(
            tray_icon_percent(std::slice::from_ref(&account), TrayUsageDisplayMode::Hidden,),
            None
        );
        assert_eq!(quota_icon_percent(&[account]), Some(40.0));
    }
}
