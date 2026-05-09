use std::env;
use std::sync::OnceLock;

use serde_json::Value;
use tauri::AppHandle;

use crate::models::AppLocale;
use crate::models::TrayUsageDisplayMode;
use crate::store::load_store;

static ZH_CN_MESSAGES: OnceLock<Value> = OnceLock::new();
static EN_US_MESSAGES: OnceLock<Value> = OnceLock::new();
static JA_JP_MESSAGES: OnceLock<Value> = OnceLock::new();
static KO_KR_MESSAGES: OnceLock<Value> = OnceLock::new();
static RU_RU_MESSAGES: OnceLock<Value> = OnceLock::new();

fn zh_cn_messages() -> &'static Value {
    ZH_CN_MESSAGES.get_or_init(|| {
        parse_locale(
            "zh-CN",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../src/i18n/locales/zh-CN.json"
            )),
        )
    })
}

fn en_us_messages() -> &'static Value {
    EN_US_MESSAGES.get_or_init(|| {
        parse_locale(
            "en-US",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../src/i18n/locales/en-US.json"
            )),
        )
    })
}

fn ja_jp_messages() -> &'static Value {
    JA_JP_MESSAGES.get_or_init(|| {
        parse_locale(
            "ja-JP",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../src/i18n/locales/ja-JP.json"
            )),
        )
    })
}

fn ko_kr_messages() -> &'static Value {
    KO_KR_MESSAGES.get_or_init(|| {
        parse_locale(
            "ko-KR",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../src/i18n/locales/ko-KR.json"
            )),
        )
    })
}

fn ru_ru_messages() -> &'static Value {
    RU_RU_MESSAGES.get_or_init(|| {
        parse_locale(
            "ru-RU",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../src/i18n/locales/ru-RU.json"
            )),
        )
    })
}

fn parse_locale(code: &str, raw: &str) -> Value {
    serde_json::from_str(raw).unwrap_or_else(|error| {
        panic!("failed to parse locale JSON {code}: {error}");
    })
}

fn locale_messages(locale: AppLocale) -> &'static Value {
    match locale {
        AppLocale::ZhCn => zh_cn_messages(),
        AppLocale::EnUs => en_us_messages(),
        AppLocale::JaJp => ja_jp_messages(),
        AppLocale::KoKr => ko_kr_messages(),
        AppLocale::RuRu => ru_ru_messages(),
    }
}

fn lookup_path<'a>(value: &'a Value, path: &[&str]) -> Option<&'a str> {
    let mut current = value;
    for segment in path {
        current = current.get(*segment)?;
    }
    current.as_str()
}

fn text(locale: AppLocale, path: &[&str]) -> &'static str {
    lookup_path(locale_messages(locale), path)
        .or_else(|| lookup_path(locale_messages(AppLocale::default()), path))
        .unwrap_or("")
}

fn fill_template(template: &str, replacements: &[(&str, String)]) -> String {
    let mut output = template.to_string();
    for (key, value) in replacements {
        output = output.replace(&format!("{{{{{key}}}}}"), value);
    }
    output
}

pub(crate) fn detect_system_locale() -> AppLocale {
    let candidates = [
        env::var("LC_ALL").ok(),
        env::var("LC_MESSAGES").ok(),
        env::var("LANG").ok(),
    ];

    for candidate in candidates.into_iter().flatten() {
        let normalized = candidate.to_lowercase();
        if normalized.starts_with("zh") {
            return AppLocale::ZhCn;
        }
        if normalized.starts_with("en") {
            return AppLocale::EnUs;
        }
        if normalized.starts_with("ja") {
            return AppLocale::JaJp;
        }
        if normalized.starts_with("ko") {
            return AppLocale::KoKr;
        }
        if normalized.starts_with("ru") {
            return AppLocale::RuRu;
        }
    }

    AppLocale::default()
}

pub(crate) fn app_locale(app: &AppHandle) -> AppLocale {
    load_store(app)
        .map(|store| store.settings.locale)
        .unwrap_or_else(|_| detect_system_locale())
}

pub(crate) fn tray_usage_mode_label(locale: AppLocale, mode: TrayUsageDisplayMode) -> &'static str {
    match mode {
        TrayUsageDisplayMode::Used => text(locale, &["settings", "trayUsageDisplay", "used"]),
        TrayUsageDisplayMode::Remaining => {
            text(locale, &["settings", "trayUsageDisplay", "remaining"])
        }
        TrayUsageDisplayMode::Hidden => text(locale, &["settings", "trayUsageDisplay", "hidden"]),
    }
}

pub(crate) fn tray_current_prefix(locale: AppLocale) -> String {
    format!("[{}] ", text(locale, &["accountCard", "currentStamp"]))
}

pub(crate) fn tray_usage_heading(locale: AppLocale) -> &'static str {
    text(locale, &["tray", "usageHeading"])
}

pub(crate) fn tray_display_mode_label(locale: AppLocale) -> &'static str {
    text(locale, &["settings", "trayUsageDisplay", "label"])
}

pub(crate) fn tray_current_label(locale: AppLocale) -> &'static str {
    text(locale, &["tray", "currentLabel"])
}

pub(crate) fn tray_current_account_label(locale: AppLocale) -> &'static str {
    text(locale, &["tray", "currentAccountLabel"])
}

pub(crate) fn tray_no_current(locale: AppLocale) -> &'static str {
    text(locale, &["tray", "noCurrent"])
}

pub(crate) fn tray_no_accounts(locale: AppLocale) -> &'static str {
    text(locale, &["tray", "noAccounts"])
}

pub(crate) fn tray_all_accounts(locale: AppLocale, count: usize) -> String {
    fill_template(
        text(locale, &["tray", "allAccounts"]),
        &[("count", count.to_string())],
    )
}

pub(crate) fn tray_more_accounts(locale: AppLocale, count: usize) -> String {
    fill_template(
        text(locale, &["tray", "moreAccounts"]),
        &[("count", count.to_string())],
    )
}

pub(crate) fn tray_empty_accounts(locale: AppLocale) -> &'static str {
    text(locale, &["tray", "emptyAccounts"])
}

pub(crate) fn tray_refresh_now(locale: AppLocale) -> &'static str {
    text(locale, &["tray", "refreshNow"])
}

pub(crate) fn tray_open_app(locale: AppLocale) -> &'static str {
    text(locale, &["tray", "openApp"])
}

pub(crate) fn tray_quit(locale: AppLocale) -> &'static str {
    text(locale, &["tray", "quit"])
}
