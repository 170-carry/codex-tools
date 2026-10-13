//! 代理池只汇总可核实的额度百分比；不同套餐的百分比不表示相同 Token 数。
use crate::models::{
    AccountSourceKind, AccountSummary, AppLocale, TrayUsageDisplayMode, UsageWindow,
};
use std::collections::HashMap;

struct PoolWindow {
    used: Option<f64>,
    known: usize,
    total: usize,
}

fn window_average(accounts: &[AccountSummary], weekly: bool, now: i64) -> PoolWindow {
    // 与代理候选一致，同一账号只保留最新且未被授权错误阻塞的变体。
    let mut groups: HashMap<&str, &AccountSummary> = HashMap::new();
    for account in accounts
        .iter()
        .filter(|a| a.api_proxy_enabled && matches!(a.source_kind, AccountSourceKind::Chatgpt))
    {
        groups
            .entry(&account.account_key)
            .and_modify(|old| {
                if (old.auth_refresh_blocked && !account.auth_refresh_blocked)
                    || (old.auth_refresh_blocked == account.auth_refresh_blocked
                        && account.updated_at > old.updated_at)
                {
                    *old = account;
                }
            })
            .or_insert(account);
    }
    let total = groups.len();
    let values: Vec<f64> = groups
        .values()
        .filter_map(|a| {
            if a.auth_refresh_blocked || a.usage_error.is_some() {
                return None;
            }
            let usage = a.usage.as_ref()?;
            // 过期快照和刷新失败不能冒充可用额度；不额外发起网络查询。
            if usage.fetched_at <= 0 || now.saturating_sub(usage.fetched_at) > 600 {
                return None;
            }
            let window: &UsageWindow = if weekly {
                usage.one_week.as_ref()?
            } else {
                usage.five_hour.as_ref()?
            };
            if !window.used_percent.is_finite() || window.reset_at.is_some_and(|reset| reset <= now)
            {
                return None;
            }
            Some(window.used_percent.clamp(0.0, 100.0))
        })
        .collect();
    PoolWindow {
        used: (!values.is_empty()).then(|| values.iter().sum::<f64>() / values.len() as f64),
        known: values.len(),
        total,
    }
}

pub(super) fn title(
    accounts: &[AccountSummary],
    mode: TrayUsageDisplayMode,
    labels: bool,
) -> String {
    if mode == TrayUsageDisplayMode::Hidden {
        return String::new();
    }
    let now = crate::utils::now_unix_seconds();
    let format = |weekly| {
        let window = window_average(accounts, weekly, now);
        let percent = super::format_percent(window.used.map(|used| {
            if mode == TrayUsageDisplayMode::Used {
                used
            } else {
                100.0 - used
            }
        }));
        let label = if labels {
            if weekly {
                "1w "
            } else {
                "5h "
            }
        } else {
            ""
        };
        format!("{label}{percent} ({}/{})", window.known, window.total)
    };
    match mode {
        TrayUsageDisplayMode::FiveHourRemaining => format!("AVG {}", format(false)),
        TrayUsageDisplayMode::OneWeekRemaining => format!("AVG {}", format(true)),
        _ => format!("AVG {} / {}", format(false), format(true)),
    }
}

pub(super) fn percent(accounts: &[AccountSummary], mode: TrayUsageDisplayMode) -> Option<f64> {
    let mode = super::quota_icon_mode(mode);
    let now = crate::utils::now_unix_seconds();
    let five = window_average(accounts, false, now).used.map(|v| 100.0 - v);
    let week = window_average(accounts, true, now).used.map(|v| 100.0 - v);
    match mode {
        TrayUsageDisplayMode::FiveHourRemaining => five,
        TrayUsageDisplayMode::OneWeekRemaining => week,
        _ => five.zip(week).map(|(a, b)| a.min(b)),
    }
}

pub(super) fn summary(accounts: &[AccountSummary], locale: AppLocale) -> String {
    let heading = match locale {
        AppLocale::ZhCn => "代理池平均剩余（有效账号 / 总账号，非 Token 总量）",
        AppLocale::JaJp => "プロキシプール平均残量（有効 / 合計、トークン総量ではありません）",
        AppLocale::KoKr => "프록시 풀 평균 잔여량 (유효 / 전체, 총 토큰 아님)",
        AppLocale::RuRu => "Средний остаток пула (известно / всего, не сумма токенов)",
        _ => "Proxy pool average remaining (known / total, not total tokens)",
    };
    format!(
        "{heading}: {}",
        title(accounts, TrayUsageDisplayMode::Remaining, true)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pool_deduplicates_accounts_and_excludes_disabled_stale_and_relay_data() {
        let mut a = super::super::tests::current_account_with_usage();
        a.api_proxy_enabled = true;
        let mut duplicate = a.clone();
        duplicate.updated_at = -1;
        let mut b = a.clone();
        b.account_key = "b".into();
        b.usage
            .as_mut()
            .unwrap()
            .five_hour
            .as_mut()
            .unwrap()
            .used_percent = 20.0;
        let mut stale = a.clone();
        stale.account_key = "stale".into();
        stale.usage.as_mut().unwrap().fetched_at = 1;
        let mut disabled = a.clone();
        disabled.account_key = "disabled".into();
        disabled.api_proxy_enabled = false;
        let mut relay = a.clone();
        relay.source_kind = AccountSourceKind::Relay;
        relay.account_key = "relay".into();
        let average = window_average(
            &[a, duplicate, b, stale, disabled, relay],
            false,
            crate::utils::now_unix_seconds(),
        );
        assert_eq!(average.used, Some(40.0));
        assert_eq!((average.known, average.total), (2, 3));
        assert_eq!(window_average(&[], false, 10).used, None);
    }
}
