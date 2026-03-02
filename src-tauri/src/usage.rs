use serde::Deserialize;

use crate::models::CreditSnapshot;
use crate::models::UsageSnapshot;
use crate::models::UsageWindow;
use crate::utils::now_unix_seconds;
use crate::utils::truncate_for_error;

const DEFAULT_CHATGPT_BASE_URL: &str = "https://chatgpt.com";
const CODEX_USAGE_PATH: &str = "/api/codex/usage";
const WHAM_USAGE_PATH: &str = "/wham/usage";

#[derive(Debug, Deserialize)]
struct UsageApiResponse {
    plan_type: Option<String>,
    rate_limit: Option<RateLimitDetails>,
    additional_rate_limits: Option<Vec<AdditionalRateLimitDetails>>,
    credits: Option<CreditDetails>,
}

#[derive(Debug, Deserialize)]
struct RateLimitDetails {
    primary_window: Option<UsageWindowRaw>,
    secondary_window: Option<UsageWindowRaw>,
}

#[derive(Debug, Deserialize)]
struct AdditionalRateLimitDetails {
    rate_limit: Option<RateLimitDetails>,
}

#[derive(Debug, Deserialize)]
struct UsageWindowRaw {
    used_percent: f64,
    limit_window_seconds: i64,
    reset_at: i64,
}

#[derive(Debug, Deserialize)]
struct CreditDetails {
    has_credits: bool,
    unlimited: bool,
    balance: Option<String>,
}

pub(crate) async fn fetch_usage_snapshot(
    access_token: &str,
    account_id: &str,
) -> Result<UsageSnapshot, String> {
    let usage_urls = resolve_usage_urls();

    let client = reqwest::Client::builder()
        .user_agent("codex-tools/0.1")
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {e}"))?;

    let mut errors: Vec<String> = Vec::new();
    for usage_url in usage_urls {
        let response = match client
            .get(&usage_url)
            .header("Authorization", format!("Bearer {access_token}"))
            .header("ChatGPT-Account-Id", account_id)
            .send()
            .await
        {
            Ok(response) => response,
            Err(err) => {
                errors.push(format!("{usage_url} -> {err}"));
                continue;
            }
        };

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            errors.push(format!(
                "{usage_url} -> {status}: {}",
                truncate_for_error(&body, 140)
            ));
            continue;
        }

        let payload: UsageApiResponse = match response.json().await {
            Ok(payload) => payload,
            Err(err) => {
                errors.push(format!("{usage_url} -> 解析返回失败: {err}"));
                continue;
            }
        };
        return Ok(map_usage_payload(payload));
    }

    if errors.is_empty() {
        return Err("请求用量接口失败: 未命中任何候选地址".to_string());
    }
    let preview = errors
        .iter()
        .take(2)
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join(" | ");
    if errors.len() > 2 {
        Err(format!(
            "请求用量接口失败: {preview} | 另有 {} 个候选地址失败",
            errors.len() - 2
        ))
    } else {
        Err(format!("请求用量接口失败: {preview}"))
    }
}

fn resolve_usage_urls() -> Vec<String> {
    let base_url =
        read_chatgpt_base_url_from_config().unwrap_or_else(|| DEFAULT_CHATGPT_BASE_URL.to_string());
    let normalized = base_url.trim_end_matches('/');
    let mut candidates = Vec::new();

    if let Some(origin) = normalized.strip_suffix("/backend-api") {
        candidates.push(format!("{origin}{CODEX_USAGE_PATH}"));
        candidates.push(format!("{normalized}{WHAM_USAGE_PATH}"));
    } else {
        candidates.push(format!("{normalized}{CODEX_USAGE_PATH}"));
        candidates.push(format!("{normalized}/backend-api{WHAM_USAGE_PATH}"));
    }

    candidates.push(format!("https://chatgpt.com{CODEX_USAGE_PATH}"));
    candidates.push("https://chatgpt.com/backend-api/wham/usage".to_string());

    let mut deduped = Vec::new();
    for url in candidates {
        if !deduped.iter().any(|existing| existing == &url) {
            deduped.push(url);
        }
    }
    deduped
}

fn read_chatgpt_base_url_from_config() -> Option<String> {
    let home = dirs::home_dir()?;
    let config_path = home.join(".codex").join("config.toml");
    let contents = std::fs::read_to_string(config_path).ok()?;

    for line in contents.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("chatgpt_base_url") {
            continue;
        }

        let (_, value) = trimmed.split_once('=')?;
        let cleaned = value.trim().trim_matches('"').trim_matches('\'');
        if !cleaned.is_empty() {
            return Some(cleaned.to_string());
        }
    }

    None
}

fn map_usage_payload(payload: UsageApiResponse) -> UsageSnapshot {
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
