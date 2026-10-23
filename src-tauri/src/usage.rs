#[path = "usage/mapping.rs"]
mod mapping;
#[path = "usage/transport.rs"]
mod transport;
use mapping::{map_reset_credits_payload, map_usage_payload};
use transport::fetch_json_from_urls;

use serde::de::DeserializeOwned;
use serde::Deserialize;
use std::time::Duration;

use crate::app_paths;
use crate::models::CreditSnapshot;
use crate::models::ResetCredit;
use crate::models::ResetCreditsSnapshot;
use crate::models::UsageSnapshot;
use crate::models::UsageWindow;
use crate::utils::now_unix_seconds;
use crate::utils::truncate_for_error;

const DEFAULT_CHATGPT_BASE_URL: &str = "https://chatgpt.com";
const CODEX_USAGE_PATH: &str = "/api/codex/usage";
const WHAM_USAGE_PATH: &str = "/wham/usage";
const WHAM_RESET_CREDITS_PATH: &str = "/wham/rate-limit-reset-credits";
const BACKEND_API_PREFIX: &str = "/backend-api";
const USAGE_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const USAGE_FETCH_BUDGET: Duration = Duration::from_secs(24);
const RESET_CREDITS_FETCH_BUDGET: Duration = Duration::from_secs(3);
const USAGE_RETRY_DELAY: Duration = Duration::from_millis(250);

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
    let client = create_usage_http_client()?;
    fetch_usage_snapshot_with_client(&client, access_token, account_id).await
}

pub(crate) fn create_usage_http_client() -> Result<reqwest::Client, String> {
    // Build once per refresh batch, so all accounts share connections while a new
    // batch still picks up changes to the Windows/Verge system proxy settings.
    reqwest::Client::builder()
        .user_agent("codex-tools/0.1")
        .connect_timeout(Duration::from_secs(6))
        .timeout(USAGE_REQUEST_TIMEOUT)
        .pool_idle_timeout(Duration::from_secs(15))
        .pool_max_idle_per_host(4)
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {e}"))
}

pub(crate) async fn fetch_usage_snapshot_with_client(
    client: &reqwest::Client,
    access_token: &str,
    account_id: &str,
) -> Result<UsageSnapshot, String> {
    fetch_usage_snapshot_from_urls(
        client,
        access_token,
        account_id,
        resolve_usage_urls(),
        resolve_reset_credit_urls(),
        USAGE_FETCH_BUDGET,
        RESET_CREDITS_FETCH_BUDGET,
    )
    .await
}

async fn fetch_usage_snapshot_from_urls(
    client: &reqwest::Client,
    access_token: &str,
    account_id: &str,
    usage_urls: Vec<String>,
    reset_urls: Vec<String>,
    usage_budget: Duration,
    reset_budget: Duration,
) -> Result<UsageSnapshot, String> {
    let payload = tokio::time::timeout(
        usage_budget,
        fetch_json_from_urls(client, access_token, account_id, usage_urls),
    )
    .await
    .map_err(|_| "请求用量接口失败: 网络请求超时（含重试最多 24 秒）".to_string())?
    .map_err(|error| format!("请求用量接口失败: {error}"))?;

    // Reset cards are optional metadata. Their endpoint must not hold up a
    // successful quota response for another full sequence of network timeouts.
    let reset_credits = match tokio::time::timeout(
        reset_budget,
        fetch_json_from_urls(client, access_token, account_id, reset_urls),
    )
    .await
    {
        Ok(Ok(payload)) => Some(map_reset_credits_payload(payload)),
        Ok(Err(error)) => {
            log::debug!("获取 Codex 重置卡失败: {error}");
            None
        }
        Err(_) => {
            log::debug!("获取 Codex 重置卡超时，保留已取得的额度结果");
            None
        }
    };
    Ok(map_usage_payload(payload, reset_credits))
}

fn resolve_usage_urls() -> Vec<String> {
    let normalized = resolve_chatgpt_base_origin();
    let mut candidates = Vec::new();

    if let Some(origin) = normalized.strip_suffix(BACKEND_API_PREFIX) {
        // 优先 backend-api/wham/usage（兼容性最好）
        candidates.push(format!("{normalized}{WHAM_USAGE_PATH}"));
        candidates.push(format!("{origin}{BACKEND_API_PREFIX}{WHAM_USAGE_PATH}"));
        candidates.push(format!("{origin}{CODEX_USAGE_PATH}"));
    } else {
        candidates.push(format!("{normalized}{BACKEND_API_PREFIX}{WHAM_USAGE_PATH}"));
        candidates.push(format!("{normalized}{WHAM_USAGE_PATH}"));
        candidates.push(format!("{normalized}{CODEX_USAGE_PATH}"));
    }

    candidates.push("https://chatgpt.com/backend-api/wham/usage".to_string());
    candidates.push(format!("https://chatgpt.com{CODEX_USAGE_PATH}"));

    let mut deduped = Vec::new();
    for url in candidates {
        if !deduped.iter().any(|existing| existing == &url) {
            deduped.push(url);
        }
    }
    deduped
}

fn resolve_reset_credit_urls() -> Vec<String> {
    let normalized = resolve_chatgpt_base_origin();
    let mut candidates = Vec::new();

    if let Some(origin) = normalized.strip_suffix(BACKEND_API_PREFIX) {
        candidates.push(format!("{normalized}{WHAM_RESET_CREDITS_PATH}"));
        candidates.push(format!(
            "{origin}{BACKEND_API_PREFIX}{WHAM_RESET_CREDITS_PATH}"
        ));
    } else {
        candidates.push(format!(
            "{normalized}{BACKEND_API_PREFIX}{WHAM_RESET_CREDITS_PATH}"
        ));
        candidates.push(format!("{normalized}{WHAM_RESET_CREDITS_PATH}"));
    }

    candidates.push(format!(
        "https://chatgpt.com{BACKEND_API_PREFIX}{WHAM_RESET_CREDITS_PATH}"
    ));

    let mut deduped = Vec::new();
    for url in candidates {
        if !deduped.iter().any(|existing| existing == &url) {
            deduped.push(url);
        }
    }
    deduped
}

pub(crate) fn resolve_chatgpt_base_origin() -> String {
    let base_url =
        read_chatgpt_base_url_from_config().unwrap_or_else(|| DEFAULT_CHATGPT_BASE_URL.to_string());
    base_url.trim_end_matches('/').to_string()
}

fn read_chatgpt_base_url_from_config() -> Option<String> {
    let config_path = app_paths::codex_config_path().ok()?;
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

#[cfg(test)]
#[path = "usage/tests.rs"]
mod tests;
