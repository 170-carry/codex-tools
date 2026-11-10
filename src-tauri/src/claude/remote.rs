use super::{desktop, oauth, store};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

pub async fn decode(response: reqwest::Response) -> Result<Value, String> {
    let status = response.status();
    if !status.is_success() {
        return Err(format!("Claude 请求失败（HTTP {}）", status.as_u16()));
    }
    response
        .json()
        .await
        .map_err(|_| "Claude 响应格式错误".into())
}

pub async fn get(client: &reqwest::Client, token: &str, path: &str) -> Result<Value, String> {
    decode(
        client
            .get(format!("https://api.anthropic.com/api/oauth/{path}"))
            .bearer_auth(token)
            .header("anthropic-beta", "oauth-2025-04-20")
            .send()
            .await
            .map_err(|_| "连接 Claude 失败，请检查网络".to_string())?,
    )
    .await
}

pub async fn authorize_desktop(
    client: &reqwest::Client,
    snapshot: &desktop::Snapshot,
) -> Result<Value, String> {
    let cookies = snapshot.cookies.clone();
    let cookie = tauri::async_runtime::spawn_blocking(move || desktop::session_header(&cookies))
        .await
        .map_err(|e| e.to_string())??;
    let (pending, _) = oauth::prepare();
    let body = json!({
        "response_type":"code", "client_id":oauth::CLIENT_ID,
        "organization_uuid":snapshot.org_uuid, "redirect_uri":oauth::REDIRECT_URI,
        "scope":"user:profile user:inference user:sessions:claude_code user:mcp_servers user:file_upload",
        "state":pending.state, "code_challenge_method":"S256",
        "code_challenge":URL_SAFE_NO_PAD.encode(Sha256::digest(pending.verifier.as_bytes()))
    });
    let response = decode(
        client
            .post(format!(
                "https://claude.ai/v1/oauth/{}/authorize",
                snapshot.org_uuid
            ))
            .header("Cookie", cookie)
            .header("Origin", "https://claude.ai")
            .header("Referer", "https://claude.ai/new")
            .header("Accept", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|_| "连接 Claude 授权服务失败".to_string())?,
    )
    .await?;
    let redirect = response["redirect_uri"]
        .as_str()
        .ok_or("Claude 未返回授权码，请重新授权")?;
    let response = oauth::complete(client, &pending, redirect).await?;
    if response
        .pointer("/account/uuid")
        .and_then(Value::as_str)
        .is_some_and(|id| id != snapshot.account_uuid)
        || response
            .pointer("/organization/uuid")
            .and_then(Value::as_str)
            .is_some_and(|id| id != snapshot.org_uuid)
    {
        return Err("Claude 授权身份与桌面登录不一致".into());
    }
    Ok(response)
}

pub fn apply_profile(account: &mut store::Account, profile: &Value) -> Result<(), String> {
    let id = profile.pointer("/account/uuid").and_then(Value::as_str);
    let org = profile
        .pointer("/organization/uuid")
        .and_then(Value::as_str);
    if let Some(desktop) = &account.desktop {
        if id != Some(desktop.account_uuid.as_str()) || org != Some(desktop.org_uuid.as_str()) {
            return Err("Claude 授权身份与桌面登录不一致".into());
        }
    }
    if let Some(email) = profile
        .pointer("/account/email")
        .or_else(|| profile.pointer("/account/email_address"))
        .and_then(Value::as_str)
    {
        if account.label == "Claude" || account.label.starts_with("Claude Desktop ") {
            account.label = email.to_string();
        }
        account.email = Some(email.to_string());
    }
    account.credentials["accountUuid"] = json!(id);
    account.credentials["orgUuid"] = json!(org);
    if let Some(plan) = profile.pointer("/organization/organization_type") {
        account.credentials["subscriptionType"] = plan.clone();
    }
    if let Some(tier) = profile.pointer("/organization/rate_limit_tier") {
        account.credentials["rateLimitTier"] = tier.clone();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn profile_sets_identity_and_plan_without_overwriting_custom_label() {
        let mut account = store::Account {
            label: "Work".into(),
            credentials: json!({}),
            ..Default::default()
        };
        apply_profile(&mut account, &json!({"account":{"uuid":"a","email":"a@example.com"},"organization":{"uuid":"o","organization_type":"claude_max","rate_limit_tier":"default_claude_max_5x"}})).unwrap();
        assert_eq!(account.email.as_deref(), Some("a@example.com"));
        assert_eq!(account.label, "Work");
        assert_eq!(account.credentials["subscriptionType"], "claude_max");
        assert_eq!(account.credentials["orgUuid"], "o");
    }
    #[test]
    fn desktop_rejects_another_account_or_organization() {
        let mut account = store::Account {
            credentials: json!({}),
            ..Default::default()
        };
        account.desktop = Some(serde_json::from_value(json!({"accountUuid":"a", "orgUuid":"o", "authConfig":{}, "cookies":{"columns":[],"rows":[]}})).unwrap());
        for profile in [
            json!({"account":{"uuid":"other"},"organization":{"uuid":"o"}}),
            json!({"account":{"uuid":"a"},"organization":{"uuid":"other"}}),
        ] {
            assert!(apply_profile(&mut account, &profile).is_err());
            assert_eq!(account.credentials, json!({}));
        }
    }
}
