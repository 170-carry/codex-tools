use serde::de::DeserializeOwned;
use serde::Deserialize;
use std::collections::HashSet;
use std::error::Error as StdError;
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

struct UsageRequestFailure {
    message: String,
    transport: bool,
    retryable: bool,
}

async fn fetch_json_from_urls<T: DeserializeOwned>(
    client: &reqwest::Client,
    access_token: &str,
    account_id: &str,
    urls: Vec<String>,
) -> Result<T, String> {
    let mut errors = Vec::new();
    let mut failed_origins = HashSet::new();
    for url in urls {
        let origin = reqwest::Url::parse(&url)
            .ok()
            .map(|url| url.origin().ascii_serialization());
        if origin
            .as_ref()
            .is_some_and(|origin| failed_origins.contains(origin))
        {
            continue;
        }
        for attempt in 0..2 {
            match fetch_json_once(client, access_token, account_id, &url).await {
                Ok(payload) => return Ok(payload),
                Err(failure) if failure.retryable && attempt == 0 => {
                    // A failed CONNECT/TLS handshake is unrelated to the API path.
                    // Retry the correct endpoint before trying legacy addresses.
                    log::debug!("额度请求暂时失败，重试同一地址: {}", failure.message);
                    tokio::time::sleep(USAGE_RETRY_DELAY).await;
                }
                Err(failure) => {
                    if failure.transport {
                        if let Some(origin) = origin.as_ref() {
                            failed_origins.insert(origin.clone());
                        }
                    }
                    errors.push(format!("{url} -> {}", failure.message));
                    break;
                }
            }
        }
    }

    if errors.is_empty() {
        return Err("未命中任何候选地址".to_string());
    }
    let preview = errors
        .iter()
        .take(2)
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join(" | ");
    if errors.len() > 2 {
        Err(format!(
            "{preview} | 另有 {} 个候选地址失败",
            errors.len() - 2
        ))
    } else {
        Err(preview)
    }
}

async fn fetch_json_once<T: DeserializeOwned>(
    client: &reqwest::Client,
    access_token: &str,
    account_id: &str,
    url: &str,
) -> Result<T, UsageRequestFailure> {
    let transport_failure = |error: reqwest::Error| UsageRequestFailure {
        message: format_reqwest_error(&error),
        transport: true,
        // reqwest reports a truncated HTTP body as a decode error. JSON schema
        // errors are handled separately below and must not be retried.
        retryable: error.is_connect() || error.is_timeout() || error.is_request() || error.is_body() || error.is_decode(),
    };
    let response = client
        .get(url)
        .header("Authorization", format!("Bearer {access_token}"))
        .header("ChatGPT-Account-Id", account_id)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(transport_failure)?;
    let status = response.status();
    let body = response.bytes().await.map_err(transport_failure)?;
    if !status.is_success() {
        return Err(UsageRequestFailure {
            message: format!(
                "{status}: {}",
                truncate_for_error(&String::from_utf8_lossy(&body), 140)
            ),
            transport: false,
            retryable: matches!(status.as_u16(), 408 | 502 | 503 | 504),
        });
    }
    serde_json::from_slice(&body).map_err(|error| UsageRequestFailure {
        message: format!("解析返回失败: {error}"),
        transport: false,
        retryable: false,
    })
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

fn format_reqwest_error(err: &reqwest::Error) -> String {
    let mut parts = vec![err.to_string()];
    let mut source = err.source();
    while let Some(next) = source {
        let text = next.to_string();
        if !parts.iter().any(|item| item == &text) {
            parts.push(text);
        }
        source = next.source();
    }
    parts.join(" -> ")
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

fn map_usage_payload(
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

fn map_reset_credits_payload(payload: serde_json::Value) -> ResetCreditsSnapshot {
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

#[cfg(test)]
mod tests {
    use super::fetch_json_from_urls;
    use super::fetch_usage_snapshot_from_urls;
    use super::map_reset_credits_payload;
    use super::map_usage_payload;
    use super::RateLimitDetails;
    use super::UsageApiResponse;
    use super::UsageWindowRaw;
    use serde_json::json;
    use std::time::Duration;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    enum MockReply {
        Disconnect,
        Json(&'static str),
        Unauthorized,
        ServiceUnavailable,
        TruncatedBody,
        Stall,
    }

    async fn mock_server(replies: Vec<MockReply>) -> (String, tokio::task::JoinHandle<Vec<String>>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let handle = tokio::spawn(async move {
            let mut paths = Vec::new();
            for reply in replies {
                let (mut stream, _) = tokio::time::timeout(Duration::from_secs(2), listener.accept())
                    .await
                    .expect("mock request should arrive")
                    .unwrap();
                let mut request = Vec::new();
                while !request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
                    let mut buffer = [0; 1024];
                    let read = stream.read(&mut buffer).await.unwrap();
                    assert!(read > 0, "request headers should be complete");
                    request.extend_from_slice(&buffer[..read]);
                }
                let request = String::from_utf8(request).unwrap();
                paths.push(request.lines().next().unwrap().to_string());
                let (status, body) = match reply {
                    MockReply::Disconnect => continue,
                    MockReply::Json(body) => ("200 OK", body),
                    MockReply::Unauthorized => ("401 Unauthorized", "{}"),
                    MockReply::ServiceUnavailable => ("503 Service Unavailable", "{}"),
                    MockReply::TruncatedBody => {
                        stream
                            .write_all(
                                b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\nConnection: close\r\n\r\n{",
                            )
                            .await
                            .unwrap();
                        continue;
                    }
                    MockReply::Stall => {
                        std::future::pending::<()>().await;
                        unreachable!()
                    }
                };
                let response = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                stream.write_all(response.as_bytes()).await.unwrap();
            }
            paths
        });
        (url, handle)
    }

    fn test_client() -> reqwest::Client {
        reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(1))
            .build()
            .unwrap()
    }

    #[tokio::test]
    async fn transient_transport_failure_retries_the_same_usage_endpoint() {
        let (url, server) = mock_server(vec![
            MockReply::Disconnect,
            MockReply::Json(r#"{"plan_type":"plus"}"#),
        ])
        .await;
        let payload: serde_json::Value = fetch_json_from_urls(
            &test_client(),
            "test-token",
            "test-account",
            vec![format!("{url}/usage"), format!("{url}/legacy")],
        )
        .await
        .unwrap();

        assert_eq!(payload["plan_type"], "plus");
        assert_eq!(server.await.unwrap(), vec!["GET /usage HTTP/1.1"; 2]);
    }

    #[tokio::test]
    async fn transient_http_failure_retries_the_same_usage_endpoint() {
        let (url, server) = mock_server(vec![
            MockReply::ServiceUnavailable,
            MockReply::Json(r#"{"plan_type":"plus"}"#),
        ])
        .await;
        let payload: serde_json::Value = fetch_json_from_urls(
            &test_client(),
            "test-token",
            "test-account",
            vec![format!("{url}/usage"), format!("{url}/legacy")],
        )
        .await
        .unwrap();

        assert_eq!(payload["plan_type"], "plus");
        assert_eq!(server.await.unwrap(), vec!["GET /usage HTTP/1.1"; 2]);
    }

    #[tokio::test]
    async fn interrupted_response_body_retries_the_same_usage_endpoint() {
        let (url, server) = mock_server(vec![
            MockReply::TruncatedBody,
            MockReply::Json(r#"{"plan_type":"plus"}"#),
        ])
        .await;
        let payload: serde_json::Value = fetch_json_from_urls(
            &test_client(),
            "test-token",
            "test-account",
            vec![format!("{url}/usage"), format!("{url}/legacy")],
        )
        .await
        .unwrap();

        assert_eq!(payload["plan_type"], "plus");
        assert_eq!(server.await.unwrap(), vec!["GET /usage HTTP/1.1"; 2]);
    }

    #[tokio::test]
    async fn authorization_failure_does_not_retry_before_endpoint_fallback() {
        let (url, server) = mock_server(vec![
            MockReply::Unauthorized,
            MockReply::Json(r#"{"plan_type":"plus"}"#),
        ])
        .await;
        let _: serde_json::Value = fetch_json_from_urls(
            &test_client(),
            "test-token",
            "test-account",
            vec![format!("{url}/usage"), format!("{url}/legacy")],
        )
        .await
        .unwrap();

        assert_eq!(
            server.await.unwrap(),
            vec!["GET /usage HTTP/1.1", "GET /legacy HTTP/1.1"]
        );
    }

    #[tokio::test]
    async fn transport_failure_skips_other_paths_on_the_same_origin() {
        let (unavailable, failing_server) =
            mock_server(vec![MockReply::Disconnect, MockReply::Disconnect]).await;
        let (fallback, fallback_server) =
            mock_server(vec![MockReply::Json(r#"{"plan_type":"plus"}"#)]).await;
        let _: serde_json::Value = fetch_json_from_urls(
            &test_client(),
            "test-token",
            "test-account",
            vec![
                format!("{unavailable}/usage"),
                format!("{unavailable}/legacy"),
                format!("{fallback}/usage"),
            ],
        )
        .await
        .unwrap();

        assert_eq!(failing_server.await.unwrap(), vec!["GET /usage HTTP/1.1"; 2]);
        assert_eq!(fallback_server.await.unwrap(), vec!["GET /usage HTTP/1.1"]);
    }

    #[tokio::test]
    async fn usage_budget_bounds_a_stalled_request_before_the_client_timeout() {
        let (url, server) = mock_server(vec![MockReply::Stall]).await;
        let result = tokio::time::timeout(
            Duration::from_millis(500),
            fetch_usage_snapshot_from_urls(
                &test_client(),
                "test-token",
                "test-account",
                vec![format!("{url}/usage")],
                vec![format!("{url}/reset")],
                Duration::from_millis(30),
                Duration::from_secs(1),
            ),
        )
        .await
        .expect("the overall usage budget should bound a stalled request");
        server.abort();

        assert!(result.unwrap_err().contains("网络请求超时"));
    }

    #[tokio::test]
    async fn optional_reset_cards_cannot_block_a_successful_usage_snapshot() {
        let (url, server) = mock_server(vec![
            MockReply::Json(r#"{"plan_type":"plus"}"#),
            MockReply::Stall,
        ])
        .await;
        let result = tokio::time::timeout(
            Duration::from_millis(500),
            fetch_usage_snapshot_from_urls(
                &test_client(),
                "test-token",
                "test-account",
                vec![format!("{url}/usage")],
                vec![format!("{url}/reset")],
                Duration::from_secs(1),
                Duration::from_millis(30),
            ),
        )
        .await
        .expect("quota should be returned without waiting for optional reset cards")
        .unwrap();
        server.abort();

        assert_eq!(result.plan_type.as_deref(), Some("plus"));
        assert!(result.reset_credits.is_none());
    }

    #[test]
    fn zero_five_hour_window_remains_independent_from_weekly_usage() {
        let snapshot = map_usage_payload(
            UsageApiResponse {
                plan_type: Some("plus".to_string()),
                rate_limit: Some(RateLimitDetails {
                    primary_window: Some(UsageWindowRaw {
                        used_percent: 0.0,
                        limit_window_seconds: 5 * 60 * 60,
                        reset_at: 1_800,
                    }),
                    secondary_window: Some(UsageWindowRaw {
                        used_percent: 37.0,
                        limit_window_seconds: 7 * 24 * 60 * 60,
                        reset_at: 604_800,
                    }),
                }),
                additional_rate_limits: None,
                credits: None,
            },
            None,
        );

        assert_eq!(snapshot.five_hour.unwrap().used_percent, 0.0);
        assert_eq!(snapshot.one_week.unwrap().used_percent, 37.0);
    }

    #[test]
    fn nonzero_five_hour_window_remains_independent_from_weekly_usage() {
        let snapshot = map_usage_payload(
            UsageApiResponse {
                plan_type: Some("plus".to_string()),
                rate_limit: Some(RateLimitDetails {
                    primary_window: Some(UsageWindowRaw {
                        used_percent: 12.0,
                        limit_window_seconds: 5 * 60 * 60,
                        reset_at: 1_800,
                    }),
                    secondary_window: Some(UsageWindowRaw {
                        used_percent: 37.0,
                        limit_window_seconds: 7 * 24 * 60 * 60,
                        reset_at: 604_800,
                    }),
                }),
                additional_rate_limits: None,
                credits: None,
            },
            None,
        );

        assert_eq!(snapshot.five_hour.unwrap().used_percent, 12.0);
        assert_eq!(snapshot.one_week.unwrap().used_percent, 37.0);
    }

    #[test]
    fn reset_credits_payload_accepts_common_timestamp_shapes() {
        let snapshot = map_reset_credits_payload(json!({
            "available_count": 2,
            "credits": [
                {
                    "granted_at": 1_797_392_400,
                    "expires_at": 1_799_984_400_000i64
                },
                {
                    "createdAt": "2026-12-15T10:00:00Z",
                    "expiresAt": "2027-01-14T10:00:00Z"
                }
            ]
        }));

        assert_eq!(snapshot.available_count, Some(2));
        assert_eq!(snapshot.credits.len(), 2);
        assert_eq!(snapshot.credits[0].granted_at, Some(1_797_392_400));
        assert_eq!(snapshot.credits[0].expires_at, Some(1_799_984_400));
        assert_eq!(snapshot.credits[1].granted_at, Some(1_797_328_800));
        assert_eq!(snapshot.credits[1].expires_at, Some(1_799_920_800));
    }

    #[test]
    fn reset_credits_payload_accepts_camel_case_available_count() {
        let snapshot = map_reset_credits_payload(json!({
            "availableCount": "1",
            "credits": [
                {
                    "obtained_at": "1797392400",
                    "expireAt": null
                }
            ]
        }));

        assert_eq!(snapshot.available_count, Some(1));
        assert_eq!(snapshot.credits[0].granted_at, Some(1_797_392_400));
        assert_eq!(snapshot.credits[0].expires_at, None);
    }
}
