use std::collections::HashMap;
use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::num::NonZeroUsize;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use futures_util::stream;
use futures_util::StreamExt;
use rfd::FileDialog;
use serde_json::json;
use tauri::AppHandle;
use zip::write::FileOptions;
use zip::CompressionMethod;

use crate::app_paths;
use crate::auth::account_group_key;
use crate::auth::account_variant_key;
use crate::auth::auth_tokens_need_keepalive_refresh;
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
use crate::models::TestApiAccountConnectionInput;
use crate::models::TestApiAccountConnectionResult;
use crate::models::UsageSnapshot;
use crate::profile_files;
use crate::state::AppState;
use crate::store::account_store_path_from_data_dir;
use crate::store::load_store;
use crate::store::load_store_from_path;
use crate::store::save_store;
use crate::store::save_store_to_path;
use crate::store::update_account_group_refresh_state_in_path;
use crate::usage::fetch_usage_snapshot;
use crate::usage::resolve_chatgpt_base_origin;
use crate::utils::now_unix_seconds;
use crate::utils::set_private_permissions;
use crate::utils::short_account;
use crate::utils::truncate_for_error;

const DEACTIVATED_WORKSPACE_NOTICE: &str = "该账号已被踢出 team 组织，请重新授权后再刷新。";
const DEACTIVATED_ACCOUNT_NOTICE: &str = "账号被封禁，请检查邮箱";
const AUTH_EXPIRED_NOTICE: &str = "工具保存的授权快照已失效，请重新登录授权。";
const USAGE_AUTH_TOKEN_EXPIRED_NOTICE: &str =
    "用量刷新失败：登录令牌已过期，请刷新用量或切换账号重新校验。";
const EXPORT_ARCHIVE_ENTRY_NAME: &str = "accounts.json";
const KEEPALIVE_REFRESH_WINDOW_SECS: i64 = 10 * 60;
// 对齐 Codex 官方客户端的刷新模型：不要每隔几小时刷新一次 refresh_token。
// 不同账号按稳定 hash 分布在 7-8 天窗口内刷新，避免同一批导入账号集中打到刷新端点。
const KEEPALIVE_LAST_REFRESH_BASE_AGE_SECS: i64 = 7 * 24 * 60 * 60;
const KEEPALIVE_LAST_REFRESH_JITTER_SECS: i64 = 24 * 60 * 60;
const CODEX_KEEPALIVE_INTERVAL_SECS: i64 = 5 * 60 * 60;
const CODEX_KEEPALIVE_TIMEOUT_SECS: u64 = 45;
const CODEX_KEEPALIVE_MODEL: &str = "gpt-5.5";
const CODEX_KEEPALIVE_VERSION: &str = "0.125.0";
const CODEX_KEEPALIVE_USER_AGENT: &str = "codex_cli_rs/0.125.0";
// 账号刷新主要等待外部 HTTP 响应，因此按 CPU 给 2 倍并发；同时用可用内存做下限保护。
const REFRESH_USAGE_WORKERS_PER_CPU: usize = 2;
const REFRESH_USAGE_MEMORY_PER_WORKER_MIB: u64 = 256;

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

#[derive(Debug)]
struct NormalizedApiAccountConnectionInput {
    _label: String,
    base_url: String,
    api_key: String,
    model_name: String,
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
        let balance =
            profile_files::validate_relay_target(&base_url, &api_key, &model_name).await?;
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
            api_proxy_enabled: true,
            codex_keepalive_last_at: None,
        };
        profile_files::sync_account_profile_in_store_path(
            &account_store_path_from_data_dir(&app_paths::app_data_dir(app)?),
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

pub(crate) async fn test_api_account_connection_internal(
    input: TestApiAccountConnectionInput,
) -> Result<TestApiAccountConnectionResult, String> {
    let normalized = normalize_api_account_connection_input(&input)?;
    let balance_text = profile_files::validate_relay_target(
        &normalized.base_url,
        &normalized.api_key,
        &normalized.model_name,
    )
    .await?;

    Ok(TestApiAccountConnectionResult {
        ok: true,
        balance_text,
        message: "连接成功".to_string(),
    })
}

fn normalize_api_account_connection_input(
    input: &TestApiAccountConnectionInput,
) -> Result<NormalizedApiAccountConnectionInput, String> {
    Ok(NormalizedApiAccountConnectionInput {
        _label: profile_files::normalize_relay_label(&input.label)?,
        base_url: profile_files::normalize_relay_base_url(&input.base_url)?,
        api_key: profile_files::normalize_relay_api_key(&input.api_key)?,
        model_name: profile_files::normalize_relay_model_name(&input.model_name)?,
    })
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
    let store_path = account_store_path_from_data_dir(&app_paths::app_data_dir(app)?);
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
    let _guard = state.store_lock.lock().await;
    let store_path = account_store_path_from_data_dir(&app_paths::app_data_dir(app)?);
    import_auth_json_accounts_into_store_path(&store_path, items).await
}

pub(crate) async fn import_auth_json_accounts_into_store_path(
    store_path: &Path,
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
        let mut store = load_store_from_path(store_path)?;
        let mut imported_count = 0usize;
        let mut updated_count = 0usize;
        let mut touched_ids = HashSet::new();

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

        save_store_to_path(store_path, &store)?;
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

pub(crate) async fn update_account_api_proxy_enabled_internal(
    app: &AppHandle,
    state: &AppState,
    account_key: &str,
    enabled: bool,
) -> Result<bool, String> {
    let now = now_unix_seconds();

    let _guard = state.store_lock.lock().await;
    let mut store = load_store(app)?;
    let mut updated = false;

    for account in store
        .accounts
        .iter_mut()
        .filter(|account| account.account_key() == account_key)
    {
        account.api_proxy_enabled = enabled;
        account.updated_at = now;
        updated = true;
    }

    if !updated {
        return Err("未找到要设置反代状态的账号".to_string());
    }

    save_store(app, &store)?;
    Ok(enabled)
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
    codex_keepalive_last_at: Option<i64>,
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
    codex_keepalive_last_at: Option<i64>,
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

    let outcomes =
        refresh_usage_targets_with_workers(app, state, refresh_targets, force_auth_refresh).await;

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
            account.codex_keepalive_last_at = outcome.codex_keepalive_last_at;
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

async fn refresh_usage_targets_with_workers(
    app: &AppHandle,
    state: &AppState,
    refresh_targets: Vec<RefreshTarget>,
    force_auth_refresh: bool,
) -> HashMap<String, RefreshOutcome> {
    let worker_count = refresh_usage_worker_count(refresh_targets.len());
    stream::iter(refresh_targets)
        .map(|target| async move {
            let account_key = target.account_key.clone();
            let outcome = refresh_usage_for_target(app, state, &target, force_auth_refresh).await;
            (account_key, outcome)
        })
        .buffer_unordered(worker_count)
        .collect::<HashMap<_, _>>()
        .await
}

fn refresh_usage_worker_count(target_count: usize) -> usize {
    let cpu_count = std::thread::available_parallelism()
        .map(NonZeroUsize::get)
        .unwrap_or(1);
    let available_memory_mib = available_system_memory_mib();
    let worker_count =
        refresh_usage_worker_count_from_resources(target_count, cpu_count, available_memory_mib);
    log::info!(
        "账号刷新 worker_count={worker_count} target_count={target_count} cpu_count={cpu_count} available_memory_mib={available_memory_mib:?}"
    );
    worker_count
}

fn refresh_usage_worker_count_from_resources(
    target_count: usize,
    cpu_count: usize,
    available_memory_mib: Option<u64>,
) -> usize {
    let target_limit = target_count.max(1);
    let cpu_limit = cpu_count
        .max(1)
        .saturating_mul(REFRESH_USAGE_WORKERS_PER_CPU)
        .max(1);
    let memory_limit = available_memory_mib
        .map(|memory_mib| {
            (memory_mib / REFRESH_USAGE_MEMORY_PER_WORKER_MIB)
                .try_into()
                .unwrap_or(usize::MAX)
        })
        .unwrap_or(usize::MAX)
        .max(1);

    target_limit.min(cpu_limit).min(memory_limit).max(1)
}

fn available_system_memory_mib() -> Option<u64> {
    let mut system = sysinfo::System::new();
    system.refresh_memory();
    let available_memory_bytes = system.available_memory();
    if available_memory_bytes == 0 {
        return None;
    }

    Some(available_memory_bytes / 1024 / 1024)
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
            codex_keepalive_last_at: account.codex_keepalive_last_at,
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

fn keepalive_max_last_refresh_age_secs(account_key: &str) -> i64 {
    if KEEPALIVE_LAST_REFRESH_JITTER_SECS <= 0 {
        return KEEPALIVE_LAST_REFRESH_BASE_AGE_SECS;
    }

    // 使用稳定 FNV-1a hash，而不是随机种子的 DefaultHasher，确保同一账号跨进程刷新窗口一致。
    let hash = account_key
        .as_bytes()
        .iter()
        .fold(0xcbf29ce484222325u64, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
        });
    KEEPALIVE_LAST_REFRESH_BASE_AGE_SECS
        + i64::try_from(hash % KEEPALIVE_LAST_REFRESH_JITTER_SECS as u64).unwrap_or(0)
}

fn auth_json_refresh_token(auth_json: &serde_json::Value) -> Option<&str> {
    auth_json
        .get("tokens")
        .and_then(serde_json::Value::as_object)
        .and_then(|tokens| tokens.get("refresh_token"))
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn auth_json_account_key(auth_json: &serde_json::Value) -> Option<String> {
    extract_auth(auth_json)
        .ok()
        .map(|auth| account_group_key(&auth.principal_id, &auth.account_id))
}

async fn latest_account_auth_json(
    app: &AppHandle,
    state: &AppState,
    account_key: &str,
) -> Option<serde_json::Value> {
    if let Ok(Some(current_auth_json)) = read_current_codex_auth_optional() {
        if auth_json_account_key(&current_auth_json).as_deref() == Some(account_key) {
            return Some(current_auth_json);
        }
    }

    let _guard = state.store_lock.lock().await;
    let store = load_store(app).ok()?;
    store
        .accounts
        .into_iter()
        .filter(|account| {
            !matches!(account.source_kind, AccountSourceKind::Relay)
                && account.account_key() == account_key
        })
        .max_by(|left, right| {
            (!left.auth_refresh_blocked)
                .cmp(&(!right.auth_refresh_blocked))
                .then(left.updated_at.cmp(&right.updated_at))
        })
        .map(|account| account.auth_json)
}

async fn refresh_latest_auth_json_if_newer(
    app: &AppHandle,
    state: &AppState,
    account_key: &str,
    auth_json: &serde_json::Value,
) -> serde_json::Value {
    let Some(latest) = latest_account_auth_json(app, state, account_key).await else {
        return auth_json.clone();
    };

    let current_refresh_token = auth_json_refresh_token(auth_json);
    let latest_refresh_token = auth_json_refresh_token(&latest);
    if latest_refresh_token.is_some() && latest_refresh_token != current_refresh_token {
        log::info!("刷新前检测到账号已有更新后的授权快照，改用最新快照 account_key={account_key}");
        return latest;
    }

    auth_json.clone()
}

fn is_stale_refresh_snapshot_error(raw_error: &str) -> bool {
    let normalized = raw_error.to_ascii_lowercase();
    normalized.contains("refresh_token_reused")
        || normalized
            .contains("your refresh token has already been used to generate a new access token")
        || is_invalid_refresh_grant(&normalized)
}

async fn recover_refresh_failure_from_latest_snapshot(
    app: &AppHandle,
    state: &AppState,
    account_key: &str,
    attempted_auth_json: &serde_json::Value,
    raw_error: &str,
) -> Option<serde_json::Value> {
    if !is_stale_refresh_snapshot_error(raw_error) {
        return None;
    }

    let latest = latest_account_auth_json(app, state, account_key).await?;
    let attempted_refresh_token = auth_json_refresh_token(attempted_auth_json);
    let latest_refresh_token = auth_json_refresh_token(&latest);
    if latest_refresh_token.is_some() && latest_refresh_token != attempted_refresh_token {
        log::warn!(
            "刷新失败后检测到本地已有更新后的授权快照，使用最新快照恢复 account_key={account_key}"
        );
        return Some(latest);
    }

    None
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

    if force_auth_refresh && !auth_refresh_blocked {
        working_auth_json =
            refresh_latest_auth_json_if_newer(app, state, &target.account_key, &working_auth_json)
                .await;
    }

    if force_auth_refresh
        && !auth_refresh_blocked
        && auth_tokens_need_keepalive_refresh(
            &working_auth_json,
            KEEPALIVE_REFRESH_WINDOW_SECS,
            keepalive_max_last_refresh_age_secs(&target.account_key),
        )
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
                if let Some(recovered) = recover_refresh_failure_from_latest_snapshot(
                    app,
                    state,
                    &target.account_key,
                    &working_auth_json,
                    &err,
                )
                .await
                {
                    working_auth_json = recovered;
                    auth_refreshed = true;
                    auth_refresh_blocked = false;
                    auth_refresh_error = None;
                } else {
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
                if let Some(recovered) = recover_refresh_failure_from_latest_snapshot(
                    app,
                    state,
                    &target.account_key,
                    &working_auth_json,
                    &err,
                )
                .await
                {
                    working_auth_json = recovered;
                    auth_refreshed = true;
                    auth_refresh_blocked = false;
                    auth_refresh_error = None;
                    extracted = extract_auth(&working_auth_json);
                    fetch_result = match &extracted {
                        Ok(auth) => {
                            fetch_usage_snapshot(&auth.access_token, &auth.account_id).await
                        }
                        Err(err) => Err(err.clone()),
                    };
                } else {
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
    }

    let (auth_plan_type, auth_email) = match &extracted {
        Ok(auth) => (auth.plan_type.clone(), auth.email.clone()),
        Err(_) => (None, None),
    };

    let usage = fetch_result.as_ref().ok().cloned();
    let now = now_unix_seconds();
    let codex_keepalive_last_at = maybe_run_codex_keepalive_ping(
        target.account_key.as_str(),
        &extracted,
        target.codex_keepalive_last_at,
        now,
    )
    .await;
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
        updated_at: now,
        auth_plan_type,
        auth_email,
        auth_json: working_auth_json,
        auth_is_current: target.auth_is_current,
        auth_refreshed,
        auth_refresh_blocked,
        auth_refresh_error,
        codex_keepalive_last_at,
    }
}

async fn maybe_run_codex_keepalive_ping(
    account_key: &str,
    extracted: &Result<crate::models::ExtractedAuth, String>,
    last_at: Option<i64>,
    now: i64,
) -> Option<i64> {
    if last_at
        .map(|last_at| now.saturating_sub(last_at) < CODEX_KEEPALIVE_INTERVAL_SECS)
        .unwrap_or(false)
    {
        return last_at;
    }

    let auth = match extracted {
        Ok(auth) => auth,
        Err(error) => {
            log::warn!("跳过 Codex 账号保活请求 account_key={account_key}: {error}");
            return last_at;
        }
    };

    match send_codex_keepalive_ping(&auth.access_token, &auth.account_id).await {
        Ok(()) => Some(now),
        Err(error) => {
            log::warn!("Codex 账号保活请求失败 account_key={account_key}: {error}");
            last_at
        }
    }
}

async fn send_codex_keepalive_ping(access_token: &str, account_id: &str) -> Result<(), String> {
    let upstream_url = format!(
        "{}/backend-api/codex/responses",
        resolve_chatgpt_base_origin().trim_end_matches('/')
    );
    let session_id = uuid::Uuid::new_v4().to_string();
    let payload = json!({
        "model": CODEX_KEEPALIVE_MODEL,
        "stream": true,
        "store": false,
        "instructions": "",
        "parallel_tool_calls": true,
        "reasoning": {
            "effort": "medium",
            "summary": "auto"
        },
        "include": ["reasoning.encrypted_content"],
        "input": [{
            "type": "message",
            "role": "user",
            "content": [{
                "type": "input_text",
                "text": "1+1"
            }]
        }]
    });

    let client = reqwest::Client::builder()
        .user_agent(CODEX_KEEPALIVE_USER_AGENT)
        .timeout(Duration::from_secs(CODEX_KEEPALIVE_TIMEOUT_SECS))
        .build()
        .map_err(|error| format!("创建 Codex 保活 HTTP 客户端失败: {error}"))?;

    let response = client
        .post(&upstream_url)
        .header("Authorization", format!("Bearer {access_token}"))
        .header("ChatGPT-Account-Id", account_id)
        .header("Accept", "text/event-stream")
        .header("Content-Type", "application/json")
        .header("Originator", "codex_cli_rs")
        .header("Version", CODEX_KEEPALIVE_VERSION)
        .header("Session_id", session_id)
        .header("User-Agent", CODEX_KEEPALIVE_USER_AGENT)
        .header("Connection", "Keep-Alive")
        .json(&payload)
        .send()
        .await
        .map_err(|error| format!("请求 Codex 保活接口失败 {upstream_url}: {error}"))?;

    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(format!(
            "{upstream_url} -> {status}: {}",
            truncate_for_error(&body, 180)
        ));
    }

    if !body.contains("response.completed") && !body.contains("\"status\":\"completed\"") {
        return Err(format!(
            "{upstream_url} -> 未收到完成事件: {}",
            truncate_for_error(&body, 180)
        ));
    }

    Ok(())
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
    let data_dir = app_paths::app_data_dir(app)?;
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
    is_refresh_token_invalidation_error(&normalized)
        || normalized.contains("deactivated_workspace")
        || normalized.contains("your openai account has been deactivated")
        || normalized.contains("account has been deactivated")
        || normalized.contains("account deactivated")
        || normalized.contains("deactivated_user")
}

fn is_refresh_token_invalidation_error(normalized: &str) -> bool {
    normalized.contains("refresh_token_reused")
        || is_invalid_refresh_grant(normalized)
        || normalized
            .contains("your refresh token has already been used to generate a new access token")
        || normalized.contains("refresh token expired")
        || normalized.contains("refresh_token expired")
        || normalized.contains("expired refresh token")
        || normalized.contains("refresh token is expired")
        || normalized.contains("refresh token revoked")
        || normalized.contains("refresh_token_revoked")
        || normalized.contains("refresh token invalid")
        || normalized.contains("invalid refresh token")
        || normalized.contains("please try signing in again")
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
    if is_refresh_token_invalidation_error(&normalized) {
        return AUTH_EXPIRED_NOTICE.to_string();
    }
    if normalized.contains("provided authentication token is expired")
        || normalized.contains("token is expired")
    {
        return USAGE_AUTH_TOKEN_EXPIRED_NOTICE.to_string();
    }
    raw_error.to_string()
}

fn is_invalid_refresh_grant(normalized_error: &str) -> bool {
    normalized_error.contains("invalid_grant")
        && (normalized_error.contains("refresh")
            || normalized_error.contains("expired")
            || normalized_error.contains("revoked")
            || normalized_error.contains("invalid"))
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
        let store_path = account_store_path_from_data_dir(&app_paths::app_data_dir(app)?);
        if let Some(account) = store
            .accounts
            .iter_mut()
            .find(|account| account.id == summary.id)
        {
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
            api_proxy_enabled: true,
            codex_keepalive_last_at: None,
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
            api_proxy_enabled: true,
            codex_keepalive_last_at: None,
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

    if existing.principal_id.as_deref().is_some_and(|value| {
        value
            .trim()
            .eq_ignore_ascii_case(prepared.principal_id.trim())
    }) || existing.account_id == prepared.account_id
    {
        return Ok(());
    }

    let target_label = existing.email.as_deref().unwrap_or(existing.label.as_str());
    let new_label = prepared
        .email
        .as_deref()
        .unwrap_or(prepared.account_id.as_str());
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
    use super::is_stale_refresh_snapshot_error;
    use super::keepalive_max_last_refresh_age_secs;
    use super::normalize_api_account_connection_input;
    use super::normalize_usage_error_message;
    use super::refresh_usage_worker_count_from_resources;
    use super::should_suspend_auth_keepalive;
    use super::upsert_prepared_import;
    use super::PreparedImport;
    use super::AUTH_EXPIRED_NOTICE;
    use super::KEEPALIVE_LAST_REFRESH_BASE_AGE_SECS;
    use super::KEEPALIVE_LAST_REFRESH_JITTER_SECS;
    use super::USAGE_AUTH_TOKEN_EXPIRED_NOTICE;
    use crate::models::AccountsStore;
    use crate::models::StoredAccount;
    use crate::models::TestApiAccountConnectionInput;
    use crate::models::UsageSnapshot;
    use crate::models::UsageWindow;
    use serde_json::json;

    fn test_api_input(
        base_url: &str,
        api_key: &str,
        model_name: &str,
    ) -> TestApiAccountConnectionInput {
        TestApiAccountConnectionInput {
            label: "Test API".to_string(),
            base_url: base_url.to_string(),
            api_key: api_key.to_string(),
            model_name: model_name.to_string(),
        }
    }

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

    #[test]
    fn test_api_account_connection_normalizes_valid_input() {
        let normalized = normalize_api_account_connection_input(&test_api_input(
            " https://example.com/v1/// ",
            " provider-token-123 ",
            " gpt-test ",
        ))
        .expect("valid input should normalize");

        assert_eq!(normalized.base_url, "https://example.com/v1");
        assert_eq!(normalized.api_key, "provider-token-123");
        assert_eq!(normalized.model_name, "gpt-test");
    }

    #[test]
    fn test_api_account_connection_rejects_invalid_base_url() {
        let error = normalize_api_account_connection_input(&test_api_input(
            "example.com/v1",
            "sk-test",
            "gpt-test",
        ))
        .expect_err("base url without scheme should fail");

        assert!(error.contains("http/https"));
    }

    #[test]
    fn test_api_account_connection_rejects_empty_key_or_model() {
        let key_error = normalize_api_account_connection_input(&test_api_input(
            "https://example.com/v1",
            " ",
            "gpt-test",
        ))
        .expect_err("empty key should fail");
        let model_error = normalize_api_account_connection_input(&test_api_input(
            "https://example.com/v1",
            "sk-test",
            " ",
        ))
        .expect_err("empty model should fail");

        assert!(key_error.contains("API Key"));
        assert!(model_error.contains("模型"));
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
    fn expand_import_json_content_normalizes_chatgpt_session_json() {
        let raw = json!({
            "WARNING_BANNER": "do not share",
            "user": {
                "id": "user-1",
                "name": "Session User",
                "email": "session@example.com"
            },
            "account": {
                "id": "account-1",
                "planType": "pro"
            },
            "accessToken": "header.eyJleHAiOjQxMDI0NDQ4MDB9.signature",
            "authProvider": "openai",
            "sessionToken": "session-1"
        })
        .to_string();

        let candidates = expand_import_json_content(&raw, "session.json", None).unwrap();

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].source, "session.json");
        let tokens = candidates[0]
            .auth_json
            .get("tokens")
            .and_then(serde_json::Value::as_object)
            .expect("session import should produce token object");
        assert_eq!(
            tokens.get("account_id").and_then(serde_json::Value::as_str),
            Some("account-1")
        );
        assert_eq!(
            tokens
                .get("session_token")
                .and_then(serde_json::Value::as_str),
            Some("session-1")
        );
    }

    #[test]
    fn upsert_prepared_import_reuses_unknown_variant_placeholder() {
        let mut store = AccountsStore::default();
        store.accounts.push(StoredAccount {
            id: "existing".to_string(),
            label: "placeholder".to_string(),
            source_kind: Default::default(),
            principal_id: Some("fresh@example.com".to_string()),
            email: Some("fresh@example.com".to_string()),
            account_id: "account-1".to_string(),
            plan_type: None,
            auth_json: json!({ "kind": "old" }),
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
            added_at: 1,
            updated_at: 1,
            usage: None,
            usage_error: None,
            auth_refresh_blocked: false,
            auth_refresh_error: None,
            api_proxy_enabled: true,
            codex_keepalive_last_at: None,
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

    #[test]
    fn keepalive_refresh_age_is_stable_and_staggered() {
        let first = keepalive_max_last_refresh_age_secs("account-a");
        let second = keepalive_max_last_refresh_age_secs("account-b");

        assert_eq!(first, keepalive_max_last_refresh_age_secs("account-a"));
        assert!(first >= KEEPALIVE_LAST_REFRESH_BASE_AGE_SECS);
        assert!(first < KEEPALIVE_LAST_REFRESH_BASE_AGE_SECS + KEEPALIVE_LAST_REFRESH_JITTER_SECS);
        assert!(second >= KEEPALIVE_LAST_REFRESH_BASE_AGE_SECS);
        assert!(second < KEEPALIVE_LAST_REFRESH_BASE_AGE_SECS + KEEPALIVE_LAST_REFRESH_JITTER_SECS);
    }

    #[test]
    fn refresh_usage_worker_count_uses_target_cpu_and_memory_limits() {
        assert_eq!(
            refresh_usage_worker_count_from_resources(0, 8, Some(4096)),
            1
        );
        assert_eq!(
            refresh_usage_worker_count_from_resources(1, 8, Some(4096)),
            1
        );
        assert_eq!(
            refresh_usage_worker_count_from_resources(100, 4, Some(4096)),
            8
        );
        assert_eq!(
            refresh_usage_worker_count_from_resources(100, 16, Some(300)),
            1
        );
        assert_eq!(
            refresh_usage_worker_count_from_resources(1000, 64, Some(1024 * 1024)),
            128
        );
        assert_eq!(
            refresh_usage_worker_count_from_resources(10, 64, Some(1024 * 1024)),
            10
        );
    }

    #[test]
    fn stale_refresh_snapshot_errors_are_recoverable() {
        assert!(is_stale_refresh_snapshot_error(
            r#"刷新登录令牌失败 -> 401: {"code":"refresh_token_reused"}"#
        ));
        assert!(is_stale_refresh_snapshot_error(
            r#"刷新登录令牌失败 -> 400: {"error":"invalid_grant"}"#
        ));
        assert!(!is_stale_refresh_snapshot_error(
            "请求用量接口失败: 500 Internal Server Error"
        ));
    }

    #[test]
    fn refresh_failures_suspend_keepalive_for_invalid_grant_refresh_token() {
        let error = r#"刷新登录令牌失败 https://auth.openai.com/oauth/token -> 400 Bad Request: {"error":"invalid_grant","error_description":"Refresh token expired"}"#;

        assert!(should_suspend_auth_keepalive(error));
        assert_eq!(normalize_usage_error_message(error), AUTH_EXPIRED_NOTICE);
    }

    #[test]
    fn refresh_failures_suspend_keepalive_for_revoked_refresh_token() {
        let error = "invalid refresh token: refresh token revoked";

        assert!(should_suspend_auth_keepalive(error));
        assert_eq!(normalize_usage_error_message(error), AUTH_EXPIRED_NOTICE);
    }

    #[test]
    fn access_token_expired_usage_errors_do_not_suspend_keepalive() {
        let error = "请求用量接口失败: 401 Unauthorized: provided authentication token is expired";

        assert!(!should_suspend_auth_keepalive(error));
        assert_eq!(
            normalize_usage_error_message(error),
            USAGE_AUTH_TOKEN_EXPIRED_NOTICE
        );
    }
}
