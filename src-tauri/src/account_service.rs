use std::collections::HashMap;
use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

use rfd::FileDialog;
use tauri::AppHandle;
use tauri::Manager;
use zip::write::FileOptions;
use zip::CompressionMethod;

use crate::auth::account_group_key;
use crate::auth::account_variant_key;
use crate::auth::auth_tokens_expire_within;
use crate::auth::current_auth_account_key;
use crate::auth::current_auth_variant_key;
use crate::auth::extract_auth;
use crate::auth::normalize_imported_auth_json;
use crate::auth::normalize_plan_type_key;
use crate::auth::read_current_codex_auth;
use crate::auth::read_current_codex_auth_optional;
use crate::auth::refresh_chatgpt_auth_tokens_serialized;
use crate::models::dedupe_account_variants;
use crate::models::AccountSourceKind;
use crate::models::AccountSummary;
use crate::models::AccountsStore;
use crate::models::AuthJsonImportInput;
use crate::models::CreateApiAccountInput;
use crate::models::ImportAccountFailure;
use crate::models::ImportAccountsResult;
use crate::models::StoredAccount;
use crate::models::UsageSnapshot;
use crate::profile_files;
use crate::state::AppState;
use crate::store::account_store_path_from_data_dir;
use crate::store::load_store;
use crate::store::save_store;
use crate::store::update_account_group_refresh_state_in_path;
use crate::usage::fetch_usage_snapshot;
use crate::utils::now_unix_seconds;
use crate::utils::set_private_permissions;
use crate::utils::short_account;

const DEACTIVATED_WORKSPACE_NOTICE: &str = "该账号已被踢出 team 组织，请重新授权后再刷新。";
const DEACTIVATED_ACCOUNT_NOTICE: &str = "账号被封禁，请检查邮箱";
const AUTH_EXPIRED_NOTICE: &str = "授权过期，请重新登录授权。";
const EXPORT_ARCHIVE_ENTRY_NAME: &str = "accounts.json";
const KEEPALIVE_REFRESH_WINDOW_SECS: i64 = 10 * 60;

#[derive(Debug, Clone)]
struct ImportCandidate {
    source: String,
    auth_json: serde_json::Value,
    label: Option<String>,
    usage: Option<UsageSnapshot>,
    plan_type: Option<String>,
    email: Option<String>,
}

struct PreparedImport {
    principal_id: String,
    auth_json: serde_json::Value,
    account_id: String,
    email: Option<String>,
    plan_type: Option<String>,
    usage: Option<UsageSnapshot>,
    label: Option<String>,
}

pub(crate) async fn list_accounts_internal(
    app: &AppHandle,
    state: &AppState,
) -> Result<Vec<AccountSummary>, String> {
    let _guard = state.store_lock.lock().await;
    let store = load_store(app)?;
    let current_account_key = current_auth_account_key();
    let current_variant_key = current_auth_variant_key();
    let mut summaries = store
        .accounts
        .iter()
        .map(|account| {
            account.to_summary(
                current_account_key.as_deref(),
                current_variant_key.as_deref(),
            )
        })
        .collect::<Vec<_>>();

    if !summaries.iter().any(|account| account.is_current) {
        if let Some(active_id) = store.settings.active_account_id.as_deref() {
            if let Some(account) = summaries.iter_mut().find(|account| account.id == active_id) {
                account.is_current = true;
            }
        }
    }

    Ok(summaries)
}

pub(crate) async fn import_current_auth_account_internal(
    app: &AppHandle,
    state: &AppState,
    label: Option<String>,
) -> Result<AccountSummary, String> {
    let auth_json = read_current_codex_auth()?;
    let prepared = prepare_auth_json_import(auth_json, label).await?;
    commit_prepared_import(app, state, prepared).await
}

pub(crate) async fn create_api_account_internal(
    app: &AppHandle,
    state: &AppState,
    input: CreateApiAccountInput,
) -> Result<AccountSummary, String> {
    let label = profile_files::normalize_relay_label(&input.label)?;
    let base_url = profile_files::normalize_relay_base_url(&input.base_url)?;
    let api_key = profile_files::normalize_relay_api_key(&input.api_key)?;
    let model_name = profile_files::normalize_relay_model_name(&input.model_name)?;

    let (last_validated_at, balance_text, profile_last_validation_error) = if input.force_save {
        (None, None, None)
    } else {
        let balance = profile_files::validate_relay_target(&base_url, &api_key, &model_name).await?;
        (Some(now_unix_seconds()), balance, None)
    };

    let current_account_key = current_auth_account_key();
    let current_variant_key = current_auth_variant_key();
    let summary = {
        let mut _guard = state.store_lock.lock().await;
        let mut store = load_store(app)?;
        let now = now_unix_seconds();
        let id = uuid::Uuid::new_v4().to_string();
        let account_id = profile_files::relay_account_id(&id);
        let mut stored = StoredAccount {
            id: id.clone(),
            label,
            source_kind: AccountSourceKind::Relay,
            principal_id: None,
            email: None,
            account_id,
            plan_type: None,
            auth_json: profile_files::build_api_auth_json(&api_key),
            api_base_url: Some(base_url),
            api_key: Some(api_key),
            model_name: Some(model_name),
            balance_text,
            profile_auth_path: None,
            profile_config_path: None,
            profile_auth_ready: false,
            profile_config_ready: false,
            profile_integrity_error: None,
            profile_last_validated_at: last_validated_at,
            profile_last_validation_error,
            added_at: now,
            updated_at: now,
            usage: None,
            usage_error: None,
            auth_refresh_blocked: false,
            auth_refresh_error: None,
        };
        profile_files::sync_account_profile_in_store_path(
            &account_store_path_from_data_dir(
                &app.path()
                    .app_data_dir()
                    .map_err(|error| format!("无法获取应用数据目录: {error}"))?,
            ),
            &mut stored,
        )?;

        let summary = stored.to_summary(
            current_account_key.as_deref(),
            current_variant_key.as_deref(),
        );
        store.accounts.push(stored);
        save_store(app, &store)?;
        summary
    };

    Ok(summary)
}

pub(crate) async fn reauthorize_account_internal(
    app: &AppHandle,
    state: &AppState,
    id: &str,
    auth_json: serde_json::Value,
) -> Result<ImportAccountsResult, String> {
    let prepared = prepare_auth_json_import(auth_json, None).await?;

    let mut _guard = state.store_lock.lock().await;
    let mut store = load_store(app)?;
    let Some(existing) = store.accounts.iter_mut().find(|account| account.id == id) else {
        return Err("未找到要重新授权的账号".to_string());
    };

    validate_reauthorization_target(existing, &prepared)?;
    apply_reauthorized_account(existing, prepared);
    let store_path = account_store_path_from_data_dir(
        &app.path()
            .app_data_dir()
            .map_err(|error| format!("无法获取应用数据目录: {error}"))?,
    );
    profile_files::sync_account_profile_in_store_path(&store_path, existing)?;
    dedupe_account_variants(&mut store.accounts);
    save_store(app, &store)?;

    Ok(ImportAccountsResult {
        total_count: 1,
        imported_count: 0,
        updated_count: 1,
        failures: Vec::new(),
    })
}

pub(crate) async fn import_auth_json_accounts_internal(
    app: &AppHandle,
    state: &AppState,
    items: Vec<AuthJsonImportInput>,
) -> Result<ImportAccountsResult, String> {
    if items.is_empty() {
        return Err("请至少提供一个 JSON 文件或 JSON 文本".to_string());
    }

    let total_count = items.len();
    let mut prepared_imports = Vec::with_capacity(total_count);
    let mut failures = Vec::new();

    for item in items {
        let source = normalize_import_source(&item.source);
        let candidates =
            match expand_import_json_content(&item.content, &source, item.label.as_deref()) {
                Ok(value) => value,
                Err(error) => {
                    failures.push(ImportAccountFailure { source, error });
                    continue;
                }
            };

        for candidate in candidates {
            let candidate_source = candidate.source.clone();
            match prepare_import_candidate(candidate).await {
                Ok(prepared) => prepared_imports.push(prepared),
                Err(error) => failures.push(ImportAccountFailure {
                    source: candidate_source,
                    error,
                }),
            }
        }
    }

    if prepared_imports.is_empty() {
        return Ok(ImportAccountsResult {
            total_count,
            imported_count: 0,
            updated_count: 0,
            failures,
        });
    }

    let current_account_key = current_auth_account_key();
    let current_variant_key = current_auth_variant_key();
    let (imported_count, updated_count) = {
        let mut _guard = state.store_lock.lock().await;
        let mut store = load_store(app)?;
        let mut imported_count = 0usize;
        let mut updated_count = 0usize;
        let mut touched_ids = HashSet::new();
        let store_path = account_store_path_from_data_dir(
            &app.path()
                .app_data_dir()
                .map_err(|error| format!("无法获取应用数据目录: {error}"))?,
        );

        for prepared in prepared_imports {
            let (summary, updated_existing) = upsert_prepared_import(
                &mut store,
                prepared,
                current_account_key.as_deref(),
                current_variant_key.as_deref(),
            );
            touched_ids.insert(summary.id);
            if updated_existing {
                updated_count += 1;
            } else {
                imported_count += 1;
            }
        }

        for account in store
            .accounts
            .iter_mut()
            .filter(|account| touched_ids.contains(&account.id))
        {
            profile_files::sync_account_profile_in_store_path(&store_path, account)?;
        }

        save_store(app, &store)?;
        (imported_count, updated_count)
    };

    Ok(ImportAccountsResult {
        total_count,
        imported_count,
        updated_count,
        failures,
    })
}

pub(crate) async fn export_accounts_zip_internal(
    app: &AppHandle,
    state: &AppState,
    account_key: Option<String>,
) -> Result<Option<String>, String> {
    let selected_account_key = account_key
        .as_deref()
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_string);
    let export_payload = {
        let _guard = state.store_lock.lock().await;
        let mut store = load_store(app)?;
        if let Some(account_key) = selected_account_key.as_ref() {
            store
                .accounts
                .retain(|account| account.account_key() == *account_key);
        }
        serde_json::to_vec_pretty(&store).map_err(|error| format!("序列化账号列表失败: {error}"))?
    };
    let default_file_name = format!("codex-tools-accounts-{}.zip", now_unix_seconds());

    tauri::async_runtime::spawn_blocking(move || {
        export_accounts_zip_sync(&default_file_name, &export_payload)
    })
    .await
    .map_err(|error| format!("导出账号列表失败: {error}"))?
}

pub(crate) async fn delete_account_internal(
    app: &AppHandle,
    state: &AppState,
    id: &str,
) -> Result<(), String> {
    let _guard = state.store_lock.lock().await;
    let mut store = load_store(app)?;
    let removed_current = store.settings.active_account_id.as_deref() == Some(id);
    let original_len = store.accounts.len();
    store.accounts.retain(|account| account.id != id);

    if original_len == store.accounts.len() {
        return Err("未找到要删除的账号".to_string());
    }

    if removed_current {
        store.settings.active_account_id = None;
    }

    save_store(app, &store)?;
    Ok(())
}

pub(crate) async fn update_account_label_internal(
    app: &AppHandle,
    state: &AppState,
    account_key: &str,
    label: String,
) -> Result<String, String> {
    let resolved_label =
        normalize_custom_label(Some(label)).ok_or_else(|| "账号别名不能为空".to_string())?;
    let now = now_unix_seconds();

    let _guard = state.store_lock.lock().await;
    let mut store = load_store(app)?;
    let mut updated = false;

    for account in store
        .accounts
        .iter_mut()
        .filter(|account| account.account_key() == account_key)
    {
        account.label = resolved_label.clone();
        account.updated_at = now;
        updated = true;
    }

    if !updated {
        return Err("未找到要设置别名的账号".to_string());
    }

    save_store(app, &store)?;
    Ok(resolved_label)
}

/// 拉取并刷新所有账号用量，返回可直接用于前端/状态栏显示的摘要。
///
/// 为避免“后台刷新覆盖新增账号”的竞态：
/// 1) 先拿快照用于网络请求；
/// 2) 请求完成后重新加载最新 store 并按账号组写回。
#[derive(Debug)]
struct RefreshTarget {
    account_key: String,
    auth_json: serde_json::Value,
    auth_is_current: bool,
    auth_refresh_blocked: bool,
    auth_refresh_error: Option<String>,
    updated_at: i64,
}

#[derive(Debug)]
struct RefreshOutcome {
    usage: Option<crate::models::UsageSnapshot>,
    usage_error: Option<String>,
    updated_at: i64,
    auth_plan_type: Option<String>,
    auth_email: Option<String>,
    auth_json: serde_json::Value,
    auth_is_current: bool,
    auth_refreshed: bool,
    auth_refresh_blocked: bool,
    auth_refresh_error: Option<String>,
}

pub(crate) async fn refresh_all_usage_internal(
    app: &AppHandle,
    state: &AppState,
    force_auth_refresh: bool,
) -> Result<Vec<AccountSummary>, String> {
    let current_auth_override: Option<(String, serde_json::Value)> =
        read_current_codex_auth_optional()
            .ok()
            .flatten()
            .and_then(|auth_json| {
                extract_auth(&auth_json).ok().map(|auth| {
                    (
                        account_group_key(&auth.principal_id, &auth.account_id),
                        auth_json,
                    )
                })
            });

    let refresh_targets: Vec<RefreshTarget> = {
        let _guard = state.store_lock.lock().await;
        let store = load_store(app)?;
        build_refresh_targets(store.accounts, current_auth_override.as_ref())
    };

    let mut outcomes: HashMap<String, RefreshOutcome> =
        HashMap::with_capacity(refresh_targets.len());
    for target in refresh_targets {
        let account_key = target.account_key.clone();
        let outcome = refresh_usage_for_target(app, state, &target, force_auth_refresh).await;
        outcomes.insert(account_key, outcome);
    }

    let store = {
        let _guard = state.store_lock.lock().await;
        let mut latest_store = load_store(app)?;

        for account in &mut latest_store.accounts {
            let Some(outcome) = outcomes.get(&account.account_key()) else {
                continue;
            };

            account.updated_at = outcome.updated_at;
            account.auth_json = outcome.auth_json.clone();
            account.auth_refresh_blocked = outcome.auth_refresh_blocked;
            account.auth_refresh_error = outcome.auth_refresh_error.clone();
            account.email = outcome.auth_email.clone().or(account.email.clone());
            let preferred_auth_plan_type = if outcome.auth_is_current || outcome.auth_refreshed {
                outcome.auth_plan_type.clone()
            } else {
                outcome.auth_plan_type.clone().or(account.plan_type.clone())
            };
            if let Some(snapshot) = outcome.usage.clone() {
                let mut resolved_snapshot = snapshot;
                let resolved_plan_type = preferred_auth_plan_type
                    .clone()
                    .or(resolved_snapshot.plan_type.clone());
                resolved_snapshot.plan_type = resolved_plan_type.clone();
                account.plan_type = resolved_plan_type;
                account.usage = Some(resolved_snapshot);
            }
            if let Some(err) = outcome.usage_error.clone() {
                if preferred_auth_plan_type.is_some() {
                    account.plan_type = preferred_auth_plan_type;
                }
                account.usage_error = Some(err);
            } else if outcome.usage.is_some() {
                account.usage_error = None;
            }
        }

        dedupe_account_variants(&mut latest_store.accounts);
        save_store(app, &latest_store)?;
        latest_store
    };

    // 与当前 auth 文件重新对齐，确保 current 标签准确。
    let current_account_key = current_auth_account_key();
    let current_variant_key = current_auth_variant_key();
    let mut summaries: Vec<AccountSummary> = store
        .accounts
        .iter()
        .map(|account| {
            account.to_summary(
                current_account_key.as_deref(),
                current_variant_key.as_deref(),
            )
        })
        .collect();

    if !summaries.iter().any(|account| account.is_current) {
        if let Some(active_id) = store.settings.active_account_id.as_deref() {
            if let Some(account) = summaries.iter_mut().find(|account| account.id == active_id) {
                account.is_current = true;
            }
        }
    }

    Ok(summaries)
}

fn build_refresh_targets(
    accounts: Vec<StoredAccount>,
    current_auth_override: Option<&(String, serde_json::Value)>,
) -> Vec<RefreshTarget> {
    let mut targets_by_account_key: HashMap<String, RefreshTarget> = HashMap::new();

    for account in accounts {
        if matches!(account.source_kind, AccountSourceKind::Relay) {
            continue;
        }

        let account_key = account.account_key();
        let current_override = current_auth_override
            .filter(|(current_account_key, _)| current_account_key == &account_key);
        let auth_is_current = current_override.is_some();
        let auth_json = current_override
            .map(|(_, auth_json)| auth_json.clone())
            .unwrap_or(account.auth_json);

        let candidate = RefreshTarget {
            account_key: account_key.clone(),
            auth_json,
            auth_is_current,
            auth_refresh_blocked: account.auth_refresh_blocked,
            auth_refresh_error: account.auth_refresh_error.clone(),
            updated_at: account.updated_at,
        };

        match targets_by_account_key.get_mut(&account_key) {
            Some(existing) => {
                if should_replace_refresh_target(existing, &candidate) {
                    *existing = candidate;
                } else if existing.auth_refresh_error.is_none() {
                    existing.auth_refresh_error = candidate.auth_refresh_error.clone();
                }
            }
            None => {
                targets_by_account_key.insert(account_key, candidate);
            }
        }
    }

    let mut targets = targets_by_account_key.into_values().collect::<Vec<_>>();
    targets.sort_by(|left, right| {
        right
            .auth_is_current
            .cmp(&left.auth_is_current)
            .then(right.updated_at.cmp(&left.updated_at))
            .then(left.account_key.cmp(&right.account_key))
    });
    targets
}

fn should_replace_refresh_target(existing: &RefreshTarget, candidate: &RefreshTarget) -> bool {
    if candidate.auth_is_current != existing.auth_is_current {
        return candidate.auth_is_current;
    }
    if candidate.auth_refresh_blocked != existing.auth_refresh_blocked {
        return !candidate.auth_refresh_blocked;
    }
    candidate.updated_at > existing.updated_at
}

async fn refresh_usage_for_target(
    app: &AppHandle,
    state: &AppState,
    target: &RefreshTarget,
    force_auth_refresh: bool,
) -> RefreshOutcome {
    let mut working_auth_json = target.auth_json.clone();
    let mut refresh_error: Option<String> = None;
    let mut auth_refreshed = false;
    let mut auth_refresh_blocked = target.auth_refresh_blocked;
    let mut auth_refresh_error = target.auth_refresh_error.clone();

    if force_auth_refresh
        && !auth_refresh_blocked
        && auth_tokens_expire_within(&working_auth_json, KEEPALIVE_REFRESH_WINDOW_SECS)
    {
        match refresh_chatgpt_auth_tokens_serialized(&working_auth_json, &state.auth_refresh_lock)
            .await
        {
            Ok(refreshed) => {
                working_auth_json = refreshed;
                auth_refreshed = true;
                auth_refresh_blocked = false;
                auth_refresh_error = None;
                if let Err(err) = persist_account_refresh_state(
                    app,
                    state,
                    &target.account_key,
                    Some(&working_auth_json),
                    false,
                    None,
                )
                .await
                {
                    refresh_error = Some(err);
                }
            }
            Err(err) => {
                handle_refresh_failure(
                    app,
                    state,
                    &target.account_key,
                    &err,
                    &mut auth_refresh_blocked,
                    &mut auth_refresh_error,
                    &mut refresh_error,
                )
                .await;
            }
        }
    }

    let mut extracted = extract_auth(&working_auth_json);
    let mut fetch_result = match &extracted {
        Ok(auth) => fetch_usage_snapshot(&auth.access_token, &auth.account_id).await,
        Err(err) => Err(err.clone()),
    };

    if !auth_refresh_blocked && should_retry_with_token_refresh(&fetch_result) {
        match refresh_chatgpt_auth_tokens_serialized(&working_auth_json, &state.auth_refresh_lock)
            .await
        {
            Ok(refreshed) => {
                working_auth_json = refreshed;
                auth_refreshed = true;
                auth_refresh_blocked = false;
                auth_refresh_error = None;
                if let Err(err) = persist_account_refresh_state(
                    app,
                    state,
                    &target.account_key,
                    Some(&working_auth_json),
                    false,
                    None,
                )
                .await
                {
                    refresh_error = Some(err);
                }
                extracted = extract_auth(&working_auth_json);
                fetch_result = match &extracted {
                    Ok(auth) => fetch_usage_snapshot(&auth.access_token, &auth.account_id).await,
                    Err(err) => Err(err.clone()),
                };
            }
            Err(err) => {
                handle_refresh_failure(
                    app,
                    state,
                    &target.account_key,
                    &err,
                    &mut auth_refresh_blocked,
                    &mut auth_refresh_error,
                    &mut refresh_error,
                )
                .await;
            }
        }
    }

    let (auth_plan_type, auth_email) = match &extracted {
        Ok(auth) => (auth.plan_type.clone(), auth.email.clone()),
        Err(_) => (None, None),
    };

    let updated_at = now_unix_seconds();
    let usage = fetch_result.as_ref().ok().cloned();
    let usage_error = match fetch_result {
        Ok(_) => refresh_error.as_deref().map(normalize_usage_error_message),
        Err(err) => {
            let combined_error = if let Some(refresh_err) = refresh_error.as_deref() {
                format!("{err} | 令牌刷新失败: {refresh_err}")
            } else {
                err
            };
            Some(normalize_usage_error_message(&combined_error))
        }
    };

    RefreshOutcome {
        usage,
        usage_error,
        updated_at,
        auth_plan_type,
        auth_email,
        auth_json: working_auth_json,
        auth_is_current: target.auth_is_current,
        auth_refreshed,
        auth_refresh_blocked,
        auth_refresh_error,
    }
}

async fn handle_refresh_failure(
    app: &AppHandle,
    state: &AppState,
    account_key: &str,
    raw_error: &str,
    auth_refresh_blocked: &mut bool,
    auth_refresh_error: &mut Option<String>,
    refresh_error: &mut Option<String>,
) {
    if should_suspend_auth_keepalive(raw_error) {
        let normalized_error = normalize_usage_error_message(raw_error);
        *auth_refresh_blocked = true;
        *auth_refresh_error = Some(normalized_error.clone());
        if let Err(err) = persist_account_refresh_state(
            app,
            state,
            account_key,
            None,
            true,
            Some(normalized_error.as_str()),
        )
        .await
        {
            *refresh_error = Some(err);
        }
        return;
    }

    *refresh_error = Some(raw_error.to_string());
}

async fn persist_account_refresh_state(
    app: &AppHandle,
    state: &AppState,
    account_key: &str,
    auth_json: Option<&serde_json::Value>,
    auth_refresh_blocked: bool,
    auth_refresh_error: Option<&str>,
) -> Result<(), String> {
    let _guard = state.store_lock.lock().await;
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("无法获取应用数据目录: {error}"))?;
    let store_path = account_store_path_from_data_dir(&data_dir);
    update_account_group_refresh_state_in_path(
        &store_path,
        account_key,
        auth_json,
        auth_refresh_blocked,
        auth_refresh_error,
        now_unix_seconds(),
        true,
    )?;
    Ok(())
}

fn should_retry_with_token_refresh(
    fetch_result: &Result<crate::models::UsageSnapshot, String>,
) -> bool {
    match fetch_result {
        Ok(snapshot) => snapshot.plan_type.is_none(),
        Err(err) => {
            let normalized = err.to_ascii_lowercase();
            normalized.contains("401")
                || normalized.contains("unauthorized")
                || normalized.contains("invalid_token")
                || normalized.contains("deactivated_workspace")
        }
    }
}

fn should_suspend_auth_keepalive(raw_error: &str) -> bool {
    let normalized = raw_error.to_ascii_lowercase();
    normalized.contains("refresh_token_reused")
        || normalized.contains("provided authentication token is expired")
        || normalized
            .contains("your refresh token has already been used to generate a new access token")
        || normalized.contains("please try signing in again")
        || normalized.contains("token is expired")
        || normalized.contains("deactivated_workspace")
        || normalized.contains("your openai account has been deactivated")
        || normalized.contains("account has been deactivated")
        || normalized.contains("account deactivated")
        || normalized.contains("deactivated_user")
        || normalized.contains("auth.json 缺少 refresh_token")
}

fn normalize_usage_error_message(raw_error: &str) -> String {
    let normalized = raw_error.to_ascii_lowercase();
    if normalized.contains("deactivated_workspace") {
        return DEACTIVATED_WORKSPACE_NOTICE.to_string();
    }
    if normalized.contains("your openai account has been deactivated")
        || normalized.contains("account has been deactivated")
        || normalized.contains("account deactivated")
        || normalized.contains("deactivated_user")
        || (normalized.contains("deactivated") && normalized.contains("check your email"))
    {
        return DEACTIVATED_ACCOUNT_NOTICE.to_string();
    }
    if normalized.contains("refresh_token_reused")
        || normalized.contains("provided authentication token is expired")
        || normalized
            .contains("your refresh token has already been used to generate a new access token")
        || normalized.contains("please try signing in again")
        || normalized.contains("token is expired")
        || normalized.contains("auth.json 缺少 refresh_token")
    {
        return AUTH_EXPIRED_NOTICE.to_string();
    }
    raw_error.to_string()
}

async fn prepare_auth_json_import(
    auth_json: serde_json::Value,
    label: Option<String>,
) -> Result<PreparedImport, String> {
    let extracted = extract_auth(&auth_json)?;

    // 用量拉取失败不阻断导入流程，避免账号无法入库。
    let usage = fetch_usage_snapshot(&extracted.access_token, &extracted.account_id)
        .await
        .ok();

    Ok(PreparedImport {
        principal_id: extracted.principal_id,
        auth_json,
        account_id: extracted.account_id,
        email: extracted.email,
        plan_type: extracted.plan_type,
        usage,
        label,
    })
}

async fn prepare_import_candidate(candidate: ImportCandidate) -> Result<PreparedImport, String> {
    let ImportCandidate {
        auth_json,
        label,
        usage,
        plan_type,
        email,
        ..
    } = candidate;

    let extracted = extract_auth(&auth_json)?;

    // 用量拉取失败不阻断导入流程；若来自账号库备份，则保留备份内已有快照。
    let usage = fetch_usage_snapshot(&extracted.access_token, &extracted.account_id)
        .await
        .ok()
        .or(usage);

    Ok(PreparedImport {
        principal_id: extracted.principal_id,
        auth_json,
        account_id: extracted.account_id,
        email: extracted.email.or(email),
        plan_type: extracted.plan_type.or(plan_type),
        usage,
        label,
    })
}

async fn commit_prepared_import(
    app: &AppHandle,
    state: &AppState,
    prepared: PreparedImport,
) -> Result<AccountSummary, String> {
    let current_account_key = current_auth_account_key();
    let current_variant_key = current_auth_variant_key();
    let summary = {
        let mut _guard = state.store_lock.lock().await;
        let mut store = load_store(app)?;
        let (summary, _) = upsert_prepared_import(
            &mut store,
            prepared,
            current_account_key.as_deref(),
            current_variant_key.as_deref(),
        );
        let store_path = account_store_path_from_data_dir(
            &app.path()
                .app_data_dir()
                .map_err(|error| format!("无法获取应用数据目录: {error}"))?,
        );
        if let Some(account) = store.accounts.iter_mut().find(|account| account.id == summary.id) {
            profile_files::sync_account_profile_in_store_path(&store_path, account)?;
        }
        save_store(app, &store)?;
        store
            .accounts
            .iter()
            .find(|account| account.id == summary.id)
            .map(|account| {
                account.to_summary(
                    current_account_key.as_deref(),
                    current_variant_key.as_deref(),
                )
            })
            .unwrap_or(summary)
    };

    Ok(summary)
}

fn export_accounts_zip_sync(
    default_file_name: &str,
    export_payload: &[u8],
) -> Result<Option<String>, String> {
    let Some(selected_path) = FileDialog::new()
        .set_title("导出账号列表")
        .add_filter("ZIP archive", &["zip"])
        .set_file_name(default_file_name)
        .save_file()
    else {
        return Ok(None);
    };

    let export_path = ensure_zip_extension(selected_path);
    write_accounts_zip_archive(&export_path, export_payload)?;
    Ok(Some(export_path.to_string_lossy().to_string()))
}

fn write_accounts_zip_archive(path: &Path, export_payload: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("无法解析导出目录 {}", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("创建导出目录失败 {}: {error}", parent.display()))?;

    let temp_path = parent.join(format!(
        ".{}.tmp-{}",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("accounts.zip"),
        uuid::Uuid::new_v4()
    ));

    let write_result = (|| -> Result<(), String> {
        let archive_file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp_path)
            .map_err(|error| format!("创建导出临时文件失败 {}: {error}", temp_path.display()))?;
        let mut archive = zip::ZipWriter::new(archive_file);
        let options = FileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .unix_permissions(0o600);
        archive
            .start_file(EXPORT_ARCHIVE_ENTRY_NAME, options)
            .map_err(|error| format!("创建压缩包内容失败: {error}"))?;
        archive
            .write_all(export_payload)
            .map_err(|error| format!("写入压缩包失败: {error}"))?;
        let archive_file = archive
            .finish()
            .map_err(|error| format!("完成压缩包写入失败: {error}"))?;
        archive_file
            .sync_all()
            .map_err(|error| format!("刷新导出文件失败 {}: {error}", temp_path.display()))?;
        drop(archive_file);
        set_private_permissions(&temp_path);

        #[cfg(target_family = "unix")]
        {
            fs::rename(&temp_path, path).map_err(|error| {
                format!(
                    "写入导出文件失败 {} -> {}: {error}",
                    temp_path.display(),
                    path.display()
                )
            })?;

            let parent_dir = fs::File::open(parent)
                .map_err(|error| format!("打开导出目录失败 {}: {error}", parent.display()))?;
            parent_dir
                .sync_all()
                .map_err(|error| format!("刷新导出目录失败 {}: {error}", parent.display()))?;
        }

        #[cfg(not(target_family = "unix"))]
        {
            if path.exists() {
                fs::remove_file(path)
                    .map_err(|error| format!("删除旧导出文件失败 {}: {error}", path.display()))?;
            }
            fs::rename(&temp_path, path).map_err(|error| {
                format!(
                    "写入导出文件失败 {} -> {}: {error}",
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

fn ensure_zip_extension(path: PathBuf) -> PathBuf {
    let has_zip_extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.eq_ignore_ascii_case("zip"))
        .unwrap_or(false);

    if has_zip_extension {
        path
    } else {
        path.with_extension("zip")
    }
}

fn upsert_prepared_import(
    store: &mut AccountsStore,
    prepared: PreparedImport,
    current_account_key: Option<&str>,
    current_variant_key: Option<&str>,
) -> (AccountSummary, bool) {
    let PreparedImport {
        principal_id,
        auth_json,
        account_id,
        email,
        plan_type,
        usage,
        label,
    } = prepared;

    let now = now_unix_seconds();
    let resolved_label = normalize_custom_label(label)
        .unwrap_or_else(|| fallback_account_label(email.as_deref(), &account_id));
    let resolved_plan_type = plan_type.or_else(|| {
        usage
            .as_ref()
            .and_then(|snapshot| snapshot.plan_type.clone())
    });
    let resolved_account_key = account_group_key(&principal_id, &account_id);
    let resolved_plan_key = normalize_plan_type_key(resolved_plan_type.as_deref());
    let resolved_variant_key =
        account_variant_key(&principal_id, &account_id, resolved_plan_type.as_deref());

    if let Some(existing) = store
        .accounts
        .iter_mut()
        .find(|account| account.variant_key() == resolved_variant_key)
    {
        apply_prepared_import_to_account(
            existing,
            principal_id.clone(),
            resolved_label,
            email,
            resolved_plan_type.clone(),
            auth_json,
            usage,
            now,
        );
        (
            existing.to_summary(current_account_key, current_variant_key),
            true,
        )
    } else if resolved_plan_key != "unknown" {
        if let Some(existing) = store.accounts.iter_mut().find(|account| {
            account.account_key() == resolved_account_key
                && normalize_plan_type_key(account.resolved_plan_type().as_deref()) == "unknown"
        }) {
            apply_prepared_import_to_account(
                existing,
                principal_id.clone(),
                resolved_label,
                email,
                resolved_plan_type.clone(),
                auth_json,
                usage,
                now,
            );
            return (
                existing.to_summary(current_account_key, current_variant_key),
                true,
            );
        }

        let stored = StoredAccount {
            id: uuid::Uuid::new_v4().to_string(),
            label: resolved_label,
            source_kind: AccountSourceKind::Chatgpt,
            principal_id: Some(principal_id.clone()),
            email,
            account_id,
            plan_type: resolved_plan_type,
            auth_json,
            api_base_url: None,
            api_key: None,
            model_name: None,
            balance_text: None,
            profile_auth_path: None,
            profile_config_path: None,
            profile_auth_ready: false,
            profile_config_ready: false,
            profile_integrity_error: None,
            profile_last_validated_at: None,
            profile_last_validation_error: None,
            added_at: now,
            updated_at: now,
            usage,
            usage_error: None,
            auth_refresh_blocked: false,
            auth_refresh_error: None,
        };
        let summary = stored.to_summary(current_account_key, current_variant_key);
        store.accounts.push(stored);
        (summary, false)
    } else {
        let stored = StoredAccount {
            id: uuid::Uuid::new_v4().to_string(),
            label: resolved_label,
            source_kind: AccountSourceKind::Chatgpt,
            principal_id: Some(principal_id),
            email,
            account_id,
            plan_type: resolved_plan_type,
            auth_json,
            api_base_url: None,
            api_key: None,
            model_name: None,
            balance_text: None,
            profile_auth_path: None,
            profile_config_path: None,
            profile_auth_ready: false,
            profile_config_ready: false,
            profile_integrity_error: None,
            profile_last_validated_at: None,
            profile_last_validation_error: None,
            added_at: now,
            updated_at: now,
            usage,
            usage_error: None,
            auth_refresh_blocked: false,
            auth_refresh_error: None,
        };
        let summary = stored.to_summary(current_account_key, current_variant_key);
        store.accounts.push(stored);
        (summary, false)
    }
}

fn apply_prepared_import_to_account(
    existing: &mut StoredAccount,
    principal_id: String,
    label: String,
    email: Option<String>,
    plan_type: Option<String>,
    auth_json: serde_json::Value,
    usage: Option<UsageSnapshot>,
    now: i64,
) {
    existing.label = label;
    existing.source_kind = AccountSourceKind::Chatgpt;
    existing.principal_id = Some(principal_id);
    existing.email = email;
    existing.plan_type = plan_type.or(existing.plan_type.clone());
    existing.auth_json = auth_json;
    existing.api_base_url = None;
    existing.api_key = None;
    existing.model_name = None;
    existing.balance_text = None;
    existing.updated_at = now;
    existing.usage = usage;
    existing.usage_error = None;
    existing.auth_refresh_blocked = false;
    existing.auth_refresh_error = None;
}

fn apply_reauthorized_account(existing: &mut StoredAccount, prepared: PreparedImport) {
    let PreparedImport {
        principal_id,
        auth_json,
        account_id,
        email,
        plan_type,
        usage,
        ..
    } = prepared;

    let now = now_unix_seconds();
    let resolved_plan_type = plan_type.or_else(|| {
        usage
            .as_ref()
            .and_then(|snapshot| snapshot.plan_type.clone())
    });

    existing.principal_id = Some(principal_id);
    existing.source_kind = AccountSourceKind::Chatgpt;
    existing.email = email.or_else(|| existing.email.clone());
    existing.account_id = account_id;
    existing.plan_type = resolved_plan_type;
    existing.auth_json = auth_json;
    existing.api_base_url = None;
    existing.api_key = None;
    existing.model_name = None;
    existing.balance_text = None;
    existing.updated_at = now;
    existing.usage = usage;
    existing.usage_error = None;
    existing.auth_refresh_blocked = false;
    existing.auth_refresh_error = None;
}

fn validate_reauthorization_target(
    existing: &StoredAccount,
    prepared: &PreparedImport,
) -> Result<(), String> {
    if let (Some(existing_email), Some(new_email)) =
        (existing.email.as_deref(), prepared.email.as_deref())
    {
        if existing_email.trim().eq_ignore_ascii_case(new_email.trim()) {
            return Ok(());
        }
    }

    if existing
        .principal_id
        .as_deref()
        .is_some_and(|value| value.trim().eq_ignore_ascii_case(prepared.principal_id.trim()))
        || existing.account_id == prepared.account_id
    {
        return Ok(());
    }

    let target_label = existing
        .email
        .as_deref()
        .unwrap_or(existing.label.as_str());
    let new_label = prepared
        .email
        .as_deref()
        .unwrap_or_else(|| prepared.account_id.as_str());
    Err(format!(
        "重新授权得到的账号与目标账号不一致。目标账号: {target_label}；新账号: {new_label}。请确认浏览器登录的是同一个账号。"
    ))
}

fn expand_import_json_content(
    raw: &str,
    source: &str,
    label_override: Option<&str>,
) -> Result<Vec<ImportCandidate>, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("JSON 内容为空".to_string());
    }

    let normalized = trimmed.strip_prefix('\u{feff}').unwrap_or(trimmed);
    let parsed =
        serde_json::from_str(normalized).map_err(|error| format!("JSON 格式无效: {error}"))?;
    expand_import_value(parsed, source, label_override)
}

fn expand_import_value(
    parsed: serde_json::Value,
    source: &str,
    label_override: Option<&str>,
) -> Result<Vec<ImportCandidate>, String> {
    if looks_like_accounts_store(&parsed) {
        return expand_accounts_store_import(parsed, source, label_override);
    }

    if looks_like_stored_account(&parsed) {
        return import_stored_account_candidates(parsed, source, label_override);
    }

    if let Some(items) = parsed.as_array() {
        if items.is_empty() {
            return Err("JSON 内没有可导入的账号".to_string());
        }

        let mut expanded = Vec::with_capacity(items.len());
        for (index, item) in items.iter().cloned().enumerate() {
            let item_source = format!("{source} / #{}", index + 1);
            let candidates = if looks_like_stored_account(&item) {
                import_stored_account_candidates(item, &item_source, label_override)?
            } else {
                vec![ImportCandidate {
                    source: item_source,
                    auth_json: normalize_imported_auth_json(item),
                    label: normalize_custom_label(label_override.map(ToString::to_string)),
                    usage: None,
                    plan_type: None,
                    email: None,
                }]
            };
            expanded.extend(candidates);
        }
        return Ok(expanded);
    }

    Ok(vec![ImportCandidate {
        source: source.to_string(),
        auth_json: normalize_imported_auth_json(parsed),
        label: normalize_custom_label(label_override.map(ToString::to_string)),
        usage: None,
        plan_type: None,
        email: None,
    }])
}

fn expand_accounts_store_import(
    parsed: serde_json::Value,
    source: &str,
    label_override: Option<&str>,
) -> Result<Vec<ImportCandidate>, String> {
    let Some(root) = parsed.as_object() else {
        return Err("账号库备份格式无效（根节点不是对象）".to_string());
    };
    let Some(accounts) = root.get("accounts").and_then(serde_json::Value::as_array) else {
        return Err("账号库备份缺少 accounts 数组".to_string());
    };
    if accounts.is_empty() {
        return Err("账号库备份里没有可导入的账号".to_string());
    }

    let mut expanded = Vec::with_capacity(accounts.len());
    for (index, account) in accounts.iter().cloned().enumerate() {
        let item_source = format!("{source} / #{}", index + 1);
        let candidates = import_stored_account_candidates(account, &item_source, label_override)?;
        expanded.extend(candidates);
    }
    Ok(expanded)
}

fn import_stored_account_candidates(
    parsed: serde_json::Value,
    source: &str,
    label_override: Option<&str>,
) -> Result<Vec<ImportCandidate>, String> {
    let Some(root) = parsed.as_object() else {
        return Err("账号备份格式无效（不是对象）".to_string());
    };

    let auth_json = root
        .get("authJson")
        .or_else(|| root.get("auth_json"))
        .cloned()
        .ok_or_else(|| "账号备份缺少 authJson".to_string())?;

    let stored_label = root
        .get("label")
        .and_then(serde_json::Value::as_str)
        .map(ToString::to_string);
    let email = root
        .get("email")
        .and_then(serde_json::Value::as_str)
        .map(ToString::to_string);
    let plan_type = root
        .get("planType")
        .or_else(|| root.get("plan_type"))
        .and_then(serde_json::Value::as_str)
        .map(ToString::to_string);
    let usage = root
        .get("usage")
        .cloned()
        .and_then(|value| serde_json::from_value::<UsageSnapshot>(value).ok());
    let account_id = root
        .get("accountId")
        .or_else(|| root.get("account_id"))
        .and_then(serde_json::Value::as_str);

    Ok(vec![ImportCandidate {
        source: describe_account_backup_source(
            source,
            normalize_custom_label(stored_label.clone())
                .as_deref()
                .or(email.as_deref())
                .or(account_id),
        ),
        auth_json: normalize_imported_auth_json(auth_json),
        label: normalize_custom_label(label_override.map(ToString::to_string))
            .or_else(|| normalize_custom_label(stored_label)),
        usage,
        plan_type,
        email,
    }])
}

fn looks_like_accounts_store(value: &serde_json::Value) -> bool {
    value
        .as_object()
        .and_then(|root| root.get("accounts"))
        .and_then(serde_json::Value::as_array)
        .is_some()
}

fn looks_like_stored_account(value: &serde_json::Value) -> bool {
    value
        .as_object()
        .map(|root| root.contains_key("authJson") || root.contains_key("auth_json"))
        .unwrap_or(false)
}

fn describe_account_backup_source(source: &str, hint: Option<&str>) -> String {
    let Some(hint) = hint.map(str::trim).filter(|value| !value.is_empty()) else {
        return source.to_string();
    };
    format!("{source} / {hint}")
}

fn normalize_custom_label(label: Option<String>) -> Option<String> {
    label.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn fallback_account_label(email: Option<&str>, account_id: &str) -> String {
    email
        .map(ToString::to_string)
        .unwrap_or_else(|| format!("Codex {}", short_account(account_id)))
}

fn normalize_import_source(source: &str) -> String {
    let trimmed = source.trim();
    if trimmed.is_empty() {
        "未命名 JSON".to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::expand_import_json_content;
    use super::upsert_prepared_import;
    use super::PreparedImport;
    use crate::models::AccountsStore;
    use crate::models::StoredAccount;
    use crate::models::UsageSnapshot;
    use crate::models::UsageWindow;
    use serde_json::json;

    fn usage_snapshot(plan_type: &str) -> UsageSnapshot {
        UsageSnapshot {
            fetched_at: 10,
            plan_type: Some(plan_type.to_string()),
            five_hour: Some(UsageWindow {
                used_percent: 10.0,
                window_seconds: 18_000,
                reset_at: Some(20),
            }),
            one_week: Some(UsageWindow {
                used_percent: 20.0,
                window_seconds: 604_800,
                reset_at: Some(30),
            }),
            credits: None,
        }
    }

    fn prepared_import(
        principal_id: &str,
        account_id: &str,
        email: &str,
        label: &str,
        plan_type: &str,
    ) -> PreparedImport {
        PreparedImport {
            principal_id: principal_id.to_string(),
            auth_json: json!({ "kind": label }),
            account_id: account_id.to_string(),
            email: Some(email.to_string()),
            plan_type: Some(plan_type.to_string()),
            usage: Some(usage_snapshot(plan_type)),
            label: Some(label.to_string()),
        }
    }

    #[test]
    fn expand_import_json_content_supports_exported_accounts_store() {
        let raw = json!({
            "version": 1,
            "accounts": [
                {
                    "id": "stored-1",
                    "label": "Alpha",
                    "email": "alpha@example.com",
                    "accountId": "account-1",
                    "planType": "team",
                    "authJson": {
                        "auth_mode": "chatgpt",
                        "tokens": {
                            "access_token": "access-1",
                            "id_token": "id-1",
                            "refresh_token": "refresh-1"
                        }
                    },
                    "addedAt": 1,
                    "updatedAt": 2,
                    "usage": {
                        "fetchedAt": 3,
                        "planType": "team",
                        "fiveHour": null,
                        "oneWeek": null,
                        "credits": null
                    },
                    "usageError": null
                }
            ],
            "settings": {}
        })
        .to_string();

        let candidates = expand_import_json_content(&raw, "accounts.json", None).unwrap();

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].label.as_deref(), Some("Alpha"));
        assert_eq!(candidates[0].email.as_deref(), Some("alpha@example.com"));
        assert_eq!(candidates[0].plan_type.as_deref(), Some("team"));
        assert_eq!(
            candidates[0]
                .usage
                .as_ref()
                .and_then(|usage| usage.plan_type.as_deref()),
            Some("team")
        );
        assert_eq!(candidates[0].source, "accounts.json / #1 / Alpha");
        assert_eq!(
            candidates[0]
                .auth_json
                .get("tokens")
                .and_then(serde_json::Value::as_object)
                .and_then(|tokens| tokens.get("access_token"))
                .and_then(serde_json::Value::as_str),
            Some("access-1")
        );
    }

    #[test]
    fn expand_import_json_content_supports_single_stored_account_backup() {
        let raw = json!({
            "id": "stored-1",
            "label": "Solo",
            "email": "solo@example.com",
            "accountId": "account-1",
            "authJson": {
                "auth_mode": "chatgpt",
                "tokens": {
                    "access_token": "access-1",
                    "id_token": "id-1",
                    "refresh_token": "refresh-1"
                }
            },
            "addedAt": 1,
            "updatedAt": 2
        })
        .to_string();

        let candidates = expand_import_json_content(&raw, "account.json", None).unwrap();

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].label.as_deref(), Some("Solo"));
        assert_eq!(candidates[0].source, "account.json / Solo");
    }

    #[test]
    fn expand_import_json_content_normalizes_flat_auth_json() {
        let raw = json!({
            "auth_mode": "chatgpt",
            "access_token": "access-1",
            "id_token": "id-1",
            "refresh_token": "refresh-1"
        })
        .to_string();

        let candidates = expand_import_json_content(&raw, "auth.json", None).unwrap();

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].source, "auth.json");
        assert_eq!(
            candidates[0]
                .auth_json
                .get("tokens")
                .and_then(serde_json::Value::as_object)
                .and_then(|tokens| tokens.get("access_token"))
                .and_then(serde_json::Value::as_str),
            Some("access-1")
        );
    }

    #[test]
    fn upsert_prepared_import_reuses_unknown_variant_placeholder() {
        let mut store = AccountsStore::default();
        store.accounts.push(StoredAccount {
            id: "existing".to_string(),
            label: "placeholder".to_string(),
            principal_id: Some("fresh@example.com".to_string()),
            email: Some("fresh@example.com".to_string()),
            account_id: "account-1".to_string(),
            plan_type: None,
            auth_json: json!({ "kind": "old" }),
            added_at: 1,
            updated_at: 1,
            usage: None,
            usage_error: None,
            auth_refresh_blocked: false,
            auth_refresh_error: None,
        });

        let prepared = prepared_import(
            "fresh@example.com",
            "account-1",
            "fresh@example.com",
            "fresh",
            "team",
        );

        let (summary, updated_existing) = upsert_prepared_import(&mut store, prepared, None, None);

        assert!(updated_existing);
        assert_eq!(store.accounts.len(), 1);
        assert_eq!(summary.id, "existing");
        assert_eq!(store.accounts[0].label, "fresh");
        assert_eq!(store.accounts[0].plan_type.as_deref(), Some("team"));
        assert_eq!(
            store.accounts[0]
                .usage
                .as_ref()
                .and_then(|usage| usage.plan_type.as_deref()),
            Some("team")
        );
    }

    #[test]
    fn upsert_prepared_import_prefers_auth_plan_type_over_usage_plan_type() {
        let mut store = AccountsStore::default();
        let prepared = PreparedImport {
            principal_id: "shared@example.com".to_string(),
            auth_json: json!({ "kind": "team-auth" }),
            account_id: "account-1".to_string(),
            email: Some("shared@example.com".to_string()),
            plan_type: Some("team".to_string()),
            usage: Some(usage_snapshot("plus")),
            label: Some("team".to_string()),
        };

        let (summary, updated_existing) = upsert_prepared_import(&mut store, prepared, None, None);

        assert!(!updated_existing);
        assert_eq!(summary.plan_type.as_deref(), Some("team"));
        assert_eq!(store.accounts[0].plan_type.as_deref(), Some("team"));
        assert_eq!(
            store.accounts[0]
                .usage
                .as_ref()
                .and_then(|usage| usage.plan_type.as_deref()),
            Some("plus")
        );
    }

    #[test]
    fn upsert_prepared_import_keeps_same_workspace_different_users_separate() {
        let mut store = AccountsStore::default();

        let first = prepared_import(
            "first@example.com",
            "workspace-1",
            "first@example.com",
            "first",
            "team",
        );
        let second = prepared_import(
            "second@example.com",
            "workspace-1",
            "second@example.com",
            "second",
            "team",
        );

        let (_, updated_first) = upsert_prepared_import(&mut store, first, None, None);
        let (_, updated_second) = upsert_prepared_import(&mut store, second, None, None);

        assert!(!updated_first);
        assert!(!updated_second);
        assert_eq!(store.accounts.len(), 2);
        assert_ne!(
            store.accounts[0].account_key(),
            store.accounts[1].account_key()
        );
        assert_ne!(
            store.accounts[0].variant_key(),
            store.accounts[1].variant_key()
        );
    }
}
