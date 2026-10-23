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

    assert_eq!(
        failing_server.await.unwrap(),
        vec!["GET /usage HTTP/1.1"; 2]
    );
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
