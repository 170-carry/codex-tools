//! Opt-in project-preserving account switching with explicit input paths.
use crate::{
    desktop_project_state::{self as engine, LocalSnapshot},
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
    expected: LocalSnapshot,
    _locks: Option<persistence::Locks>,
}
pub(crate) fn strict_store(path: &Path) -> Result<AccountsStore, String> {
    let bytes = store_lock::read_optional(path)?.ok_or("PROJECT_SHARING_INVALID_STATE")?;
    let store: AccountsStore =
        serde_json::from_slice(&bytes).map_err(|_| "PROJECT_SHARING_INVALID_STATE".to_string())?;
    store.loaded_revision.set(Some(store_lock::digest(&bytes)));
    Ok(store)
}
mod lifecycle;
mod persistence;
pub(crate) use lifecycle::NativeLifecycle;

struct Plan {
    store: AccountsStore,
    account: StoredAccount,
    source_owner: String,
    target_owner: String,
    source_id: Option<String>,
    auth: Value,
    config: String,
    sharing: engine::SharingPlan,
    preview: ProjectSwitchPreview,
    inputs: Vec<(PathBuf, Option<Vec<u8>>)>,
}
fn state_error(_: engine::StateError) -> String {
    "PROJECT_SHARING_INVALID_STATE".into()
}
fn json_file(path: &Path) -> Result<Value, String> {
    serde_json::from_slice(
        &store_lock::read_optional(path)?.ok_or("PROJECT_SHARING_INVALID_STATE")?,
    )
    .map_err(|_| "PROJECT_SHARING_INVALID_STATE".into())
}
fn config(paths: &SwitchPaths) -> Result<String, String> {
    let bytes =
        store_lock::read_optional(&paths.codex_home.join("config.toml"))?.unwrap_or_default();
    let text = String::from_utf8(bytes).map_err(|_| "PROJECT_SHARING_INVALID_STATE")?;
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
    store_lock::ensure_store_available(&paths.store_path)?;
    if paths.journal().exists() {
        return Err("PROJECT_SHARING_RECOVERY_REQUIRED".into());
    }
    let mut inputs = [
        paths.store_path.clone(),
        paths.codex_home.join("auth.json"),
        paths.codex_home.join("config.toml"),
        paths.global(),
        paths.registry(),
    ]
    .into_iter()
    .map(|path| store_lock::read_optional(&path).map(|bytes| (path, bytes)))
    .collect::<Result<Vec<_>, _>>()?;
    let store = strict_store(&paths.store_path)?;
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
    let auth = json_file(&paths.codex_home.join("auth.json"))?;
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
    let config = config(paths)?;
    let document = json_file(&paths.global())?;
    let registry: Option<engine::SharingRegistry> = store_lock::read_optional(&paths.registry())?
        .map(|b| {
            serde_json::from_slice(&b).map_err(|_| "PROJECT_SHARING_INVALID_STATE".to_string())
        })
        .transpose()?;
    let sharing = engine::plan_share(
        &document,
        &source_owner,
        &target_owner,
        &paths.host(),
        registry.as_ref(),
    )
    .map_err(state_error)?;
    validate_database(paths, &sharing.registry.canonical)?;
    let snapshot = &sharing.registry.canonical;
    let thread_link_count = snapshot
        .thread_memberships
        .values()
        .filter(|m| m.project_id.is_some())
        .count();
    let retained = sharing.summary.retained_target_project_ids.len();
    let target_globals =
        &document["electron-account-renderer-state-v1"]["inactive"][&target_owner]["globals"];
    let has_target_data = [
        "local-projects",
        "thread-project-assignments",
        "projectless-thread-ids",
    ]
    .iter()
    .any(|key| {
        target_globals.get(*key).is_some_and(|v| {
            v.as_object().is_some_and(|m| !m.is_empty())
                || v.as_array().is_some_and(|a| !a.is_empty())
        })
    });
    let requires = registry
        .as_ref()
        .map_or(true, |r| !r.baselines.contains_key(&target_owner))
        && has_target_data;
    let fingerprint = store_lock::digest(
        &serde_json::to_vec(&serde_json::json!([
            source_owner,
            target_owner,
            snapshot,
            retained,
            config
        ]))
        .map_err(|_| "PROJECT_SHARING_INVALID_STATE")?,
    );
    let preview = ProjectSwitchPreview {
        project_count: snapshot.projects.len(),
        thread_link_count,
        unassigned_thread_count: snapshot.thread_memberships.len() - thread_link_count,
        retained_target_project_count: retained,
        requires_merge_confirmation: requires,
        preview_fingerprint: fingerprint,
        no_op: sharing.summary.no_op,
    };
    for key in [
        format!("profile/{target}/auth.json"),
        format!("profile/{target}/config.toml"),
    ] {
        let path = persistence::target(paths, &key)?;
        inputs.push((path.clone(), store_lock::read_optional(&path)?));
    }
    check_inputs(&inputs)?;
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
fn check_inputs(inputs: &[(PathBuf, Option<Vec<u8>>)]) -> Result<(), String> {
    for (path, bytes) in inputs {
        if &store_lock::read_optional(path)? != bytes {
            return Err("PROJECT_SHARING_EXTERNAL_CHANGE".into());
        }
    }
    Ok(())
}
/// Owns only a synthetic validation copy; never opens the source with SQLite.
struct DatabaseSnapshot(PathBuf);
impl Drop for DatabaseSnapshot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
impl DatabaseSnapshot {
    fn capture(file: &Path) -> Result<Self, String> {
        let wal = file.with_file_name("state_5.sqlite-wal");
        let journal = file.with_file_name("state_5.sqlite-journal");
        for _ in 0..3 {
            if store_lock::read_optional(&journal)?.is_some_and(|b| !b.is_empty()) {
                return Err("PROJECT_SHARING_DATABASE_BUSY".into());
            }
            let main =
                store_lock::read_optional(file)?.ok_or("PROJECT_SHARING_DATABASE_MISMATCH")?;
            let wal_bytes = store_lock::read_optional(&wal)?;
            if store_lock::fingerprint(file)?.as_deref() != Some(store_lock::digest(&main).as_str())
                || store_lock::fingerprint(&wal)?
                    != wal_bytes.as_ref().map(|b| store_lock::digest(b))
            {
                continue;
            }
            let copy = Self(
                std::env::temp_dir()
                    .join(format!("codex-project-validation-{}", uuid::Uuid::new_v4())),
            );
            store_lock::private_dir(&copy.0)?;
            store_lock::atomic_write(&copy.0.join("state_5.sqlite"), &main)?;
            if let Some(bytes) = wal_bytes {
                store_lock::atomic_write(&copy.0.join("state_5.sqlite-wal"), &bytes)?;
            }
            return Ok(copy);
        }
        Err("PROJECT_SHARING_DATABASE_BUSY".into())
    }
}
fn validate_database(paths: &SwitchPaths, snapshot: &LocalSnapshot) -> Result<(), String> {
    use rusqlite::{Connection, OpenFlags, OptionalExtension};
    let file = paths.codex_home.join("state_5.sqlite");
    if snapshot.thread_memberships.is_empty() && snapshot.app_server_project_ids.is_empty() {
        return Ok(());
    }
    // Read source bytes only. SQLite may create SHM bookkeeping even with
    // READ_ONLY, so open a private consistent DB+WAL copy instead.
    let copy = DatabaseSnapshot::capture(&file)?;
    let db = Connection::open_with_flags(
        copy.0.join("state_5.sqlite"),
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|_| "PROJECT_SHARING_DATABASE_MISMATCH")?;
    db.pragma_update(None, "query_only", true)
        .map_err(|_| "PROJECT_SHARING_DATABASE_MISMATCH")?;
    let check = (|| -> rusqlite::Result<bool> {
        // The supported app-server schema always has metadata. Desktop startup
        // can rehydrate cloud bindings from it even when renderer JSON is stale.
        db.prepare("SELECT metadata FROM projects LIMIT 0")?;
        // Renderer relationships can outlive an indexed thread row. Keep the
        // saved assignment/unassignment: an index miss is not a user deletion,
        // and projecting metadata never creates or edits a conversation row.
        db.prepare("SELECT id FROM threads LIMIT 0")?;
        for (legacy, server) in &snapshot.app_server_project_ids {
            let Some((id, metadata)) = db
                .query_row(
                    "SELECT id, metadata FROM projects WHERE id=?1",
                    [server],
                    |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
                )
                .optional()?
            else {
                return Ok(false);
            };
            if &id != server {
                return Ok(false);
            }
            let Ok(Value::Object(metadata)) = serde_json::from_str::<Value>(&metadata) else {
                return Ok(false);
            };
            if [
                "codex.chatGptBacking",
                "codex.chatGptBackingCreation",
                "codex.retiredChatGptBacking",
            ]
            .iter()
            .any(|key| metadata.get(*key).is_some_and(|value| !value.is_null()))
                || metadata
                    .get("codex.localProjectId")
                    .is_some_and(|value| value.as_str() != Some(legacy.as_str()))
            {
                return Ok(false);
            }
            let mut stmt =
                db.prepare("SELECT path FROM project_roots WHERE project_id=?1 ORDER BY position")?;
            let roots = stmt
                .query_map([server], |r| r.get::<_, String>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            let expected: Vec<_> = snapshot.projects[legacy]["rootPaths"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap().to_owned())
                .collect();
            if roots != expected {
                return Ok(false);
            }
        }
        Ok(true)
    })();
    if check != Ok(true) {
        return Err("PROJECT_SHARING_DATABASE_MISMATCH".into());
    }
    Ok(())
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
        expected: plan.sharing.registry.canonical.clone(),
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
        if let Some((_, before)) = plan.inputs.iter().find(|(p, _)| p == &path) {
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
    let mut changes = Vec::new();
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
        source.profile_config_ready =
            store_lock::read_optional(&paths.codex_home.join("config.toml"))?.is_some();
        source.profile_integrity_error = None;
        changes.push(persistence::change(
            paths,
            format!("profile/{id}/auth.json"),
            serde_json::to_vec_pretty(&plan.auth).map_err(|_| "PROJECT_SHARING_INVALID_STATE")?,
        )?);
        if let Some(config) = store_lock::read_optional(&paths.codex_home.join("config.toml"))? {
            changes.push(persistence::change(
                paths,
                format!("profile/{id}/config.toml"),
                config,
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
        "registry",
        serde_json::to_vec_pretty(&plan.sharing.registry)
            .map_err(|_| "PROJECT_SHARING_INVALID_STATE")?,
    )?);
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
    check_inputs(&plan.inputs)?;
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
            && json_file(&paths.global()).ok().is_some_and(|doc| {
                engine::verify_restored(
                    &doc,
                    &prepared.target_owner,
                    &paths.host(),
                    &prepared.expected,
                )
                .is_ok()
            })
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
