use crate::models::{EditorAppId, SwitchAccountResult};
use crate::{account_switch, project_switch, state::AppState, tray};
use tauri::AppHandle;

#[allow(clippy::too_many_arguments)]
pub(crate) async fn switch(
    app: &AppHandle,
    state: &AppState,
    paths: &project_switch::SwitchPaths,
    id: &str,
    workspace: Option<&str>,
    launch: bool,
    accept: bool,
    fingerprint: Option<String>,
    restart_editors_override: Option<bool>,
    restart_targets_override: Option<Vec<EditorAppId>>,
) -> Result<SwitchAccountResult, String> {
    let _auth = state.auth_operation_lock.lock().await;
    crate::ensure_no_pending_auth_operation(state).await?;
    let store_guard = state.store_lock.lock().await;
    let (mut prepared, mut lifecycle, settings) = {
        let settings = project_switch::strict_store(&paths.store_path)?.settings;
        let mut lifecycle = project_switch::NativeLifecycle::new(
            paths,
            settings.codex_launch_path.as_deref(),
            settings.launch_codex_as_admin,
            launch,
        )?;
        let prepared = project_switch::execute_with_refresh(
            paths,
            id,
            project_switch::SwitchOptions {
                accept_merge: accept,
                preview_fingerprint: fingerprint,
                target_auth: None,
            },
            &mut lifecycle,
        )
        .await?;
        (prepared, lifecycle, settings)
    };
    let mut result = crate::noop_switch_account_result(&prepared.account);
    result.no_op = prepared.result.status == "noOp";
    if !result.no_op {
        let companions = account_switch::run_companions(
            &prepared.account,
            settings.sync_opencode_openai_auth,
            settings.sync_opencode_openai_auth && settings.restart_opencode_desktop_on_switch,
            restart_editors_override.unwrap_or(settings.restart_editors_on_switch),
            &restart_targets_override.unwrap_or_else(|| settings.restart_editor_targets.clone()),
            &mut account_switch::NativeCompanionActions,
        );
        result.opencode_synced = companions.opencode_synced;
        result.opencode_sync_error = companions.opencode_sync_error;
        result.opencode_desktop_restarted = companions.opencode_desktop_restarted;
        result.opencode_desktop_restart_error = companions.opencode_desktop_restart_error;
        result.restarted_editor_apps = companions.restarted_editor_apps;
        result.editor_restart_error = companions.editor_restart_error;
    }
    if launch && !result.no_op {
        match lifecycle.launch(paths, workspace) {
            Ok((path, fallback)) => {
                result.launched_app_path = path;
                result.used_fallback_cli = fallback;
                project_switch::verify_after_launch(paths, &mut prepared).await;
            }
            Err(_) => {
                prepared.result.status = "pending".into();
                prepared.result.warning = Some("PROJECT_SHARING_LAUNCH_FAILED".into());
            }
        }
    }
    result.local_project_preservation = Some(prepared.result.clone());
    drop(prepared);
    drop(store_guard);
    let _ = tray::refresh_usage_surfaces_snapshot(app);
    Ok(result)
}
