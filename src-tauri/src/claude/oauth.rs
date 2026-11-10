use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::utils::now_unix_seconds;

pub(super) const CLIENT_ID: &str = "9d1c250a-e61b-44d9-88ed-5944d1962f5e";
const AUTHORIZE_URL: &str = "https://claude.com/cai/oauth/authorize";
const TOKEN_URL: &str = "https://platform.claude.com/v1/oauth/token";
pub(super) const REDIRECT_URI: &str = "https://platform.claude.com/oauth/code/callback";
const SCOPE: &str = "org:create_api_key user:profile user:inference user:sessions:claude_code user:mcp_servers user:file_upload";

pub struct PendingLogin {
    pub state: String,
    pub verifier: String,
    pub expires_at: i64,
}

pub fn prepare() -> (PendingLogin, String) {
    let state = uuid::Uuid::new_v4().simple().to_string();
    let verifier = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    let mut url = reqwest::Url::parse(AUTHORIZE_URL).unwrap();
    url.query_pairs_mut()
        .append_pair("code", "true")
        .append_pair("client_id", CLIENT_ID)
        .append_pair("response_type", "code")
        .append_pair("redirect_uri", REDIRECT_URI)
        .append_pair("scope", SCOPE)
        .append_pair(
            "code_challenge",
            &URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes())),
        )
        .append_pair("code_challenge_method", "S256")
        .append_pair("state", &state);
    (
        PendingLogin {
            state,
            verifier,
            expires_at: now_unix_seconds() + 1800,
        },
        url.to_string(),
    )
}

fn exchange_body(pending: &PendingLogin, input: &str) -> Result<Value, String> {
    if now_unix_seconds() > pending.expires_at {
        return Err("Claude 授权已过期，请重新生成链接".into());
    }
    let input = input.trim();
    let (code, state) = if input.starts_with("https://") {
        let url = reqwest::Url::parse(input).map_err(|e| e.to_string())?;
        let params: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
        (
            params.get("code").cloned().unwrap_or_default(),
            params.get("state").cloned(),
        )
    } else {
        let (code, state) = input
            .split_once('#')
            .map_or((input, None), |(c, s)| (c, Some(s.to_string())));
        (code.to_string(), state)
    };
    if code.is_empty() {
        return Err("请输入 Claude 授权码".into());
    }
    if state.as_ref().is_some_and(|s| s != &pending.state) {
        return Err("Claude 授权 state 不匹配，请使用本次授权码".into());
    }
    let mut body = json!({"grant_type":"authorization_code", "client_id":CLIENT_ID,
        "redirect_uri":REDIRECT_URI, "code":code, "code_verifier":pending.verifier});
    if let Some(state) = state {
        body["state"] = json!(state);
    }
    Ok(body)
}

async fn token_request(client: &reqwest::Client, body: Value) -> Result<Value, String> {
    let response = client
        .post(TOKEN_URL)
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        let status = response.status().as_u16();
        let error: Value = response.json().await.unwrap_or_default();
        let invalid = error["error"].as_str() == Some("invalid_grant");
        return Err(if invalid {
            "Claude 登录已失效（invalid_grant），请重新授权".into()
        } else {
            format!("Claude 令牌请求失败（HTTP {status}）")
        });
    }
    response
        .json()
        .await
        .map_err(|e| format!("Claude 令牌响应无法解析: {e}"))
}

pub async fn complete(
    client: &reqwest::Client,
    pending: &PendingLogin,
    input: &str,
) -> Result<Value, String> {
    token_request(client, exchange_body(pending, input)?).await
}

pub async fn refresh(client: &reqwest::Client, refresh_token: &str) -> Result<Value, String> {
    token_request(
        client,
        json!({"grant_type":"refresh_token", "client_id":CLIENT_ID, "refresh_token":refresh_token}),
    )
    .await
}

pub fn credentials(response: &Value, previous: Option<&Value>) -> Result<Value, String> {
    let access = response
        .get("access_token")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or("Claude 响应缺少 access_token")?;
    let refresh = response
        .get("refresh_token")
        .cloned()
        .or_else(|| previous.and_then(|v| v.get("refreshToken").cloned()));
    let expires_at = response
        .get("expires_at")
        .and_then(Value::as_i64)
        .map(|v| v * 1000)
        .or_else(|| {
            response
                .get("expires_in")
                .and_then(Value::as_i64)
                .map(|v| (now_unix_seconds() + v) * 1000)
        });
    Ok(json!({"accessToken":access, "refreshToken":refresh,
        "expiresAt":expires_at,
        "scopes":response.get("scope").and_then(Value::as_str).unwrap_or(SCOPE).split_whitespace().collect::<Vec<_>>(),
        "subscriptionType":previous.and_then(|v| v.get("subscriptionType")),
        "accountUuid":response.pointer("/account/uuid").or_else(|| previous.and_then(|v| v.get("accountUuid"))),
        "orgUuid":response.pointer("/organization/uuid").or_else(|| previous.and_then(|v| v.get("orgUuid"))),
        "rateLimitTier":previous.and_then(|v| v.get("rateLimitTier"))}))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn claude_pkce_and_callback_contract() {
        let (pending, url) = prepare();
        let parsed = reqwest::Url::parse(&url).unwrap();
        assert_eq!(parsed.host_str(), Some("claude.com"));
        let query: std::collections::HashMap<_, _> = parsed.query_pairs().into_owned().collect();
        assert_eq!(query["redirect_uri"], REDIRECT_URI);
        assert_eq!(
            query["code_challenge"],
            URL_SAFE_NO_PAD.encode(Sha256::digest(pending.verifier.as_bytes()))
        );
        assert!(exchange_body(&pending, "code#wrong-state").is_err());
        assert_eq!(
            exchange_body(&pending, &format!("code#{}", pending.state)).unwrap()["code"],
            "code"
        );
        assert_eq!(
            exchange_body(&pending, "plain-code").unwrap()["code"],
            "plain-code"
        );
    }
    #[test]
    fn refresh_keeps_existing_refresh_token() {
        let c = credentials(
            &json!({"access_token":"new", "expires_in":3600}),
            Some(&json!({"refreshToken":"keep"})),
        )
        .unwrap();
        assert_eq!(c["refreshToken"], "keep");
        assert!(credentials(&json!({"id_token":"codex"}), None).is_err());
    }
}
