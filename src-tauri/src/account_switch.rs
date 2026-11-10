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
    let store_write_guard = store::begin_store_write(&store_path, &latest_store)?;
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
    if let Err(save_error) =
        store::save_store_with_guard(&store_path, &latest_store, &store_write_guard)
    {
        if previous_active_id.is_none() {
            match store::save_store_with_guard(&store_path, &latest_store, &store_write_guard) {
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
            let store_rollback =
                store::save_store_with_guard(&store_path, &latest_store, &store_write_guard);

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

#[derive(Default)]
pub(crate) struct CompanionResult {
    pub opencode_synced: bool,
    pub opencode_sync_error: Option<String>,
    pub opencode_desktop_restarted: bool,
    pub opencode_desktop_restart_error: Option<String>,
    pub restarted_editor_apps: Vec<crate::models::EditorAppId>,
    pub editor_restart_error: Option<String>,
}
pub(crate) trait CompanionActions {
    fn sync_opencode(&mut self, auth: &serde_json::Value) -> Result<(), String>;
    fn restart_opencode(&mut self) -> Result<(), String>;
    fn restart_editors(
        &mut self,
        targets: &[crate::models::EditorAppId],
    ) -> (Vec<crate::models::EditorAppId>, Option<String>);
}
pub(crate) struct NativeCompanionActions;
impl CompanionActions for NativeCompanionActions {
    fn sync_opencode(&mut self, auth: &serde_json::Value) -> Result<(), String> {
        crate::opencode::sync_openai_auth_from_codex_auth(auth)
    }
    fn restart_opencode(&mut self) -> Result<(), String> {
        crate::opencode::restart_opencode_desktop_app()
    }
    fn restart_editors(
        &mut self,
        targets: &[crate::models::EditorAppId],
    ) -> (Vec<crate::models::EditorAppId>, Option<String>) {
        crate::editor_apps::restart_selected_editor_apps(targets)
    }
}
pub(crate) fn run_companions(
    account: &StoredAccount,
    sync: bool,
    restart_opencode: bool,
    restart_editors: bool,
    targets: &[crate::models::EditorAppId],
    actions: &mut impl CompanionActions,
) -> CompanionResult {
    let mut result = CompanionResult::default();
    if sync {
        let synced = if matches!(
            account.source_kind,
            crate::models::AccountSourceKind::Chatgpt
        ) {
            actions.sync_opencode(&account.auth_json)
        } else {
            Err("当前条目为 API 中转站配置，无法同步为 opencode 的 OAuth 登录态。".to_string())
        };
        match synced {
            Ok(()) => {
                result.opencode_synced = true;
                if restart_opencode {
                    match actions.restart_opencode() {
                        Ok(()) => result.opencode_desktop_restarted = true,
                        Err(error) => {
                            log::warn!("重启 opencode 桌面端失败: {error}");
                            result.opencode_desktop_restart_error = Some(error);
                        }
                    }
                }
            }
            Err(error) => {
                log::warn!("同步 opencode OpenAI 认证失败: {error}");
                result.opencode_sync_error = Some(error);
            }
        }
    }
    if restart_editors {
        (result.restarted_editor_apps, result.editor_restart_error) =
            actions.restart_editors(targets);
    }
    result
}
#[cfg(test)]
mod companion_tests {
    use super::*;
    #[derive(Default)]
    struct Fake {
        calls: Vec<&'static str>,
        fail_sync: bool,
    }
    impl CompanionActions for Fake {
        fn sync_opencode(&mut self, _: &serde_json::Value) -> Result<(), String> {
            self.calls.push("sync");
            if self.fail_sync {
                Err("synthetic failure".into())
            } else {
                Ok(())
            }
        }
        fn restart_opencode(&mut self) -> Result<(), String> {
            self.calls.push("opencode");
            Ok(())
        }
        fn restart_editors(
            &mut self,
            targets: &[crate::models::EditorAppId],
        ) -> (Vec<crate::models::EditorAppId>, Option<String>) {
            self.calls.push("editors");
            (targets.to_vec(), None)
        }
    }
    fn account() -> StoredAccount {
        serde_json::from_value(serde_json::json!({"id":"synthetic","label":"Synthetic","accountId":"synthetic","authJson":{},"addedAt":1,"updatedAt":1})).unwrap()
    }
    #[test]
    fn configured_companions_run_in_existing_order() {
        let mut fake = Fake::default();
        let out = run_companions(&account(), true, true, true, &[], &mut fake);
        assert_eq!(fake.calls, vec!["sync", "opencode", "editors"]);
        assert!(out.opencode_synced && out.opencode_desktop_restarted);
    }
    #[test]
    fn failed_sync_never_restarts_opencode_but_editor_request_survives() {
        let mut fake = Fake {
            fail_sync: true,
            ..Default::default()
        };
        let out = run_companions(&account(), true, true, true, &[], &mut fake);
        assert_eq!(fake.calls, vec!["sync", "editors"]);
        assert!(out.opencode_sync_error.is_some());
    }
    #[test]
    fn disabled_companions_have_no_side_effects() {
        let mut fake = Fake::default();
        run_companions(&account(), false, true, false, &[], &mut fake);
        assert!(fake.calls.is_empty());
    }
}
