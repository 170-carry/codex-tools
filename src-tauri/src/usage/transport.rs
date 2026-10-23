use super::*;
use std::collections::HashSet;
use std::error::Error as StdError;

struct UsageRequestFailure {
    message: String,
    transport: bool,
    retryable: bool,
}

pub(super) async fn fetch_json_from_urls<T: DeserializeOwned>(
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
        retryable: error.is_connect()
            || error.is_timeout()
            || error.is_request()
            || error.is_body()
            || error.is_decode(),
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
