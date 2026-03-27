use base64::engine::general_purpose::URL_SAFE;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use serde_json::Map;
use serde_json::Value;
use sha2::Digest;
use sha2::Sha256;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::models::ExtractedAuth;
use crate::models::PreparedOauthLogin;
use crate::utils::set_private_permissions;
use crate::utils::truncate_for_error;

const DEFAULT_OAUTH_ISSUER: &str = "https://auth.openai.com";
const DEFAULT_OAUTH_CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
const DEFAULT_OAUTH_SCOPE: &str = "openid profile email offline_access";
const DEFAULT_OAUTH_ORIGINATOR: &str = "codex_vscode";
const DEFAULT_OAUTH_REDIRECT_PORT: u16 = 1455;
const DEFAULT_OAUTH_TIMEOUT_SECS: i64 = 300;
const NON_CHATGPT_AUTH_MODE_ERROR: &str =
    "当前账号不是 ChatGPT 登录模式，无法读取 Codex 5h/1week 用量。请先执行 codex login。";
const MISSING_CHATGPT_TOKEN_ERROR: &str = "当前 auth.json 未包含 ChatGPT 登录令牌。若该文件来自新版 Codex（尤其是 macOS），令牌可能保存在系统钥匙串/安全存储中，因此不能仅靠这个 auth.json 跨机导入。请在目标设备执行 codex login，或提供包含 access_token / id_token / refresh_token 的完整 auth.json。";

pub(crate) struct CodexOAuthTokens {
    pub(crate) access_token: String,
    pub(crate) refresh_token: String,
    pub(crate) account_id: Option<String>,
    pub(crate) expires_at_ms: Option<i64>,
}

#[derive(Debug, Clone)]
pub(crate) struct PendingOauthLogin {
    pub(crate) redirect_uri: String,
    pub(crate) state: String,
    pub(crate) code_verifier: String,
    pub(crate) expires_at: i64,
}

pub(crate) fn oauth_redirect_port() -> u16 {
    DEFAULT_OAUTH_REDIRECT_PORT
}

fn oauth_redirect_uri(port: u16) -> String {
    format!("http://localhost:{port}/auth/callback")
}

pub(crate) fn read_current_codex_auth() -> Result<Value, String> {
    let path = codex_auth_path()?;
    let raw = fs::read_to_string(&path)
        .map_err(|e| format!("读取当前 Codex 认证文件失败 {}: {e}", path.display()))?;
    serde_json::from_str(&raw).map_err(|e| format!("当前 Codex 认证文件不是合法 JSON: {e}"))
}

pub(crate) fn read_current_codex_auth_optional() -> Result<Option<Value>, String> {
    let path = codex_auth_path()?;
    if !path.exists() {
        return Ok(None);
    }

    let raw = fs::read_to_string(&path)
        .map_err(|e| format!("读取当前 Codex 认证文件失败 {}: {e}", path.display()))?;
    let value =
        serde_json::from_str(&raw).map_err(|e| format!("当前 Codex 认证文件不是合法 JSON: {e}"))?;
    Ok(Some(value))
}

pub(crate) fn write_active_codex_auth(auth_json: &Value) -> Result<(), String> {
    let path = codex_auth_path()?;
    let parent = path
        .parent()
        .ok_or_else(|| format!("无法解析 auth 目录 {}", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|e| format!("创建 auth 目录失败 {}: {e}", parent.display()))?;

    let normalized = normalize_auth_json_for_codex(auth_json.clone());
    let serialized = serde_json::to_string_pretty(&normalized)
        .map_err(|e| format!("序列化 auth.json 失败: {e}"))?;
    write_auth_file_atomically(&path, serialized.as_bytes())
}

pub(crate) fn prepare_oauth_login(
    redirect_port: u16,
) -> Result<(PendingOauthLogin, PreparedOauthLogin), String> {
    let state = uuid::Uuid::new_v4().simple().to_string();
    let code_verifier = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    let code_challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(code_verifier.as_bytes()));
    let redirect_uri = oauth_redirect_uri(redirect_port);
    let expires_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("读取系统时间失败: {error}"))?
        .as_secs() as i64
        + DEFAULT_OAUTH_TIMEOUT_SECS;

    let mut auth_url = reqwest::Url::parse(&format!("{DEFAULT_OAUTH_ISSUER}/oauth/authorize"))
        .map_err(|error| format!("生成授权链接失败: {error}"))?;
    auth_url
        .query_pairs_mut()
        .append_pair("response_type", "code")
        .append_pair("client_id", DEFAULT_OAUTH_CLIENT_ID)
        .append_pair("redirect_uri", &redirect_uri)
        .append_pair("scope", DEFAULT_OAUTH_SCOPE)
        .append_pair("state", &state)
        .append_pair("code_challenge", &code_challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("id_token_add_organizations", "true")
        .append_pair("codex_cli_simplified_flow", "true")
        .append_pair("originator", DEFAULT_OAUTH_ORIGINATOR);

    let auth_url = auth_url.to_string();
    let pending = PendingOauthLogin {
        redirect_uri: redirect_uri.clone(),
        state,
        code_verifier,
        expires_at,
    };
    let prepared = PreparedOauthLogin {
        auth_url,
        redirect_uri,
    };
    Ok((pending, prepared))
}

pub(crate) async fn complete_oauth_callback_login(
    pending: &PendingOauthLogin,
    callback_url: &str,
) -> Result<Value, String> {
    let callback_url = callback_url.trim();
    if callback_url.is_empty() {
        return Err("请粘贴回调链接".to_string());
    }

    let parsed_url = parse_oauth_callback_url(callback_url)?;
    let params: std::collections::HashMap<String, String> = parsed_url
        .query_pairs()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect();

    if let Some(error) = params.get("error") {
        let description = params
            .get("error_description")
            .map(String::as_str)
            .unwrap_or(error.as_str());
        return Err(format!("授权失败: {description}"));
    }

    let Some(state) = params.get("state") else {
        return Err("回调链接缺少 state 参数".to_string());
    };
    if state != &pending.state {
        return Err("回调链接 state 不匹配，请重新生成授权链接".to_string());
    }

    let Some(code) = params.get("code") else {
        return Err("回调链接缺少 code 参数".to_string());
    };

    exchange_authorization_code(code, pending).await
}

pub(crate) fn normalize_imported_auth_json(auth_json: Value) -> Value {
    let Some(root) = auth_json.as_object() else {
        return auth_json;
    };

    if root.get("tokens").and_then(Value::as_object).is_some() {
        return normalize_auth_json_for_codex(auth_json);
    }

    let Some(access_token) = root.get("access_token").and_then(Value::as_str) else {
        return auth_json;
    };
    let Some(id_token) = root.get("id_token").and_then(Value::as_str) else {
        return auth_json;
    };

    let mut tokens = Map::new();
    tokens.insert(
        "access_token".to_string(),
        Value::String(access_token.to_string()),
    );
    tokens.insert("id_token".to_string(), Value::String(id_token.to_string()));

    if let Some(refresh_token) = root.get("refresh_token").and_then(Value::as_str) {
        tokens.insert(
            "refresh_token".to_string(),
            Value::String(refresh_token.to_string()),
        );
    }
    if let Some(account_id) = root.get("account_id").and_then(Value::as_str) {
        tokens.insert(
            "account_id".to_string(),
            Value::String(account_id.to_string()),
        );
    }

    let mut normalized = Map::new();
    normalized.insert(
        "auth_mode".to_string(),
        Value::String(
            root.get("auth_mode")
                .and_then(Value::as_str)
                .unwrap_or("chatgpt")
                .to_string(),
        ),
    );
    normalized.insert("tokens".to_string(), Value::Object(tokens));

    if let Some(last_refresh) = root.get("last_refresh") {
        normalized.insert("last_refresh".to_string(), last_refresh.clone());
    }

    normalize_auth_json_for_codex(Value::Object(normalized))
}

/// 解析当前 auth.json，提取账号标识和用量接口所需 token。
///
/// 注意：`auth_mode` 在某些版本可能缺失，因此优先按 `tokens` 字段判断是否可用。
pub(crate) fn extract_auth(auth_json: &Value) -> Result<ExtractedAuth, String> {
    let mode = auth_json
        .get("auth_mode")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();

    let tokens = auth_token_object(auth_json);
    let tokens = match tokens {
        Some(value) => value,
        None => {
            if !mode.is_empty() && mode != "chatgpt" && mode != "chatgpt_auth_tokens" {
                return Err(NON_CHATGPT_AUTH_MODE_ERROR.to_string());
            }
            return Err(MISSING_CHATGPT_TOKEN_ERROR.to_string());
        }
    };

    let access_token = tokens
        .get("access_token")
        .and_then(Value::as_str)
        .ok_or_else(|| "auth.json 缺少 access_token".to_string())?
        .to_string();

    let id_token = tokens
        .get("id_token")
        .and_then(Value::as_str)
        .ok_or_else(|| "auth.json 缺少 id_token".to_string())?;

    let mut account_id = tokens
        .get("account_id")
        .and_then(Value::as_str)
        .map(ToString::to_string);
    let mut email = None;
    let mut plan_type = None;
    let mut principal_id = None;

    if let Ok(claims) = decode_jwt_payload(id_token) {
        email = claims
            .get("email")
            .and_then(Value::as_str)
            .map(ToString::to_string);

        let auth_claim = claims
            .get("https://api.openai.com/auth")
            .and_then(Value::as_object);
        if account_id.is_none() {
            account_id = auth_claim
                .and_then(|value| value.get("chatgpt_account_id"))
                .and_then(Value::as_str)
                .map(ToString::to_string);
        }
        plan_type = auth_claim
            .and_then(|value| value.get("chatgpt_plan_type"))
            .and_then(Value::as_str)
            .map(ToString::to_string);
        principal_id = email
            .as_deref()
            .and_then(normalize_principal_key)
            .or_else(|| {
                auth_claim
                    .and_then(|value| {
                        value
                            .get("chatgpt_user_id")
                            .or_else(|| value.get("user_id"))
                    })
                    .and_then(Value::as_str)
                    .and_then(normalize_principal_key)
            })
            .or_else(|| {
                claims
                    .get("sub")
                    .and_then(Value::as_str)
                    .and_then(normalize_principal_key)
            });
    }

    let account_id =
        account_id.ok_or_else(|| "无法从 auth.json 识别 chatgpt_account_id".to_string())?;
    let principal_id = principal_id.unwrap_or_else(|| account_id.clone());

    Ok(ExtractedAuth {
        principal_id,
        account_id,
        access_token,
        email,
        plan_type,
    })
}

pub(crate) fn current_auth_account_key() -> Option<String> {
    read_current_codex_auth()
        .ok()
        .and_then(|auth_json| extract_auth(&auth_json).ok())
        .map(|auth| account_group_key(&auth.principal_id, &auth.account_id))
}

pub(crate) fn normalize_plan_type_key(plan_type: Option<&str>) -> String {
    let Some(value) = plan_type.map(str::trim).filter(|value| !value.is_empty()) else {
        return "unknown".to_string();
    };
    value.to_ascii_lowercase()
}

pub(crate) fn account_group_key(principal_id: &str, account_id: &str) -> String {
    format!("{}|{}", principal_id.trim(), account_id.trim())
}

pub(crate) fn account_variant_key(
    principal_id: &str,
    account_id: &str,
    plan_type: Option<&str>,
) -> String {
    format!(
        "{}|{}",
        account_group_key(principal_id, account_id),
        normalize_plan_type_key(plan_type)
    )
}

pub(crate) fn auth_variant_key(auth_json: &Value) -> Option<String> {
    let extracted = extract_auth(auth_json).ok()?;
    Some(account_variant_key(
        &extracted.principal_id,
        &extracted.account_id,
        extracted.plan_type.as_deref(),
    ))
}

pub(crate) fn current_auth_variant_key() -> Option<String> {
    read_current_codex_auth()
        .ok()
        .and_then(|auth_json| auth_variant_key(&auth_json))
}

fn normalize_principal_key(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.contains('@') {
        Some(trimmed.to_ascii_lowercase())
    } else {
        Some(trimmed.to_string())
    }
}

/// 为第三方客户端同步登录态时，提取可复用的 OpenAI OAuth token。
pub(crate) fn extract_codex_oauth_tokens(auth_json: &Value) -> Result<CodexOAuthTokens, String> {
    let tokens = auth_token_object(auth_json).ok_or_else(|| "auth.json 缺少 tokens".to_string())?;

    let access_token = tokens
        .get("access_token")
        .and_then(Value::as_str)
        .ok_or_else(|| "auth.json 缺少 access_token".to_string())?
        .to_string();
    let refresh_token = tokens
        .get("refresh_token")
        .and_then(Value::as_str)
        .ok_or_else(|| "auth.json 缺少 refresh_token".to_string())?
        .to_string();
    let account_id = tokens
        .get("account_id")
        .and_then(Value::as_str)
        .map(ToString::to_string);

    let expires_at_ms = tokens
        .get("id_token")
        .and_then(Value::as_str)
        .and_then(|id_token| decode_jwt_payload(id_token).ok())
        .and_then(|payload| payload.get("exp").and_then(Value::as_i64))
        .map(|value| value * 1000);

    Ok(CodexOAuthTokens {
        access_token,
        refresh_token,
        account_id,
        expires_at_ms,
    })
}

/// 使用 auth.json 内的 refresh_token 刷新 ChatGPT OAuth 令牌。
///
/// 返回更新后的 auth.json（仅内存对象，不会自动写盘）。
pub(crate) async fn refresh_chatgpt_auth_tokens(auth_json: &Value) -> Result<Value, String> {
    let tokens = auth_token_object(auth_json).ok_or_else(|| "auth.json 缺少 tokens".to_string())?;

    let refresh_token = tokens
        .get("refresh_token")
        .and_then(Value::as_str)
        .ok_or_else(|| "auth.json 缺少 refresh_token".to_string())?;
    let id_token = tokens
        .get("id_token")
        .and_then(Value::as_str)
        .ok_or_else(|| "auth.json 缺少 id_token".to_string())?;

    let claims = decode_jwt_payload(id_token)?;
    let issuer = claims
        .get("iss")
        .and_then(Value::as_str)
        .unwrap_or("https://auth.openai.com")
        .trim_end_matches('/')
        .to_string();
    let token_url = format!("{issuer}/oauth/token");

    let mut form_pairs: Vec<(&str, String)> = vec![
        ("grant_type", "refresh_token".to_string()),
        ("refresh_token", refresh_token.to_string()),
    ];
    if let Some(client_id) = extract_client_id_from_claims(&claims) {
        form_pairs.push(("client_id", client_id));
    }

    let client = reqwest::Client::builder()
        .user_agent("codex-tools/0.1")
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {e}"))?;

    let response = client
        .post(&token_url)
        .form(&form_pairs)
        .send()
        .await
        .map_err(|e| format!("刷新登录令牌失败 {token_url}: {e}"))?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(format!(
            "刷新登录令牌失败 {token_url} -> {status}: {}",
            truncate_for_error(&body, 140)
        ));
    }

    let refreshed: RefreshedTokenPayload = response
        .json()
        .await
        .map_err(|e| format!("解析刷新令牌响应失败: {e}"))?;

    let mut updated = auth_json.clone();
    let root = updated
        .as_object_mut()
        .ok_or_else(|| "auth.json 结构异常（根节点不是对象）".to_string())?;
    let tokens = root
        .get_mut("tokens")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| "auth.json 缺少 tokens".to_string())?;

    tokens.insert(
        "access_token".to_string(),
        Value::String(refreshed.access_token),
    );
    tokens.insert("id_token".to_string(), Value::String(refreshed.id_token));
    if let Some(refresh_token) = refreshed.refresh_token {
        tokens.insert("refresh_token".to_string(), Value::String(refresh_token));
    }

    update_last_refresh(&mut updated)?;
    Ok(normalize_auth_json_for_codex(updated))
}

fn parse_oauth_callback_url(callback_url: &str) -> Result<reqwest::Url, String> {
    reqwest::Url::parse(callback_url)
        .or_else(|_| reqwest::Url::parse(&format!("http://localhost{callback_url}")))
        .map_err(|error| format!("回调链接格式无效: {error}"))
}

async fn exchange_authorization_code(
    code: &str,
    pending: &PendingOauthLogin,
) -> Result<Value, String> {
    let client = reqwest::Client::builder()
        .user_agent("codex-tools/0.1")
        .build()
        .map_err(|error| format!("创建 HTTP 客户端失败: {error}"))?;

    let token_url = format!("{DEFAULT_OAUTH_ISSUER}/oauth/token");
    let response = client
        .post(&token_url)
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", pending.redirect_uri.as_str()),
            ("client_id", DEFAULT_OAUTH_CLIENT_ID),
            ("code_verifier", pending.code_verifier.as_str()),
        ])
        .send()
        .await
        .map_err(|error| format!("换取登录令牌失败 {token_url}: {error}"))?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(format!(
            "换取登录令牌失败 {token_url} -> {status}: {}",
            truncate_for_error(&body, 200)
        ));
    }

    let token_response: OAuthTokenResponse = response
        .json()
        .await
        .map_err(|error| format!("解析 OAuth 登录响应失败: {error}"))?;

    build_auth_json_from_oauth_tokens(token_response)
}

fn build_auth_json_from_oauth_tokens(token_response: OAuthTokenResponse) -> Result<Value, String> {
    let id_token_claims = decode_jwt_payload(&token_response.id_token)?;
    let account_id = id_token_claims
        .get("https://api.openai.com/auth")
        .and_then(Value::as_object)
        .and_then(|auth| auth.get("chatgpt_account_id"))
        .and_then(Value::as_str)
        .ok_or_else(|| "无法从 OAuth 登录结果识别 chatgpt_account_id".to_string())?;

    let last_refresh = current_rfc3339_timestamp()?;

    Ok(serde_json::json!({
        "OPENAI_API_KEY": Value::Null,
        "auth_mode": "chatgpt",
        "last_refresh": last_refresh,
        "tokens": {
            "access_token": token_response.access_token,
            "refresh_token": token_response.refresh_token,
            "id_token": token_response.id_token,
            "account_id": account_id
        }
    }))
}

fn codex_auth_path() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or_else(|| "无法读取 HOME 目录".to_string())?;
    Ok(home.join(".codex").join("auth.json"))
}

fn auth_token_object(auth_json: &Value) -> Option<&Map<String, Value>> {
    auth_json
        .get("tokens")
        .and_then(Value::as_object)
        .or_else(|| {
            let root = auth_json.as_object()?;
            if root.contains_key("access_token") && root.contains_key("id_token") {
                Some(root)
            } else {
                None
            }
        })
}

fn normalize_auth_json_for_codex(auth_json: Value) -> Value {
    let Some(root) = auth_json.as_object() else {
        return auth_json;
    };

    let mut normalized = root.clone();

    match normalized
        .get("last_refresh")
        .and_then(normalize_last_refresh_value)
    {
        Some(value) => {
            normalized.insert("last_refresh".to_string(), Value::String(value));
        }
        None => {
            normalized.remove("last_refresh");
        }
    }

    Value::Object(normalized)
}

fn normalize_last_refresh_value(value: &Value) -> Option<String> {
    match value {
        Value::String(raw) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                return None;
            }
            if OffsetDateTime::parse(trimmed, &Rfc3339).is_ok() {
                return Some(trimmed.to_string());
            }
            trimmed
                .parse::<i64>()
                .ok()
                .and_then(unix_timestamp_to_rfc3339)
        }
        Value::Number(number) => number.as_i64().and_then(unix_timestamp_to_rfc3339),
        _ => None,
    }
}

fn unix_timestamp_to_rfc3339(timestamp: i64) -> Option<String> {
    let datetime = if timestamp.abs() >= 1_000_000_000_000 {
        OffsetDateTime::from_unix_timestamp_nanos(i128::from(timestamp) * 1_000_000).ok()?
    } else {
        OffsetDateTime::from_unix_timestamp(timestamp).ok()?
    };
    datetime.format(&Rfc3339).ok()
}

fn current_rfc3339_timestamp() -> Result<String, String> {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|error| format!("生成 last_refresh 失败: {error}"))
}

fn update_last_refresh(auth_json: &mut Value) -> Result<(), String> {
    let timestamp = current_rfc3339_timestamp()?;
    let root = auth_json
        .as_object_mut()
        .ok_or_else(|| "auth.json 结构异常（根节点不是对象）".to_string())?;
    root.insert("last_refresh".to_string(), Value::String(timestamp));
    Ok(())
}

fn write_auth_file_atomically(path: &Path, contents: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("无法解析 auth 目录 {}", path.display()))?;
    let temp_path = parent.join(format!(
        ".{}.tmp-{}",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("auth.json"),
        uuid::Uuid::new_v4()
    ));

    let write_result = (|| -> Result<(), String> {
        let mut temp_file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp_path)
            .map_err(|e| format!("创建临时 auth.json 失败 {}: {e}", temp_path.display()))?;
        temp_file
            .write_all(contents)
            .map_err(|e| format!("写入临时 auth.json 失败 {}: {e}", temp_path.display()))?;
        temp_file
            .sync_all()
            .map_err(|e| format!("刷新临时 auth.json 失败 {}: {e}", temp_path.display()))?;
        drop(temp_file);
        set_private_permissions(&temp_path);

        #[cfg(target_family = "unix")]
        {
            fs::rename(&temp_path, path).map_err(|e| {
                format!(
                    "替换 auth.json 失败 {} -> {}: {e}",
                    temp_path.display(),
                    path.display()
                )
            })?;

            let parent_dir = fs::File::open(parent)
                .map_err(|e| format!("打开 auth 目录失败 {}: {e}", parent.display()))?;
            parent_dir
                .sync_all()
                .map_err(|e| format!("刷新 auth 目录失败 {}: {e}", parent.display()))?;
        }

        #[cfg(not(target_family = "unix"))]
        {
            if path.exists() {
                fs::remove_file(path)
                    .map_err(|e| format!("移除旧 auth.json 失败 {}: {e}", path.display()))?;
            }
            fs::rename(&temp_path, path).map_err(|e| {
                format!(
                    "替换 auth.json 失败 {} -> {}: {e}",
                    temp_path.display(),
                    path.display()
                )
            })?;
        }

        set_private_permissions(path);
        Ok(())
    })();

    if write_result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }

    write_result
}

fn decode_jwt_payload(token: &str) -> Result<Value, String> {
    let payload = token
        .split('.')
        .nth(1)
        .ok_or_else(|| "id_token 格式无效".to_string())?;

    let decoded = URL_SAFE_NO_PAD
        .decode(payload)
        .or_else(|_| {
            let remainder = payload.len() % 4;
            let padded = if remainder == 0 {
                payload.to_string()
            } else {
                format!("{payload}{}", "=".repeat(4 - remainder))
            };
            URL_SAFE.decode(padded)
        })
        .map_err(|e| format!("解码 id_token 失败: {e}"))?;

    serde_json::from_slice(&decoded).map_err(|e| format!("解析 id_token payload 失败: {e}"))
}

#[derive(Debug, serde::Deserialize)]
struct RefreshedTokenPayload {
    access_token: String,
    id_token: String,
    refresh_token: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct OAuthTokenResponse {
    access_token: String,
    refresh_token: String,
    id_token: String,
}

fn extract_client_id_from_claims(claims: &Value) -> Option<String> {
    let aud = claims.get("aud")?;
    match aud {
        Value::String(value) => {
            if value.is_empty() {
                None
            } else {
                Some(value.to_string())
            }
        }
        Value::Array(items) => items.iter().find_map(|item| {
            item.as_str().and_then(|value| {
                if value.is_empty() {
                    None
                } else {
                    Some(value.to_string())
                }
            })
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::extract_auth;
    use super::normalize_auth_json_for_codex;
    use super::oauth_redirect_port;
    use super::prepare_oauth_login;
    use super::MISSING_CHATGPT_TOKEN_ERROR;
    use super::NON_CHATGPT_AUTH_MODE_ERROR;
    use serde_json::json;

    #[test]
    fn normalizes_legacy_unix_last_refresh_string() {
        let normalized = normalize_auth_json_for_codex(json!({
            "auth_mode": "chatgpt",
            "last_refresh": "1711111111",
            "tokens": {
                "access_token": "a",
                "refresh_token": "b",
                "id_token": "c"
            }
        }));

        let last_refresh = normalized
            .get("last_refresh")
            .and_then(serde_json::Value::as_str)
            .expect("last_refresh should be preserved");
        assert!(last_refresh.contains('T'));
        assert!(last_refresh.ends_with('Z'));
        assert_ne!(last_refresh, "1711111111");
    }

    #[test]
    fn removes_unparseable_last_refresh() {
        let normalized = normalize_auth_json_for_codex(json!({
            "auth_mode": "chatgpt",
            "last_refresh": "not-a-timestamp",
            "tokens": {
                "access_token": "a",
                "refresh_token": "b",
                "id_token": "c"
            }
        }));

        assert!(normalized.get("last_refresh").is_none());
    }

    #[test]
    fn keeps_valid_rfc3339_last_refresh() {
        let normalized = normalize_auth_json_for_codex(json!({
            "auth_mode": "chatgpt",
            "last_refresh": "2026-03-16T03:20:39.082325Z",
            "tokens": {
                "access_token": "a",
                "refresh_token": "b",
                "id_token": "c"
            }
        }));

        assert_eq!(
            normalized.get("last_refresh"),
            Some(&json!("2026-03-16T03:20:39.082325Z"))
        );
    }

    #[test]
    fn extract_auth_reports_portable_hint_when_chatgpt_tokens_are_missing() {
        let error = extract_auth(&json!({
            "auth_mode": "chatgpt",
            "last_refresh": "2026-03-16T03:20:39.082325Z"
        }))
        .expect_err("chatgpt auth without tokens should fail");

        assert_eq!(error, MISSING_CHATGPT_TOKEN_ERROR);
    }

    #[test]
    fn extract_auth_keeps_non_chatgpt_mode_error_when_tokens_are_missing() {
        let error = extract_auth(&json!({
            "auth_mode": "apikey"
        }))
        .expect_err("non-chatgpt auth without tokens should fail");

        assert_eq!(error, NON_CHATGPT_AUTH_MODE_ERROR);
    }

    #[test]
    fn prepare_oauth_login_uses_requested_redirect_port() {
        let custom_port = oauth_redirect_port() + 17;
        let (pending, prepared) =
            prepare_oauth_login(custom_port).expect("oauth login prep should succeed");

        assert!(pending
            .redirect_uri
            .contains(&format!("localhost:{custom_port}/auth/callback")));
        assert_eq!(pending.redirect_uri, prepared.redirect_uri);
        assert!(prepared
            .auth_url
            .contains(&format!("redirect_uri=http%3A%2F%2Flocalhost%3A{custom_port}%2Fauth%2Fcallback")));
    }
}
