use std::fs;
use std::io::ErrorKind;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use reqwest::StatusCode;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use toml_edit::value;
use toml_edit::DocumentMut;
use uuid::Uuid;

use crate::app_paths;
use crate::auth;
use crate::models::AccountSourceKind;
use crate::models::StoredAccount;
use crate::utils;
use crate::utils::set_private_permissions;

const PROFILE_DIR_NAME: &str = "profiles";
const PROFILE_AUTH_FILE_NAME: &str = "auth.json";
const PROFILE_CONFIG_FILE_NAME: &str = "config.toml";
const PROFILE_INCOMPLETE_MESSAGE: &str = "配置不完整";
const RELAY_INCOMPLETE_MESSAGE: &str = "API 条目资料不完整";
const CODEX_PROXY_BACKUP_DIR_NAME: &str = "codex-tools-api-proxy-backup";
const CODEX_PROXY_BACKUP_METADATA_FILE_NAME: &str = "metadata.json";
const VALIDATE_TIMEOUT_SECS: u64 = 18;

#[derive(Debug, Clone)]
pub(crate) struct CodexProxyBindingState {
    pub(crate) bound: bool,
    pub(crate) restore_available: bool,
    pub(crate) current_base_url: Option<String>,
    pub(crate) config_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CodexProxyBindingBackupMetadata {
    config_existed: bool,
    auth_existed: bool,
    bound_base_url: String,
    bound_at: i64,
}

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
    fs::create_dir_all(profile_dir).map_err(|error| {
        format!(
            "创建账号 profile 目录失败 {}: {error}",
            profile_dir.display()
        )
    })?;

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

    let auth_contents = fs::read_to_string(&auth_path).map_err(|error| {
        format!(
            "读取账号 profile auth.json 失败 {}: {error}",
            auth_path.display()
        )
    })?;
    let auth_json: Value = serde_json::from_str(&auth_contents).map_err(|error| {
        format!(
            "账号 profile auth.json 不是合法 JSON {}: {error}",
            auth_path.display()
        )
    })?;
    auth::write_active_codex_auth(&auth_json)?;

    let config_contents = fs::read_to_string(&config_path).map_err(|error| {
        format!(
            "读取账号 profile config.toml 失败 {}: {error}",
            config_path.display()
        )
    })?;
    let active_config_path = current_codex_config_path()?;
    let parent = active_config_path
        .parent()
        .ok_or_else(|| format!("无法解析 Codex 配置目录 {}", active_config_path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("创建 Codex 配置目录失败 {}: {error}", parent.display()))?;
    write_file_atomically(&active_config_path, config_contents.as_bytes())?;
    Ok(())
}

pub(crate) fn codex_proxy_binding_state(
    expected_base_url: Option<&str>,
) -> Result<CodexProxyBindingState, String> {
    let config_path = current_codex_config_path()?;
    let current_base_url = read_optional_text(&config_path)?
        .as_deref()
        .and_then(codex_config_openai_base_url);
    let expected_base_url = expected_base_url.and_then(normalize_codex_proxy_base_url);

    Ok(CodexProxyBindingState {
        bound: expected_base_url
            .as_deref()
            .is_some_and(|expected| current_base_url.as_deref() == Some(expected)),
        restore_available: codex_proxy_backup_metadata_path()?.is_file(),
        current_base_url,
        config_path: Some(config_path.to_string_lossy().to_string()),
    })
}

pub(crate) fn bind_codex_to_api_proxy(
    base_url: &str,
    api_key: &str,
) -> Result<CodexProxyBindingState, String> {
    let base_url = normalize_codex_proxy_base_url(base_url)
        .ok_or_else(|| "本机反代 Base URL 为空。请先启动 API 反代。".to_string())?;
    let api_key = api_key.trim();
    if api_key.is_empty() {
        return Err("本机反代 API Key 为空。请先启动 API 反代。".to_string());
    }

    let config_path = current_codex_config_path()?;
    let auth_path = app_paths::codex_auth_path()?;
    ensure_codex_proxy_backup(&config_path, &auth_path, &base_url)?;

    let config_template = read_optional_text(&config_path)?;
    let config_text = build_codex_proxy_config(config_template.as_deref(), &base_url);
    let auth_json = build_api_auth_json(api_key);
    let serialized_auth = serde_json::to_string_pretty(&auth_json)
        .map_err(|error| format!("序列化本机反代 auth.json 失败: {error}"))?;

    write_file_atomically(&config_path, config_text.as_bytes())?;
    write_file_atomically(&auth_path, serialized_auth.as_bytes())?;
    codex_proxy_binding_state(Some(&base_url))
}

pub(crate) fn restore_codex_proxy_binding() -> Result<CodexProxyBindingState, String> {
    let metadata_path = codex_proxy_backup_metadata_path()?;
    if !metadata_path.is_file() {
        return Err("没有可恢复的 Codex 反代绑定备份。".to_string());
    }

    let metadata_text = fs::read_to_string(&metadata_path).map_err(|error| {
        format!(
            "读取 Codex 反代绑定备份失败 {}: {error}",
            metadata_path.display()
        )
    })?;
    let metadata: CodexProxyBindingBackupMetadata = serde_json::from_str(&metadata_text)
        .map_err(|error| format!("Codex 反代绑定备份元数据无效: {error}"))?;

    let config_path = current_codex_config_path()?;
    let auth_path = app_paths::codex_auth_path()?;
    restore_backup_file(
        &codex_proxy_config_backup_path()?,
        &config_path,
        metadata.config_existed,
    )?;
    restore_backup_file(
        &codex_proxy_auth_backup_path()?,
        &auth_path,
        metadata.auth_existed,
    )?;

    cleanup_codex_proxy_backup();
    codex_proxy_binding_state(None)
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
            StatusCode::NOT_FOUND => {
                "Base URL 不支持 /responses 接口，请确认填写到 /v1 为止。".to_string()
            }
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

fn build_codex_proxy_config(current_config: Option<&str>, base_url: &str) -> String {
    let mut document = parse_config_or_default(current_config);
    document["openai_base_url"] = value(base_url);
    document["model_provider"] = value("openai");
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

fn codex_proxy_backup_dir() -> Result<PathBuf, String> {
    Ok(app_paths::codex_dir()?.join(CODEX_PROXY_BACKUP_DIR_NAME))
}

fn codex_proxy_backup_metadata_path() -> Result<PathBuf, String> {
    Ok(codex_proxy_backup_dir()?.join(CODEX_PROXY_BACKUP_METADATA_FILE_NAME))
}

fn codex_proxy_config_backup_path() -> Result<PathBuf, String> {
    Ok(codex_proxy_backup_dir()?.join(PROFILE_CONFIG_FILE_NAME))
}

fn codex_proxy_auth_backup_path() -> Result<PathBuf, String> {
    Ok(codex_proxy_backup_dir()?.join(PROFILE_AUTH_FILE_NAME))
}

fn normalize_codex_proxy_base_url(value: &str) -> Option<String> {
    let trimmed = value.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn codex_config_openai_base_url(raw: &str) -> Option<String> {
    raw.parse::<DocumentMut>()
        .ok()?
        .get("openai_base_url")?
        .as_str()
        .and_then(normalize_codex_proxy_base_url)
}

fn ensure_codex_proxy_backup(
    config_path: &Path,
    auth_path: &Path,
    bound_base_url: &str,
) -> Result<(), String> {
    let backup_dir = codex_proxy_backup_dir()?;
    ensure_codex_proxy_backup_in_dir(config_path, auth_path, &backup_dir, bound_base_url)
}

fn ensure_codex_proxy_backup_in_dir(
    config_path: &Path,
    auth_path: &Path,
    backup_dir: &Path,
    bound_base_url: &str,
) -> Result<(), String> {
    let metadata_path = backup_dir.join(CODEX_PROXY_BACKUP_METADATA_FILE_NAME);
    if metadata_path.is_file() {
        return Ok(());
    }

    fs::create_dir_all(backup_dir).map_err(|error| {
        format!(
            "创建 Codex 反代备份目录失败 {}: {error}",
            backup_dir.display()
        )
    })?;
    set_private_permissions(backup_dir);

    let config_existed = config_path.is_file();
    if config_existed {
        let config_backup_path = backup_dir.join(PROFILE_CONFIG_FILE_NAME);
        fs::copy(config_path, &config_backup_path).map_err(|error| {
            format!(
                "备份 Codex config.toml 失败 {}: {error}",
                config_path.display()
            )
        })?;
        set_private_permissions(&config_backup_path);
    }

    let auth_existed = auth_path.is_file();
    if auth_existed {
        let auth_backup_path = backup_dir.join(PROFILE_AUTH_FILE_NAME);
        fs::copy(auth_path, &auth_backup_path).map_err(|error| {
            format!("备份 Codex auth.json 失败 {}: {error}", auth_path.display())
        })?;
        set_private_permissions(&auth_backup_path);
    }

    let metadata = CodexProxyBindingBackupMetadata {
        config_existed,
        auth_existed,
        bound_base_url: bound_base_url.to_string(),
        bound_at: utils::now_unix_seconds(),
    };
    let metadata_text = serde_json::to_string_pretty(&metadata)
        .map_err(|error| format!("序列化 Codex 反代备份元数据失败: {error}"))?;
    write_file_atomically(&metadata_path, metadata_text.as_bytes())
}

fn restore_backup_file(
    backup_path: &Path,
    target_path: &Path,
    existed: bool,
) -> Result<(), String> {
    if existed {
        let backup = fs::read(backup_path).map_err(|error| {
            format!("读取 Codex 备份文件失败 {}: {error}", backup_path.display())
        })?;
        write_file_atomically(target_path, &backup)
    } else {
        match fs::remove_file(target_path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
            Err(error) => Err(format!(
                "移除 Codex 临时绑定文件失败 {}: {error}",
                target_path.display()
            )),
        }
    }
}

fn cleanup_codex_proxy_backup() {
    if let Ok(path) = codex_proxy_backup_metadata_path() {
        let _ = fs::remove_file(path);
    }
    if let Ok(path) = codex_proxy_config_backup_path() {
        let _ = fs::remove_file(path);
    }
    if let Ok(path) = codex_proxy_auth_backup_path() {
        let _ = fs::remove_file(path);
    }
    if let Ok(path) = codex_proxy_backup_dir() {
        let _ = fs::remove_dir(path);
    }
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

#[cfg(test)]
mod tests {
    use super::build_codex_proxy_config;
    use super::ensure_codex_proxy_backup_in_dir;
    use super::normalize_codex_proxy_base_url;
    use super::CODEX_PROXY_BACKUP_METADATA_FILE_NAME;
    use super::PROFILE_AUTH_FILE_NAME;
    use super::PROFILE_CONFIG_FILE_NAME;
    use std::env;
    use std::fs;
    use std::path::PathBuf;
    use toml_edit::DocumentMut;
    use uuid::Uuid;

    fn unique_test_dir(name: &str) -> PathBuf {
        env::temp_dir().join(format!(
            "codex-tools-profile-files-test-{name}-{}",
            Uuid::new_v4()
        ))
    }

    #[test]
    fn build_codex_proxy_config_points_builtin_openai_at_local_proxy() {
        let config = build_codex_proxy_config(
            Some(
                r#"
model = "gpt-5.4"
model_provider = "other"
openai_base_url = "https://api.openai.com/v1"
"#,
            ),
            "http://127.0.0.1:8787/v1",
        );
        let document = config
            .parse::<DocumentMut>()
            .expect("proxy config should parse");

        assert_eq!(
            document["openai_base_url"].as_str(),
            Some("http://127.0.0.1:8787/v1")
        );
        assert_eq!(document["model_provider"].as_str(), Some("openai"));
        assert_eq!(document["model"].as_str(), Some("gpt-5.4"));
    }

    #[test]
    fn normalize_codex_proxy_base_url_trims_trailing_slashes() {
        assert_eq!(
            normalize_codex_proxy_base_url(" http://127.0.0.1:8787/v1/// ").as_deref(),
            Some("http://127.0.0.1:8787/v1")
        );
        assert_eq!(normalize_codex_proxy_base_url("   "), None);
    }

    #[cfg(unix)]
    #[test]
    fn ensure_codex_proxy_backup_keeps_backup_dir_searchable() {
        use std::os::unix::fs::PermissionsExt;

        let sandbox = unique_test_dir("backup");
        let codex_dir = sandbox.join("codex");
        let backup_dir = sandbox.join("backup");
        fs::create_dir_all(&codex_dir).expect("create codex dir");
        let config_path = codex_dir.join(PROFILE_CONFIG_FILE_NAME);
        let auth_path = codex_dir.join(PROFILE_AUTH_FILE_NAME);
        fs::write(&config_path, "model = \"gpt-5.4\"\n").expect("write config");
        fs::write(&auth_path, "{\"OPENAI_API_KEY\":\"sk-test\"}\n").expect("write auth");

        ensure_codex_proxy_backup_in_dir(
            &config_path,
            &auth_path,
            &backup_dir,
            "http://127.0.0.1:8787/v1",
        )
        .expect("backup should succeed");

        let mode = fs::metadata(&backup_dir)
            .expect("read backup dir metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o700);
        assert_eq!(
            fs::read_to_string(backup_dir.join(PROFILE_CONFIG_FILE_NAME))
                .expect("read backup config"),
            "model = \"gpt-5.4\"\n"
        );
        assert_eq!(
            fs::read_to_string(backup_dir.join(PROFILE_AUTH_FILE_NAME)).expect("read backup auth"),
            "{\"OPENAI_API_KEY\":\"sk-test\"}\n"
        );
        assert!(backup_dir
            .join(CODEX_PROXY_BACKUP_METADATA_FILE_NAME)
            .is_file());

        let _ = fs::remove_dir_all(&sandbox);
    }

    #[cfg(unix)]
    #[test]
    fn ensure_codex_proxy_backup_repairs_existing_unsearchable_backup_dir() {
        use std::os::unix::fs::PermissionsExt;

        let sandbox = unique_test_dir("backup-repair");
        let codex_dir = sandbox.join("codex");
        let backup_dir = sandbox.join("backup");
        fs::create_dir_all(&codex_dir).expect("create codex dir");
        fs::create_dir_all(&backup_dir).expect("create backup dir");

        let mut permissions = fs::metadata(&backup_dir)
            .expect("read backup dir metadata")
            .permissions();
        permissions.set_mode(0o600);
        fs::set_permissions(&backup_dir, permissions).expect("make backup dir unsearchable");

        let config_path = codex_dir.join(PROFILE_CONFIG_FILE_NAME);
        let auth_path = codex_dir.join(PROFILE_AUTH_FILE_NAME);
        fs::write(&config_path, "model = \"gpt-5.5\"\n").expect("write config");
        fs::write(&auth_path, "{\"OPENAI_API_KEY\":\"sk-test\"}\n").expect("write auth");

        ensure_codex_proxy_backup_in_dir(
            &config_path,
            &auth_path,
            &backup_dir,
            "http://127.0.0.1:8787/v1",
        )
        .expect("backup should repair directory permissions and succeed");

        let mode = fs::metadata(&backup_dir)
            .expect("read repaired backup dir metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o700);
        assert_eq!(
            fs::read_to_string(backup_dir.join(PROFILE_CONFIG_FILE_NAME))
                .expect("read backup config"),
            "model = \"gpt-5.5\"\n"
        );

        let _ = fs::remove_dir_all(&sandbox);
    }
}
