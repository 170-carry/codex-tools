use std::collections::HashMap;

use tauri::AppHandle;

use crate::auth::current_auth_account_id;
use crate::auth::extract_auth;
use crate::auth::read_current_codex_auth;
use crate::models::AccountSummary;
use crate::models::StoredAccount;
use crate::state::AppState;
use crate::store::load_store;
use crate::store::save_store;
use crate::usage::fetch_usage_snapshot;
use crate::utils::now_unix_seconds;
use crate::utils::short_account;

pub(crate) async fn list_accounts_internal(
    app: &AppHandle,
    state: &AppState,
) -> Result<Vec<AccountSummary>, String> {
    let _guard = state.store_lock.lock().await;
    let store = load_store(app)?;
    let current_account_id = current_auth_account_id();
    Ok(store
        .accounts
        .iter()
        .map(|account| account.to_summary(current_account_id.as_deref()))
        .collect())
}

pub(crate) async fn import_current_auth_account_internal(
    app: &AppHandle,
    state: &AppState,
    label: Option<String>,
) -> Result<AccountSummary, String> {
    let auth_json = read_current_codex_auth()?;
    let extracted = extract_auth(&auth_json)?;

    // 用量拉取失败不阻断导入流程，避免账号无法入库。
    let usage = fetch_usage_snapshot(&extracted.access_token, &extracted.account_id)
        .await
        .ok();

    let mut _guard = state.store_lock.lock().await;
    let mut store = load_store(app)?;

    let now = now_unix_seconds();
    let fallback_label = extracted
        .email
        .clone()
        .unwrap_or_else(|| format!("Codex {}", short_account(&extracted.account_id)));
    let new_label = label
        .and_then(|value| {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        })
        .unwrap_or(fallback_label);

    let summary = if let Some(existing) = store
        .accounts
        .iter_mut()
        .find(|account| account.account_id == extracted.account_id)
    {
        existing.label = new_label;
        existing.email = extracted.email;
        existing.plan_type = usage
            .as_ref()
            .and_then(|snapshot| snapshot.plan_type.clone())
            .or(extracted.plan_type)
            .or(existing.plan_type.clone());
        existing.auth_json = auth_json;
        existing.updated_at = now;
        existing.usage = usage;
        existing.usage_error = None;
        existing.to_summary(current_auth_account_id().as_deref())
    } else {
        let stored = StoredAccount {
            id: uuid::Uuid::new_v4().to_string(),
            label: new_label,
            email: extracted.email,
            account_id: extracted.account_id,
            plan_type: usage
                .as_ref()
                .and_then(|snapshot| snapshot.plan_type.clone())
                .or(extracted.plan_type),
            auth_json,
            added_at: now,
            updated_at: now,
            usage,
            usage_error: None,
        };
        let summary = stored.to_summary(current_auth_account_id().as_deref());
        store.accounts.push(stored);
        summary
    };

    save_store(app, &store)?;
    Ok(summary)
}

pub(crate) async fn delete_account_internal(
    app: &AppHandle,
    state: &AppState,
    id: &str,
) -> Result<(), String> {
    let mut _guard = state.store_lock.lock().await;
    let mut store = load_store(app)?;
    let original_len = store.accounts.len();
    store.accounts.retain(|account| account.id != id);

    if original_len == store.accounts.len() {
        return Err("未找到要删除的账号".to_string());
    }

    save_store(app, &store)?;
    Ok(())
}

/// 拉取并刷新所有账号用量，返回可直接用于前端/状态栏显示的摘要。
///
/// 为避免“后台刷新覆盖新增账号”的竞态：
/// 1) 先拿快照用于网络请求；
/// 2) 请求完成后重新加载最新 store 并按 account_id 合并写回。
pub(crate) async fn refresh_all_usage_internal(
    app: &AppHandle,
    state: &AppState,
) -> Result<Vec<AccountSummary>, String> {
    let refresh_targets: Vec<(String, serde_json::Value)> = {
        let _guard = state.store_lock.lock().await;
        let store = load_store(app)?;
        store
            .accounts
            .into_iter()
            .map(|account| (account.account_id, account.auth_json))
            .collect()
    };

    #[derive(Debug)]
    struct RefreshOutcome {
        usage: Option<crate::models::UsageSnapshot>,
        usage_error: Option<String>,
        updated_at: i64,
    }

    let mut outcomes: HashMap<String, RefreshOutcome> = HashMap::new();
    for (account_id, auth_json) in refresh_targets {
        let fetch_result = match extract_auth(&auth_json) {
            Ok(auth) => fetch_usage_snapshot(&auth.access_token, &auth.account_id).await,
            Err(err) => Err(err),
        };

        let updated_at = now_unix_seconds();
        match fetch_result {
            Ok(snapshot) => {
                outcomes.insert(
                    account_id,
                    RefreshOutcome {
                        usage: Some(snapshot),
                        usage_error: None,
                        updated_at,
                    },
                );
            }
            Err(err) => {
                outcomes.insert(
                    account_id,
                    RefreshOutcome {
                        usage: None,
                        usage_error: Some(err),
                        updated_at,
                    },
                );
            }
        }
    }

    let store = {
        let _guard = state.store_lock.lock().await;
        let mut latest_store = load_store(app)?;

        for account in &mut latest_store.accounts {
            let Some(outcome) = outcomes.get(&account.account_id) else {
                continue;
            };

            account.updated_at = outcome.updated_at;
            if let Some(snapshot) = outcome.usage.clone() {
                account.plan_type = snapshot.plan_type.clone().or(account.plan_type.clone());
                account.usage = Some(snapshot);
                account.usage_error = None;
            } else if let Some(err) = outcome.usage_error.clone() {
                account.usage_error = Some(err);
            }
        }

        save_store(app, &latest_store)?;
        latest_store
    };

    // 与当前 auth 文件重新对齐，确保 current 标签准确。
    let current_account_id = current_auth_account_id();
    let summaries: Vec<AccountSummary> = store
        .accounts
        .iter()
        .map(|account| account.to_summary(current_account_id.as_deref()))
        .collect();

    Ok(summaries)
}
