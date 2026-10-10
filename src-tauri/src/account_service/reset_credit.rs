//! 重置卡消费：指定卡片、提交前回读、持久化幂等 ID，禁止自动消费下一张卡。
use crate::models::AccountSummary;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ResetCreditResult {
    pub(crate) code: String,
    pub(crate) windows_reset: u32,
    pub(crate) accounts: Vec<AccountSummary>,
    pub(crate) refresh_error: Option<String>,
}

#[derive(Deserialize)]
struct CreditList {
    credits: Vec<Credit>,
}
#[derive(Deserialize)]
struct Credit {
    id: String,
    status: String,
    expires_at: Option<String>,
}
#[derive(Debug, Deserialize)]
struct Outcome {
    code: String,
    #[serde(default)]
    windows_reset: u32,
}

async fn consume(
    client: &reqwest::Client,
    endpoint: &str,
    token: &str,
    account: &str,
    credit: &str,
    request_id: &str,
) -> Result<Outcome, String> {
    let response = client
        .get(endpoint)
        .bearer_auth(token)
        .header("ChatGPT-Account-Id", account)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|_| "无法读取最新重置卡状态，未提交消费请求".to_string())?;
    if !response.status().is_success() {
        return Err(format!(
            "读取重置卡失败（HTTP {}），未提交消费请求",
            response.status().as_u16()
        ));
    }
    let cards: CreditList = response
        .json()
        .await
        .map_err(|_| "重置卡列表格式无法确认，未提交消费请求".to_string())?;
    let selected = cards
        .credits
        .iter()
        .find(|c| c.id == credit && c.status == "available")
        .ok_or_else(|| "所选重置卡已不可用，请刷新用量后重新选择".to_string())?;
    if let Some(expires) = &selected.expires_at {
        let expires =
            time::OffsetDateTime::parse(expires, &time::format_description::well_known::Rfc3339)
                .map_err(|_| "无法确认重置卡有效期，未提交消费请求".to_string())?;
        if expires.unix_timestamp() <= crate::utils::now_unix_seconds() {
            return Err("所选重置卡已过期，未提交消费请求".to_string());
        }
    }
    // POST 没有候选地址回退或自动重试；网络失败不能证明上游没有消费卡片。
    let response = client
        .post(format!("{endpoint}/consume"))
        .bearer_auth(token)
        .header("ChatGPT-Account-Id", account)
        .header("Accept", "application/json")
        .json(&serde_json::json!({ "credit_id": credit, "redeem_request_id": request_id }))
        .send()
        .await
        .map_err(|_| {
            "重置卡提交结果未知，请刷新用量确认；重试同一张卡会复用原请求 ID".to_string()
        })?;
    if !response.status().is_success() {
        return Err(format!(
            "重置卡提交返回 HTTP {}，结果未确认，请刷新用量核对",
            response.status().as_u16()
        ));
    }
    let outcome: Outcome = response
        .json()
        .await
        .map_err(|_| "重置卡响应无法解析，结果未知，请刷新用量确认".to_string())?;
    match outcome.code.as_str() {
        "reset" | "nothing_to_reset" | "no_credit" | "already_redeemed" => Ok(outcome),
        _ => Err("重置卡返回未知结果，请刷新用量确认".to_string()),
    }
}

pub(crate) async fn use_credit(
    app: &tauri::AppHandle,
    state: &crate::state::AppState,
    id: &str,
    credit_id: &str,
) -> Result<ResetCreditResult, String> {
    // 与预热串行，避免刚重置的窗口立即被本工具预热；网络请求期间不持有存储锁。
    let _operation = state.account_warmup_lock.lock().await;
    let (account, request_id) = {
        let _guard = state.store_lock.lock().await;
        let mut store = crate::store::load_store(app)?;
        let account = store
            .accounts
            .iter()
            .find(|a| a.id == id)
            .cloned()
            .ok_or("找不到账号")?;
        if !matches!(
            account.source_kind,
            crate::models::AccountSourceKind::Chatgpt
        ) || account.auth_refresh_blocked
        {
            return Err("此账号暂不支持使用重置卡，请先检查授权".to_string());
        }
        if credit_id.trim().is_empty() {
            return Err("必须指定重置卡".to_string());
        }
        let key = serde_json::to_string(&(account.account_key(), credit_id))
            .map_err(|e| e.to_string())?;
        let request_id = store
            .settings
            .reset_credit_requests
            .entry(key)
            .or_insert_with(|| uuid::Uuid::new_v4().to_string())
            .clone();
        // 在发送前落盘；进程重启后继续重试同一张卡也不会更换幂等 ID。
        crate::store::save_store(app, &store)?;
        (account, request_id)
    };
    let mut auth = crate::account_service::refresh_latest_auth_json_if_newer(
        app,
        state,
        &account.account_key(),
        &account.auth_json,
    )
    .await;
    if crate::auth::auth_tokens_need_refresh(&auth) {
        auth = crate::account_service::refresh_account_auth_with_operation_guard(
            app,
            state,
            &account.account_key(),
            &auth,
        )
        .await
        .map_err(|_| "重置前刷新授权失败，请重新授权账号".to_string())?
        .auth_json;
    }
    let extracted = crate::auth::extract_auth(&auth)?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(18))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| format!("创建重置卡客户端失败: {e}"))?;
    let origin = crate::usage::resolve_chatgpt_base_origin();
    let base = if origin.ends_with("/backend-api") {
        origin
    } else {
        format!("{origin}/backend-api")
    };
    let outcome = consume(
        &client,
        &format!("{base}/wham/rate-limit-reset-credits"),
        &extracted.access_token,
        &extracted.account_id,
        credit_id,
        &request_id,
    )
    .await?;
    // 只刷新当前账号组；消费结果和刷新结果分别返回，避免已消费却显示操作失败。
    let usage =
        crate::usage::fetch_usage_snapshot(&extracted.access_token, &extracted.account_id).await;
    let refresh_error = usage.as_ref().err().cloned();
    {
        let _guard = state.store_lock.lock().await;
        let mut store = crate::store::load_store(app)?;
        for item in store
            .accounts
            .iter_mut()
            .filter(|a| a.account_key() == account.account_key())
        {
            match &usage {
                Ok(snapshot) => {
                    item.usage = Some(snapshot.clone());
                    item.usage_error = None;
                }
                Err(error) => {
                    item.usage = None;
                    item.usage_error = Some(error.clone());
                }
            }
        }
        if outcome.code == "nothing_to_reset" {
            // 上游明确未消费，后续窗口有用量后允许用户发起一次新的兑换。
            let key = serde_json::to_string(&(account.account_key(), credit_id))
                .map_err(|e| e.to_string())?;
            store.settings.reset_credit_requests.remove(&key);
        }
        crate::store::save_store(app, &store)?;
    }
    Ok(ResetCreditResult {
        code: outcome.code,
        windows_reset: outcome.windows_reset,
        accounts: crate::account_service::list_accounts_internal(app, state).await?,
        refresh_error,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{extract::State, http::HeaderMap, routing::get, Json, Router};
    use std::sync::{Arc, Mutex};

    #[derive(Clone)]
    struct Mock {
        status: &'static str,
        code: &'static str,
        expires: Option<&'static str>,
        get_status: u16,
        post_status: u16,
        calls: Arc<Mutex<Vec<serde_json::Value>>>,
    }
    async fn list(State(mock): State<Mock>) -> (axum::http::StatusCode, Json<serde_json::Value>) {
        (
            axum::http::StatusCode::from_u16(mock.get_status).unwrap(),
            Json(
                serde_json::json!({"credits":[{"id":"card", "status":mock.status, "expires_at":mock.expires}]}),
            ),
        )
    }
    async fn redeem(
        State(mock): State<Mock>,
        headers: HeaderMap,
        Json(body): Json<serde_json::Value>,
    ) -> (axum::http::StatusCode, Json<serde_json::Value>) {
        assert_eq!(headers["authorization"], "Bearer test-token");
        assert_eq!(headers["chatgpt-account-id"], "test-account");
        mock.calls.lock().unwrap().push(body);
        (
            axum::http::StatusCode::from_u16(mock.post_status).unwrap(),
            Json(serde_json::json!({"code":mock.code,"windows_reset":2})),
        )
    }
    #[tokio::test]
    async fn consume_requires_available_card_and_preserves_idempotency_payload() {
        for (status, code, expires, get_status, post_status, expected_calls, succeeds) in [
            ("available", "reset", None, 200, 200, 1, true),
            ("redeemed", "reset", None, 200, 200, 0, false),
            ("available", "unknown", None, 200, 200, 1, false),
            ("available", "nothing_to_reset", None, 200, 200, 1, true),
            ("available", "already_redeemed", None, 200, 200, 1, true),
            ("available", "no_credit", None, 200, 200, 1, true),
            (
                "available",
                "reset",
                Some("2020-01-01T00:00:00Z"),
                200,
                200,
                0,
                false,
            ),
            (
                "available",
                "reset",
                Some("invalid-date"),
                200,
                200,
                0,
                false,
            ),
            ("available", "reset", None, 503, 200, 0, false),
            ("available", "reset", None, 200, 503, 1, false),
        ] {
            let calls = Arc::new(Mutex::new(Vec::new()));
            let mock = Mock {
                status,
                code,
                expires,
                get_status,
                post_status,
                calls: calls.clone(),
            };
            let router = Router::new()
                .route("/credits", get(list))
                .route("/credits/consume", axum::routing::post(redeem))
                .with_state(mock);
            let socket = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = socket.local_addr().unwrap();
            let task = tokio::spawn(async move {
                axum::serve(socket, router).await.unwrap();
            });
            let result = consume(
                &reqwest::Client::builder().no_proxy().build().unwrap(),
                &format!("http://{address}/credits"),
                "test-token",
                "test-account",
                "card",
                "stable-request",
            )
            .await;
            assert_eq!(result.is_ok(), succeeds, "{result:?}");
            let calls = calls.lock().unwrap();
            assert_eq!(calls.len(), expected_calls);
            if let Some(body) = calls.first() {
                assert_eq!(
                    body,
                    &serde_json::json!({"credit_id":"card","redeem_request_id":"stable-request"})
                );
            }
            task.abort();
        }
    }
}
