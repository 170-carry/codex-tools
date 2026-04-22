use std::fs;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use reqwest::StatusCode;
use serde_json::Value;
use toml_edit::value;
use toml_edit::DocumentMut;
use uuid::Uuid;

use crate::app_paths;
use crate::auth;
use crate::models::AccountSourceKind;
use crate::models::StoredAccount;
use crate::utils::set_private_permissions;

const PROFILE_DIR_NAME: &str = "profiles";
const PROFILE_AUTH_FILE_NAME: &str = "auth.json";
const PROFILE_CONFIG_FILE_NAME: &str = "config.toml";
const PROFILE_INCOMPLETE_MESSAGE: &str = "配置不完整";
const RELAY_INCOMPLETE_MESSAGE: &str = "API 条目资料不完整";
const VALIDATE_TIMEOUT_SECS: u64 = 18;

pub(crate) fn profile_dir_from_store_path(store_path: &Path, id: &str) -> PathBuf {
    store_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(PROFILE_DIR_NAME)
        .join(id)
}

pub(crate) fn profile_auth_path_from_store_path(store_path: &Path, id: &str) -> PathBuf {
    profile_dir_from_store_path(store_path, id).join(PROFILE_AUTH_FILE_NAME)
}

pub(crate) fn profile_config_path_from_store_path(store_path: &Path, id: &str) -> PathBuf {
    profile_dir_from_store_path(store_path, id).join(PROFILE_CONFIG_FILE_NAME)
}

pub(crate) fn ensure_profile_metadata(store_path: &Path, account: &mut StoredAccount) -> bool {
    let mut changed = false;
    let auth_path = profile_auth_path_from_store_path(store_path, &account.id);
    let config_path = profile_config_path_from_store_path(store_path, &account.id);
    let auth_path_string = auth_path.to_string_lossy().to_string();
    let config_path_string = config_path.to_string_lossy().to_string();

    if account.profile_auth_path.as_deref() != Some(auth_path_string.as_str()) {
        account.profile_auth_path = Some(auth_path_string);
        changed = true;
    }
    if account.profile_config_path.as_deref() != Some(config_path_string.as_str()) {
        account.profile_config_path = Some(config_path_string);
        changed = true;
    }

    let auth_ready = auth_path.is_file();
    let config_ready = config_path.is_file();
    if account.profile_auth_ready != auth_ready {
        account.profile_auth_ready = auth_ready;
        changed = true;
    }
    if account.profile_config_ready != config_ready {
        account.profile_config_ready = config_ready;
        changed = true;
    }

    let integrity_error = compute_profile_integrity_error(account, auth_ready, config_ready);
    if account.profile_integrity_error != integrity_error {
        account.profile_integrity_error = integrity_error;
        changed = true;
    }

    changed
}

pub(crate) fn sync_account_profile_in_store_path(
    store_path: &Path,
    account: &mut StoredAccount,
) -> Result<(), String> {
    let auth_path = profile_auth_path_from_store_path(store_path, &account.id);
    let config_path = profile_config_path_from_store_path(store_path, &account.id);
    let profile_dir = auth_path
        .parent()
        .ok_or_else(|| format!("无法解析账号 profile 目录 {}", auth_path.display()))?;
    fs::create_dir_all(profile_dir)
        .map_err(|error| format!("创建账号 profile 目录失败 {}: {error}", profile_dir.display()))?;

    let config_template =
        read_optional_text(&config_path)?.or(read_current_codex_config_optional()?);
    let config_text = match account.source_kind {
        AccountSourceKind::Chatgpt => build_chatgpt_profile_config(config_template.as_deref()),
        AccountSourceKind::Relay => build_relay_profile_config(
            config_template.as_deref(),
            account
                .api_base_url
                .as_deref()
                .ok_or_else(|| RELAY_INCOMPLETE_MESSAGE.to_string())?,
            account
                .model_name
                .as_deref()
                .ok_or_else(|| RELAY_INCOMPLETE_MESSAGE.to_string())?,
        ),
    };

    let auth_json = match account.source_kind {
        AccountSourceKind::Chatgpt => account.auth_json.clone(),
        AccountSourceKind::Relay => build_api_auth_json(
            account
                .api_key
                .as_deref()
                .ok_or_else(|| RELAY_INCOMPLETE_MESSAGE.to_string())?,
        ),
    };

    let serialized_auth = serde_json::to_string_pretty(&auth_json)
        .map_err(|error| format!("序列化账号 profile auth.json 失败: {error}"))?;
    write_file_atomically(&auth_path, serialized_auth.as_bytes())?;
    write_file_atomically(&config_path, config_text.as_bytes())?;

    account.profile_auth_path = Some(auth_path.to_string_lossy().to_string());
    account.profile_config_path = Some(config_path.to_string_lossy().to_string());
    account.profile_auth_ready = true;
    account.profile_config_ready = true;
    account.profile_integrity_error = None;
    Ok(())
}

pub(crate) fn apply_account_profile(account: &StoredAccount) -> Result<(), String> {
    let auth_path = account
        .profile_auth_path
        .as_deref()
        .map(PathBuf::from)
        .ok_or_else(|| PROFILE_INCOMPLETE_MESSAGE.to_string())?;
    let config_path = account
        .profile_config_path
        .as_deref()
        .map(PathBuf::from)
        .ok_or_else(|| PROFILE_INCOMPLETE_MESSAGE.to_string())?;

    if !auth_path.is_file() || !config_path.is_file() {
        return Err(account
            .profile_integrity_error
            .clone()
            .unwrap_or_else(|| PROFILE_INCOMPLETE_MESSAGE.to_string()));
    }

    let auth_contents = fs::read_to_string(&auth_path)
        .map_err(|error| format!("读取账号 profile auth.json 失败 {}: {error}", auth_path.display()))?;
    let auth_json: Value = serde_json::from_str(&auth_contents)
        .map_err(|error| format!("账号 profile auth.json 不是合法 JSON {}: {error}", auth_path.display()))?;
    auth::write_active_codex_auth(&auth_json)?;

    let config_contents = fs::read_to_string(&config_path)
        .map_err(|error| format!("读取账号 profile config.toml 失败 {}: {error}", config_path.display()))?;
    let active_config_path = current_codex_config_path()?;
    let parent = active_config_path
        .parent()
        .ok_or_else(|| format!("无法解析 Codex 配置目录 {}", active_config_path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("创建 Codex 配置目录失败 {}: {error}", parent.display()))?;
    write_file_atomically(&active_config_path, config_contents.as_bytes())?;
    Ok(())
}

pub(crate) fn build_api_auth_json(api_key: &str) -> Value {
    serde_json::json!({
        "OPENAI_API_KEY": api_key,
        "auth_mode": "apikey"
    })
}

pub(crate) fn relay_account_key(id: &str) -> String {
    format!("relay|{id}")
}

pub(crate) fn relay_account_id(id: &str) -> String {
    format!("relay:{id}")
}

pub(crate) fn normalize_relay_label(label: &str) -> Result<String, String> {
    let trimmed = label.trim();
    if trimmed.is_empty() {
        return Err("请输入 API 名称。".to_string());
    }
    Ok(trimmed.to_string())
}

pub(crate) fn normalize_relay_model_name(model_name: &str) -> Result<String, String> {
    let trimmed = model_name.trim();
    if trimmed.is_empty() {
        return Err("请输入模型名称。".to_string());
    }
    Ok(trimmed.to_string())
}

pub(crate) fn normalize_relay_api_key(api_key: &str) -> Result<String, String> {
    let trimmed = api_key.trim();
    if trimmed.is_empty() {
        return Err("请输入 API Key。".to_string());
    }
    if !trimmed.starts_with("sk-") {
        return Err("仅支持 OpenAI 格式 API Key，例如 sk-...".to_string());
    }
    Ok(trimmed.to_string())
}

pub(crate) fn normalize_relay_base_url(base_url: &str) -> Result<String, String> {
    let trimmed = base_url.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return Err("请输入 Base URL。".to_string());
    }
    if !(trimmed.starts_with("https://") || trimmed.starts_with("http://")) {
        return Err("Base URL 仅支持 http/https 地址。".to_string());
    }
    Ok(trimmed.to_string())
}

pub(crate) async fn validate_relay_target(
    base_url: &str,
    api_key: &str,
    model_name: &str,
) -> Result<Option<String>, String> {
    let endpoint = format!("{}/responses", base_url.trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(VALIDATE_TIMEOUT_SECS))
        .build()
        .map_err(|error| format!("创建 API 检测客户端失败: {error}"))?;

    let payload = serde_json::json!({
        "model": model_name,
        "input": "ping",
        "max_output_tokens": 1
    });

    let response = client
        .post(&endpoint)
        .bearer_auth(api_key)
        .json(&payload)
        .send()
        .await
        .map_err(|error| format!("检测 API 失败 {endpoint}: {error}"))?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(match status {
            StatusCode::UNAUTHORIZED => "API Key 无效或已失效。".to_string(),
            StatusCode::NOT_FOUND => "Base URL 不支持 /responses 接口，请确认填写到 /v1 为止。".to_string(),
            StatusCode::BAD_REQUEST => {
                if body.to_ascii_lowercase().contains("model") {
                    format!("模型名称不可用: {}", truncate_message(&body))
                } else {
                    format!("接口请求被拒绝: {}", truncate_message(&body))
                }
            }
            _ => format!("检测接口返回 {status}: {}", truncate_message(&body)),
        });
    }

    let balance = fetch_relay_balance_best_effort(&client, base_url, api_key).await;
    Ok(balance)
}

async fn fetch_relay_balance_best_effort(
    client: &reqwest::Client,
    base_url: &str,
    api_key: &str,
) -> Option<String> {
    let mut candidates = Vec::new();
    let normalized = base_url.trim_end_matches('/');
    candidates.push(format!("{normalized}/dashboard/billing/credit_grants"));
    if let Some(stripped) = normalized.strip_suffix("/v1") {
        candidates.push(format!("{stripped}/dashboard/billing/credit_grants"));
    }

    for endpoint in candidates {
        let Ok(response) = client.get(&endpoint).bearer_auth(api_key).send().await else {
            continue;
        };
        if !response.status().is_success() {
            continue;
        }
        let Ok(payload) = response.json::<Value>().await else {
            continue;
        };
        if let Some(value) = payload
            .get("total_available")
            .and_then(Value::as_f64)
            .map(|number| format!("${number:.2}"))
        {
            return Some(value);
        }
        if let Some(value) = payload
            .get("balance")
            .and_then(Value::as_str)
            .map(ToString::to_string)
        {
            return Some(value);
        }
    }

    None
}

fn compute_profile_integrity_error(
    account: &StoredAccount,
    auth_ready: bool,
    config_ready: bool,
) -> Option<String> {
    if matches!(account.source_kind, AccountSourceKind::Relay)
        && (account.api_base_url.as_deref().is_none()
            || account.api_key.as_deref().is_none()
            || account.model_name.as_deref().is_none())
    {
        return Some(RELAY_INCOMPLETE_MESSAGE.to_string());
    }

    if auth_ready && config_ready {
        None
    } else {
        Some(PROFILE_INCOMPLETE_MESSAGE.to_string())
    }
}

fn build_chatgpt_profile_config(current_config: Option<&str>) -> String {
    let mut document = parse_config_or_default(current_config);
    let had_base_url = document.get("openai_base_url").is_some();
    document.remove("openai_base_url");
    if had_base_url {
        document.remove("model");
    }
    document.to_string()
}

fn build_relay_profile_config(
    current_config: Option<&str>,
    base_url: &str,
    model_name: &str,
) -> String {
    let mut document = parse_config_or_default(current_config);
    document["openai_base_url"] = value(base_url);
    document["model"] = value(model_name);
    document.to_string()
}

fn parse_config_or_default(current_config: Option<&str>) -> DocumentMut {
    current_config
        .and_then(|raw| raw.parse::<DocumentMut>().ok())
        .unwrap_or_default()
}

fn read_current_codex_config_optional() -> Result<Option<String>, String> {
    let path = current_codex_config_path()?;
    read_optional_text(&path)
}

fn current_codex_config_path() -> Result<PathBuf, String> {
    app_paths::codex_config_path()
}

fn truncate_message(message: &str) -> String {
    let trimmed = message.trim();
    if trimmed.chars().count() <= 160 {
        trimmed.to_string()
    } else {
        let truncated = trimmed.chars().take(157).collect::<String>();
        format!("{truncated}...")
    }
}

fn read_optional_text(path: &Path) -> Result<Option<String>, String> {
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(path)
        .map_err(|error| format!("读取文件失败 {}: {error}", path.display()))?;
    Ok(Some(raw))
}

fn write_file_atomically(path: &Path, contents: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("无法解析目标目录 {}", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("创建目标目录失败 {}: {error}", parent.display()))?;

    let temp_path = parent.join(format!(
        ".{}.tmp-{}",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("profile"),
        Uuid::new_v4()
    ));

    let write_result = (|| -> Result<(), String> {
        let mut temp_file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp_path)
            .map_err(|error| format!("创建临时文件失败 {}: {error}", temp_path.display()))?;
        temp_file
            .write_all(contents)
            .map_err(|error| format!("写入临时文件失败 {}: {error}", temp_path.display()))?;
        temp_file
            .sync_all()
            .map_err(|error| format!("刷新临时文件失败 {}: {error}", temp_path.display()))?;
        drop(temp_file);
        set_private_permissions(&temp_path);

        #[cfg(target_family = "unix")]
        {
            fs::rename(&temp_path, path).map_err(|error| {
                format!(
                    "替换目标文件失败 {} -> {}: {error}",
                    temp_path.display(),
                    path.display()
                )
            })?;

            let parent_dir = fs::File::open(parent)
                .map_err(|error| format!("打开目标目录失败 {}: {error}", parent.display()))?;
            parent_dir
                .sync_all()
                .map_err(|error| format!("刷新目标目录失败 {}: {error}", parent.display()))?;
        }

        #[cfg(not(target_family = "unix"))]
        {
            if path.exists() {
                fs::remove_file(path)
                    .map_err(|error| format!("移除旧文件失败 {}: {error}", path.display()))?;
            }
            fs::rename(&temp_path, path).map_err(|error| {
                format!(
                    "替换目标文件失败 {} -> {}: {error}",
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
