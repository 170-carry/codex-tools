use super::*;

pub(super) fn map_usage_payload(
    payload: UsageApiResponse,
    reset_credits: Option<ResetCreditsSnapshot>,
) -> UsageSnapshot {
    let mut windows: Vec<UsageWindowRaw> = Vec::new();

    if let Some(rate_limit) = payload.rate_limit {
        if let Some(primary) = rate_limit.primary_window {
            windows.push(primary);
        }
        if let Some(secondary) = rate_limit.secondary_window {
            windows.push(secondary);
        }
    }

    if let Some(additional) = payload.additional_rate_limits {
        for limit in additional {
            if let Some(rate_limit) = limit.rate_limit {
                if let Some(primary) = rate_limit.primary_window {
                    windows.push(primary);
                }
                if let Some(secondary) = rate_limit.secondary_window {
                    windows.push(secondary);
                }
            }
        }
    }

    let five_hour = pick_nearest_window(&windows, 5 * 60 * 60).map(to_usage_window);
    let one_week = pick_nearest_window(&windows, 7 * 24 * 60 * 60).map(to_usage_window);

    // A zero percentage is a valid freshly reset window. Keep each API window
    // independent instead of inferring one window's value from another.
    UsageSnapshot {
        fetched_at: now_unix_seconds(),
        plan_type: payload.plan_type,
        five_hour,
        one_week,
        credits: payload.credits.map(|credit| CreditSnapshot {
            has_credits: credit.has_credits,
            unlimited: credit.unlimited,
            balance: credit.balance,
        }),
        reset_credits,
    }
}

pub(super) fn map_reset_credits_payload(payload: serde_json::Value) -> ResetCreditsSnapshot {
    let available_count = payload
        .get("available_count")
        .or_else(|| payload.get("availableCount"))
        .and_then(parse_optional_i64);

    let credits = payload
        .get("credits")
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|item| ResetCredit {
                    id: item
                        .get("id")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_string),
                    status: item
                        .get("status")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_string),
                    granted_at: get_timestamp_field(
                        item,
                        &[
                            "granted_at",
                            "grantedAt",
                            "created_at",
                            "createdAt",
                            "obtained_at",
                            "obtainedAt",
                        ],
                    ),
                    expires_at: get_timestamp_field(
                        item,
                        &[
                            "expires_at",
                            "expiresAt",
                            "expiration",
                            "expire_at",
                            "expireAt",
                        ],
                    ),
                })
                .collect()
        })
        .unwrap_or_default();

    ResetCreditsSnapshot {
        available_count,
        credits,
    }
}

fn get_timestamp_field(value: &serde_json::Value, keys: &[&str]) -> Option<i64> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(parse_optional_i64))
}

fn parse_optional_i64(value: &serde_json::Value) -> Option<i64> {
    if let Some(number) = value.as_i64() {
        return Some(normalize_epoch_seconds(number));
    }

    if let Some(number) = value.as_u64() {
        return i64::try_from(number).ok().map(normalize_epoch_seconds);
    }

    if let Some(text) = value
        .as_str()
        .map(str::trim)
        .filter(|text| !text.is_empty())
    {
        if let Ok(number) = text.parse::<i64>() {
            return Some(normalize_epoch_seconds(number));
        }
        if let Ok(parsed) =
            time::OffsetDateTime::parse(text, &time::format_description::well_known::Rfc3339)
        {
            return Some(parsed.unix_timestamp());
        }
    }

    None
}

fn normalize_epoch_seconds(value: i64) -> i64 {
    if value > 10_000_000_000 {
        value / 1000
    } else {
        value
    }
}

fn pick_nearest_window(windows: &[UsageWindowRaw], target_seconds: i64) -> Option<UsageWindowRaw> {
    windows
        .iter()
        .min_by_key(|window| (window.limit_window_seconds - target_seconds).abs())
        .map(|window| UsageWindowRaw {
            used_percent: window.used_percent,
            limit_window_seconds: window.limit_window_seconds,
            reset_at: window.reset_at,
        })
}

fn to_usage_window(window: UsageWindowRaw) -> UsageWindow {
    UsageWindow {
        used_percent: window.used_percent,
        window_seconds: window.limit_window_seconds,
        reset_at: Some(window.reset_at),
    }
}
