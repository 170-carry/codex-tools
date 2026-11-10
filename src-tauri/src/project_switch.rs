//! Opt-in project-preserving account switching with explicit input paths.
use crate::{
    desktop_project_state::{self as engine, SessionSnapshot},
    models::{AccountsStore, StoredAccount},
    store_lock,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub(crate) struct SwitchPaths {
    pub codex_home: PathBuf,
    pub store_path: PathBuf,
}
impl SwitchPaths {
    pub(crate) fn new(codex_home: PathBuf, store_path: PathBuf) -> Result<Self, String> {
        if !codex_home.is_absolute() || !store_path.is_absolute() {
            return Err("PROJECT_SHARING_INVALID_STATE".into());
        }
        Ok(Self {
            codex_home,
            store_path,
        })
    }
    fn host(&self) -> String {
        format!("local:{}", self.codex_home.display())
    }
    fn global(&self) -> PathBuf {
        self.codex_home.join(".codex-global-state.json")
    }
    fn registry(&self) -> PathBuf {
        self.codex_home.join(".codex-tools-project-sharing.json")
    }
    fn journal(&self) -> PathBuf {
        self.codex_home.join(".codex-tools-project-switch")
    }
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProjectSwitchPreview {
    pub project_count: usize,
    pub thread_link_count: usize,
    pub unassigned_thread_count: usize,
    pub retained_target_project_count: usize,
    pub requires_merge_confirmation: bool,
    pub preview_fingerprint: String,
    pub no_op: bool,
}
pub(crate) use crate::models::PreservationResult;
#[derive(Default)]
pub(crate) struct SwitchOptions {
    pub accept_merge: bool,
    pub preview_fingerprint: Option<String>,
    pub target_auth: Option<Value>,
}
pub(crate) trait DesktopLifecycle {
    fn stop_and_verify(&mut self) -> Result<(), String>;
    fn assert_stopped(&mut self) -> Result<(), String>;
}
pub(crate) struct PreparedSwitch {
    pub account: StoredAccount,
    pub result: PreservationResult,
    target_owner: String,
    expected: SessionSnapshot,
    _locks: Option<persistence::Locks>,
}
pub(crate) fn strict_store(path: &Path) -> Result<AccountsStore, String> {
    let bytes = store_lock::read_optional(path)?.ok_or("PROJECT_SHARING_INVALID_STATE")?;
    let store: AccountsStore =
        serde_json::from_slice(&bytes).map_err(|_| "PROJECT_SHARING_INVALID_STATE".to_string())?;
    store.loaded_revision.set(Some(store_lock::digest(&bytes)));
    Ok(store)
}
mod inputs;
mod lifecycle;
mod persistence;
mod session_files;
pub(crate) use lifecycle::NativeLifecycle;

struct Plan {
    store: AccountsStore,
    account: StoredAccount,
    source_owner: String,
    target_owner: String,
    source_id: Option<String>,
    auth: Value,
    config: String,
    sharing: engine::SessionPlan,
    preview: ProjectSwitchPreview,
    inputs: inputs::PlanInputs,
}
fn state_error(error: engine::StateError) -> String {
    log::warn!("PROJECT_SHARING_INVALID_STATE reason={error}");
    "PROJECT_SHARING_INVALID_STATE".into()
}
fn json_file(path: &Path) -> Result<Value, String> {
    serde_json::from_slice(
        &store_lock::read_optional(path)?.ok_or("PROJECT_SHARING_INVALID_STATE")?,
    )
    .map_err(|_| "PROJECT_SHARING_INVALID_STATE".into())
}
fn config(bytes: &[u8]) -> Result<String, String> {
    let text = std::str::from_utf8(bytes).map_err(|_| "PROJECT_SHARING_INVALID_STATE")?;
    let mut doc = text
        .parse::<toml_edit::DocumentMut>()
        .map_err(|_| "PROJECT_SHARING_INVALID_STATE")?;
    if doc
        .get("model_provider")
        .is_some_and(|v| v.as_str() != Some("openai"))
        || doc.get("openai_base_url").is_some()
        || doc.get("chatgpt_base_url").is_some()
        || doc
            .get("model_providers")
            .and_then(|providers| providers.get("openai"))
            .is_some()
        || doc.get("profile").is_some()
    {
        return Err("PROJECT_SHARING_UNSUPPORTED_ROUTE".into());
    }
    doc["model_provider"] = toml_edit::value("openai");
    doc["cli_auth_credentials_store"] = toml_edit::value("file");
    Ok(doc.to_string())
}
fn make_plan(
    paths: &SwitchPaths,
    target: &str,
    auth_override: Option<&Value>,
) -> Result<Plan, String> {
    make_plan_with_reader(paths, target, auth_override, &mut store_lock::read_optional)
}
fn make_plan_with_reader(
    paths: &SwitchPaths,
    target: &str,
    auth_override: Option<&Value>,
    read: &mut impl FnMut(&Path) -> Result<Option<Vec<u8>>, String>,
) -> Result<Plan, String> {
    store_lock::ensure_store_available(&paths.store_path)?;
    if paths.journal().exists() {
        return Err("PROJECT_SHARING_RECOVERY_REQUIRED".into());
    }
    let mut inputs = inputs::PlanInputs::capture(
        [
            paths.store_path.clone(),
            paths.codex_home.join("auth.json"),
            paths.codex_home.join("config.toml"),
            paths.global(),
        ],
        read,
    )?;
    let store = inputs.store(&paths.store_path)?;
    let mut account = store
        .accounts
        .iter()
        .find(|a| a.id == target)
        .cloned()
        .ok_or("PROJECT_SHARING_INVALID_STATE")?;
    if !persistence::safe_id(&account.id)
        || !matches!(
            account.source_kind,
            crate::models::AccountSourceKind::Chatgpt
        )
    {
        return Err("PROJECT_SHARING_UNSUPPORTED_IDENTITY".into());
    }
    let target_owner = engine::owner_from_auth(&account.auth_json)
        .map_err(|_| "PROJECT_SHARING_UNSUPPORTED_IDENTITY")?;
    if let Some(auth) = auth_override {
        if engine::owner_from_auth(auth).map_err(state_error)? != target_owner {
            return Err("PROJECT_SHARING_IDENTITY_CHANGED".into());
        }
        account.auth_json = auth.clone();
    }
    let auth = inputs.json(&paths.codex_home.join("auth.json"))?;
    let source_owner =
        engine::owner_from_auth(&auth).map_err(|_| "PROJECT_SHARING_UNSUPPORTED_IDENTITY")?;
    let candidates = store
        .accounts
        .iter()
        .filter(|a| {
            persistence::safe_id(&a.id)
                && matches!(a.source_kind, crate::models::AccountSourceKind::Chatgpt)
                && engine::owner_from_auth(&a.auth_json).ok().as_deref()
                    == Some(source_owner.as_str())
        })
        .collect::<Vec<_>>();
    let source_id = candidates
        .iter()
        .find(|a| store.settings.active_account_id.as_deref() == Some(a.id.as_str()))
        .map(|a| a.id.clone())
        .or_else(|| {
            if candidates.len() == 1 {
                Some(candidates[0].id.clone())
            } else {
                None
            }
        });
    persistence::target(paths, &format!("profile/{target}/auth.json"))?;
    if let Some(id) = &source_id {
        persistence::target(paths, &format!("profile/{id}/auth.json"))?;
    }
    let config = config(
        inputs
            .optional(&paths.codex_home.join("config.toml"))
            .unwrap_or_default(),
    )?;
    let document = if inputs.optional(&paths.global()).is_some() {
        inputs.json(&paths.global())?
    } else {
        serde_json::json!({})
    };
    let sharing = engine::plan_session_overwrite(&document, &source_owner, &target_owner)
        .map_err(state_error)?;
    let (project_count, thread_link_count, unassigned_thread_count) = sharing.source.counts();
    let fingerprint = store_lock::digest(
        &serde_json::to_vec(&serde_json::json!([source_owner, target_owner, config]))
            .map_err(|_| "PROJECT_SHARING_INVALID_STATE")?,
    );
    let preview = ProjectSwitchPreview {
        project_count,
        thread_link_count,
        unassigned_thread_count,
        retained_target_project_count: 0,
        requires_merge_confirmation: false,
        preview_fingerprint: fingerprint,
        no_op: sharing.no_op,
    };
    for key in [
        format!("profile/{target}/auth.json"),
        format!("profile/{target}/config.toml"),
        format!("profile/{target}/desktop-state.json"),
        format!("profile/{target}/desktop-state.previous.json"),
    ] {
        let path = persistence::target(paths, &key)?;
        inputs.track(path, read)?;
    }
    // The desktop keeps updating globals while a read-only preview is built.
    // Replan after it exits, then compare the captured bytes before writing.
    Ok(Plan {
        store,
        account,
        source_owner,
        target_owner,
        source_id,
        auth,
        config,
        sharing,
        preview,
        inputs,
    })
}
pub(crate) fn preview(paths: &SwitchPaths, target: &str) -> Result<ProjectSwitchPreview, String> {
    Ok(make_plan(paths, target, None)?.preview)
}
fn prepared(plan: &Plan, status: &str) -> PreparedSwitch {
    PreparedSwitch {
        account: plan.account.clone(),
        result: PreservationResult {
            status: status.into(),
            project_count: plan.preview.project_count,
            thread_link_count: plan.preview.thread_link_count,
            unassigned_thread_count: plan.preview.unassigned_thread_count,
            warning: None,
        },
        target_owner: plan.target_owner.clone(),
        expected: plan.sharing.source.clone(),
        _locks: None,
    }
}
fn confirmed(plan: &Plan, options: &SwitchOptions) -> Result<(), String> {
    if options
        .preview_fingerprint
        .as_ref()
        .is_some_and(|f| f != &plan.preview.preview_fingerprint)
    {
        return Err("PROJECT_SHARING_PREVIEW_CHANGED".into());
    }
    if plan.preview.requires_merge_confirmation
        && (!options.accept_merge
            || options.preview_fingerprint.as_ref() != Some(&plan.preview.preview_fingerprint))
    {
        return Err("PROJECT_SHARING_CONFIRMATION_REQUIRED".into());
    }
    Ok(())
}
pub(crate) fn execute(
    paths: &SwitchPaths,
    target: &str,
    options: SwitchOptions,
    lifecycle: &mut impl DesktopLifecycle,
) -> Result<PreparedSwitch, String> {
    let initial = make_plan(paths, target, None)?;
    if initial.preview.no_op {
        return Ok(prepared(&initial, "noOp"));
    }
    let (mut plan, locks) = begin_stopped(paths, target, &options, lifecycle, &initial)?;
    if let Some(auth) = options.target_auth.as_ref() {
        persist_refreshed_target(paths, &plan, auth, lifecycle)?;
        plan = make_plan(paths, target, None)?;
        confirmed(&plan, &options)?;
    }
    apply_stopped_plan(paths, target, plan, lifecycle, locks)
}
fn begin_stopped(
    paths: &SwitchPaths,
    target: &str,
    options: &SwitchOptions,
    lifecycle: &mut impl DesktopLifecycle,
    initial: &Plan,
) -> Result<(Plan, persistence::Locks), String> {
    confirmed(initial, options)?;
    let locks = persistence::locks(paths)?;
    lifecycle.stop_and_verify()?;
    let plan = make_plan(paths, target, None)?;
    confirmed(&plan, options)?;
    if plan.source_owner != initial.source_owner || plan.target_owner != initial.target_owner {
        return Err("PROJECT_SHARING_IDENTITY_CHANGED".into());
    }
    Ok((plan, locks))
}
fn persist_refreshed_target(
    paths: &SwitchPaths,
    plan: &Plan,
    auth: &Value,
    lifecycle: &mut impl DesktopLifecycle,
) -> Result<(), String> {
    if engine::owner_from_auth(auth).map_err(state_error)? != plan.target_owner {
        return Err("PROJECT_SHARING_IDENTITY_CHANGED".into());
    }
    let mut store = plan.store.clone();
    let target = store
        .accounts
        .iter_mut()
        .find(|a| a.id == plan.account.id)
        .ok_or("PROJECT_SHARING_INVALID_STATE")?;
    target.auth_json = auth.clone();
    target.updated_at = crate::utils::now_unix_seconds();
    target.auth_refresh_blocked = false;
    target.auth_refresh_error = None;
    let key = format!("profile/{}/auth.json", target.id);
    target.profile_auth_path = Some(
        persistence::target(paths, &key)?
            .to_string_lossy()
            .into_owned(),
    );
    target.profile_auth_ready = true;
    let mut changes = vec![persistence::change(
        paths,
        key,
        serde_json::to_vec_pretty(auth).map_err(|_| "PROJECT_SHARING_INVALID_STATE")?,
    )?];
    let serialized =
        serde_json::to_vec_pretty(&store).map_err(|_| "PROJECT_SHARING_INVALID_STATE")?;
    if let Some(old) = store_lock::read_optional(
        &paths
            .store_path
            .with_file_name("accounts.json.last-good.json"),
    )? {
        changes.push(persistence::change(paths, "prev-good", old)?);
    }
    changes.push(persistence::change(paths, "last-good", serialized.clone())?);
    changes.push(persistence::change(paths, "store", serialized)?);
    for change in &mut changes {
        let path = persistence::target(paths, &change.key)?;
        if let Some(before) = plan.inputs.before(&path) {
            change.before = before.clone();
        }
    }
    persistence::apply_refreshed_credentials(paths, changes, lifecycle)
}
fn apply_stopped_plan(
    paths: &SwitchPaths,
    target: &str,
    mut plan: Plan,
    lifecycle: &mut impl DesktopLifecycle,
    locks: persistence::Locks,
) -> Result<PreparedSwitch, String> {
    let mut changes = session_files::checkpoint_changes(paths, target, &plan)?;
    // Capture the exact final active credentials/config only after desktop exit.
    if let Some(id) = plan.source_id.clone() {
        let source = plan
            .store
            .accounts
            .iter_mut()
            .find(|a| a.id == id)
            .ok_or("PROJECT_SHARING_INVALID_STATE")?;
        source.auth_json = plan.auth.clone();
        source.profile_auth_path = Some(
            persistence::target(paths, &format!("profile/{id}/auth.json"))?
                .to_string_lossy()
                .into_owned(),
        );
        source.profile_config_path = Some(
            persistence::target(paths, &format!("profile/{id}/config.toml"))?
                .to_string_lossy()
                .into_owned(),
        );
        source.profile_auth_ready = true;
        source.profile_config_ready = plan
            .inputs
            .optional(&paths.codex_home.join("config.toml"))
            .is_some();
        source.profile_integrity_error = None;
        changes.push(persistence::change(
            paths,
            format!("profile/{id}/auth.json"),
            serde_json::to_vec_pretty(&plan.auth).map_err(|_| "PROJECT_SHARING_INVALID_STATE")?,
        )?);
        if let Some(config) = plan.inputs.optional(&paths.codex_home.join("config.toml")) {
            changes.push(persistence::change(
                paths,
                format!("profile/{id}/config.toml"),
                config.to_vec(),
            )?);
        }
    }
    let auth = serde_json::to_vec_pretty(&plan.account.auth_json)
        .map_err(|_| "PROJECT_SHARING_INVALID_STATE")?;
    changes.push(persistence::change(
        paths,
        format!("profile/{target}/auth.json"),
        auth.clone(),
    )?);
    changes.push(persistence::change(
        paths,
        format!("profile/{target}/config.toml"),
        plan.config.as_bytes().to_vec(),
    )?);
    plan.account.profile_auth_path = Some(
        persistence::target(paths, &format!("profile/{target}/auth.json"))?
            .to_string_lossy()
            .into_owned(),
    );
    plan.account.profile_config_path = Some(
        persistence::target(paths, &format!("profile/{target}/config.toml"))?
            .to_string_lossy()
            .into_owned(),
    );
    plan.account.profile_auth_ready = true;
    plan.account.profile_config_ready = true;
    plan.account.profile_integrity_error = None;
    plan.account.auth_refresh_blocked = false;
    plan.account.auth_refresh_error = None;
    *plan
        .store
        .accounts
        .iter_mut()
        .find(|a| a.id == target)
        .unwrap() = plan.account.clone();
    plan.store.settings.active_account_id = Some(target.into());
    let global = serde_json::to_vec_pretty(&plan.sharing.document)
        .map_err(|_| "PROJECT_SHARING_INVALID_STATE")?;
    changes.push(persistence::change(paths, "global", global.clone())?);
    changes.push(persistence::change(paths, "global-backup", global)?);
    changes.push(persistence::change(
        paths,
        "config",
        plan.config.as_bytes().to_vec(),
    )?);
    changes.push(persistence::change(paths, "auth", auth)?);
    let serialized =
        serde_json::to_vec_pretty(&plan.store).map_err(|_| "PROJECT_SHARING_INVALID_STATE")?;
    let old_latest = store_lock::read_optional(
        &paths
            .store_path
            .with_file_name("accounts.json.last-good.json"),
    )?;
    if let Some(previous) = old_latest {
        changes.push(persistence::change(paths, "prev-good", previous)?);
    }
    changes.push(persistence::change(paths, "last-good", serialized.clone())?);
    changes.push(persistence::change(paths, "store", serialized)?);
    if store_lock::fingerprint(&paths.store_path)? != plan.store.loaded_revision.get() {
        return Err("PROJECT_SHARING_STALE_STORE".into());
    }
    plan.inputs.ensure_unchanged()?;
    persistence::apply(paths, changes, lifecycle)?;
    let mut result = prepared(&plan, "prepared");
    result._locks = Some(locks);
    Ok(result)
}
pub(crate) fn recover(
    paths: &SwitchPaths,
    lifecycle: &mut impl DesktopLifecycle,
) -> Result<bool, String> {
    let _locks = persistence::locks(paths)?;
    lifecycle.stop_and_verify()?;
    persistence::recover_locked(paths, lifecycle)
}
pub(crate) async fn verify_after_launch(paths: &SwitchPaths, prepared: &mut PreparedSwitch) {
    if prepared.result.status == "noOp" {
        return;
    }
    for _ in 0..20 {
        if json_file(&paths.codex_home.join("auth.json"))
            .ok()
            .and_then(|auth| engine::owner_from_auth(&auth).ok())
            .as_deref()
            == Some(prepared.target_owner.as_str())
            && json_file(&paths.global())
                .ok()
                .is_some_and(|doc| prepared.expected.is_restored(&doc, &prepared.target_owner))
        {
            prepared.result.status = "verified".into();
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }
    prepared.result.status = "pending".into();
    prepared.result.warning = Some("PROJECT_SHARING_RESTORE_PENDING".into());
}
#[cfg(test)]
mod tests;

pub(crate) fn enabled_at_path(path: &Path) -> Result<bool, String> {
    Ok(store_lock::read_optional(path)?
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .and_then(|v| v.get("settings")?.get("preserveLocalProjects")?.as_bool())
        .unwrap_or(true))
}
pub(crate) async fn execute_with_refresh(
    paths: &SwitchPaths,
    target: &str,
    options: SwitchOptions,
    lifecycle: &mut impl DesktopLifecycle,
) -> Result<PreparedSwitch, String> {
    execute_with_injected_refresh(paths, target, options, lifecycle, |auth| async move {
        crate::auth::refresh_chatgpt_auth_tokens(&auth)
            .await
            .map_err(|_| "PROJECT_SHARING_AUTH_REFRESH_FAILED".into())
    })
    .await
}
async fn execute_with_injected_refresh<F, Fut>(
    paths: &SwitchPaths,
    target: &str,
    options: SwitchOptions,
    lifecycle: &mut impl DesktopLifecycle,
    refresh: F,
) -> Result<PreparedSwitch, String>
where
    F: FnOnce(Value) -> Fut,
    Fut: std::future::Future<Output = Result<Value, String>>,
{
    let initial = make_plan(paths, target, None)?;
    if initial.preview.no_op {
        return Ok(prepared(&initial, "noOp"));
    }
    let (mut plan, locks) = begin_stopped(paths, target, &options, lifecycle, &initial)?;
    if crate::auth::auth_tokens_need_refresh(&plan.account.auth_json) {
        if plan.account.auth_refresh_blocked {
            return Err("PROJECT_SHARING_AUTH_REFRESH_FAILED".into());
        }
        lifecycle.assert_stopped()?;
        let auth = refresh(plan.account.auth_json.clone()).await?;
        persist_refreshed_target(paths, &plan, &auth, lifecycle)?;
        plan = make_plan(paths, target, None)?;
        confirmed(&plan, &options)?;
        if plan.source_owner != initial.source_owner || plan.target_owner != initial.target_owner {
            return Err("PROJECT_SHARING_IDENTITY_CHANGED".into());
        }
    }
    apply_stopped_plan(paths, target, plan, lifecycle, locks)
}

/// Legacy switches cooperate with preserving switches without changing their
/// existing profile/provider behavior. An unfinished journal must be recovered.
pub(crate) fn legacy_switch_lock(paths: &SwitchPaths) -> Result<store_lock::FileLock, String> {
    store_lock::ensure_store_available(&paths.store_path)?;
    if paths.journal().exists() {
        return Err("PROJECT_SHARING_RECOVERY_REQUIRED".into());
    }
    if !paths.codex_home.exists() {
        store_lock::private_dir(&paths.codex_home)?;
    }
    store_lock::FileLock::acquire(&paths.codex_home.join(".codex-tools-project-switch.lock"))
}
