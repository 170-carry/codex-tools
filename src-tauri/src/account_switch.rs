//! Persist account switches with profile/store rollback.
use crate::models::{AccountsStore, StoredAccount};
use crate::state::AppState;
use crate::{app_paths, capture_current_auth_for_active_profile, profile_files, store};
use tauri::AppHandle;

pub(crate) async fn apply_target_profile(
    app: &AppHandle,
    state: &AppState,
    id: &str,
    account: StoredAccount,
    refreshed_auth_updated_at: Option<i64>,
) -> Result<StoredAccount, String> {
    let _guard = state.store_lock.lock().await;
    let mut latest_store = store::load_store(app)?;
    let store_path = store::account_store_path_from_data_dir(&app_paths::app_data_dir(app)?);
    let previous_active_id = latest_store
        .settings
        .active_account_id
        .clone()
        .filter(|active_id| active_id != id);
    if let Some(active_id) = previous_active_id.as_deref() {
        capture_current_auth_for_active_profile(&store_path, &mut latest_store)?;
        // 先保存当前账号在 Codex 内产生的配置改动，再应用目标 profile。
        profile_files::capture_current_config_for_profile(&store_path, active_id)?;
    }
    let target_account = {
        let stored_account = latest_store
            .accounts
            .iter_mut()
            .find(|stored| stored.id == id)
            .ok_or_else(|| "找不到要切换的账号".to_string())?;
        if let Some(refreshed_at) = refreshed_auth_updated_at {
            stored_account.auth_json = account.auth_json.clone();
            stored_account.updated_at = refreshed_at;
            stored_account.auth_refresh_blocked = false;
            stored_account.auth_refresh_error = None;
        }
        profile_files::sync_account_profile_in_store_path(&store_path, stored_account)?;
        stored_account.clone()
    };

    if let Err(apply_error) = profile_files::apply_account_profile(&target_account) {
        let rollback_result =
            rollback_previous_account_profile(&latest_store, previous_active_id.as_deref());
        return Err(match rollback_result {
            Err(rollback_error) => format!(
                "应用目标账号配置失败: {apply_error}；恢复原账号配置也失败: {rollback_error}"
            ),
            Ok(true) => {
                format!("应用目标账号配置失败: {apply_error}；已恢复原账号配置")
            }
            Ok(false) => format!("应用目标账号配置失败: {apply_error}"),
        });
    }

    latest_store.settings.active_account_id = Some(target_account.id.clone());
    let account = target_account;
    if let Err(save_error) = store::save_store(app, &latest_store) {
        if previous_active_id.is_none() {
            match store::save_store(app, &latest_store) {
                Ok(()) => {
                    log::warn!("保存首次活动账号记录第一次失败，重试已成功: {save_error}");
                }
                Err(retry_error) => {
                    return Err(format!(
                                "保存首次活动账号记录失败: {save_error}；重试也失败: {retry_error}。目标账号配置已应用，但活动账号记录可能未持久化"
                            ));
                }
            }
        } else {
            let profile_rollback =
                rollback_previous_account_profile(&latest_store, previous_active_id.as_deref());
            latest_store.settings.active_account_id = previous_active_id.clone();
            let store_rollback = store::save_store(app, &latest_store);

            let rollback_detail = match (profile_rollback, store_rollback) {
                (Ok(true), Ok(())) => "已恢复原账号配置和活动账号记录".to_string(),
                (profile_result, store_result) => format!(
                    "回滚结果：账号配置={}；活动账号记录={}",
                    describe_switch_rollback_result(profile_result),
                    describe_switch_store_rollback_result(store_result)
                ),
            };
            return Err(format!(
                "保存活动账号记录失败: {save_error}；{rollback_detail}"
            ));
        }
    }
    Ok(account)
}

fn rollback_previous_account_profile(
    store: &AccountsStore,
    previous_active_id: Option<&str>,
) -> Result<bool, String> {
    let Some(previous_active_id) = previous_active_id else {
        return Ok(false);
    };
    let previous_account = store
        .accounts
        .iter()
        .find(|stored| stored.id == previous_active_id)
        .ok_or_else(|| "找不到原账号 profile".to_string())?;
    profile_files::apply_account_profile(previous_account)?;
    Ok(true)
}

fn describe_switch_rollback_result(result: Result<bool, String>) -> String {
    match result {
        Ok(true) => "已恢复".to_string(),
        Ok(false) => "没有可恢复的原账号".to_string(),
        Err(error) => format!("失败（{error}）"),
    }
}

fn describe_switch_store_rollback_result(result: Result<(), String>) -> String {
    match result {
        Ok(()) => "已恢复".to_string(),
        Err(error) => format!("失败（{error}）"),
    }
}
