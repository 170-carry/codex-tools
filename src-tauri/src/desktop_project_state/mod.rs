//! Pure planning for sharing local Codex projects between account-scoped states.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

mod identity;
mod merge;
mod snapshot;

pub use identity::owner_from_auth;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateError(pub String);

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LocalSnapshot {
    pub projects: BTreeMap<String, Value>,
    pub thread_memberships: BTreeMap<String, ThreadMembership>,
    pub project_order: Vec<String>,
    pub pinned_project_ids: BTreeSet<String>,
    pub project_appearances: BTreeMap<String, Value>,
    pub sidebar_project_thread_orders: BTreeMap<String, Vec<String>>,
    pub app_server_project_ids: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ThreadMembership {
    pub project_id: Option<String>,
    pub host_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SharingRegistry {
    pub schema_version: u32,
    pub host_id: String,
    pub canonical: LocalSnapshot,
    pub baselines: BTreeMap<String, LocalSnapshot>,
    pub deletions: BTreeMap<String, BTreeSet<String>>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SharingSummary {
    pub no_op: bool,
    pub added_project_ids: Vec<String>,
    pub updated_project_ids: Vec<String>,
    pub deleted_project_ids: Vec<String>,
    pub changed_thread_ids: Vec<String>,
    pub retained_target_project_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SharingPlan {
    pub document: Value,
    pub registry: SharingRegistry,
    pub summary: SharingSummary,
}

impl std::fmt::Display for StateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for StateError {}

fn invalid(message: impl Into<String>) -> StateError {
    StateError(message.into())
}

/// Plans A -> B while A is still active. The caller must persist both returned
/// values transactionally before changing auth, with Codex fully stopped.
/// This function performs no filesystem, database, process or auth mutation.
pub fn plan_share(
    document: &Value,
    source_owner: &str,
    target_owner: &str,
    host_id: &str,
    registry: Option<&SharingRegistry>,
) -> Result<SharingPlan, StateError> {
    identity::validate_owner(source_owner)?;
    identity::validate_owner(target_owner)?;
    identity::validate_host(host_id)?;
    let state = account_state(document)?;
    let active_owner = state["owner"].as_str().unwrap();
    if active_owner != source_owner {
        return Err(invalid(
            "source identity does not own active renderer globals",
        ));
    }
    let inactive = state["inactive"].as_object().unwrap();
    let mut registry = registry.cloned().unwrap_or_else(|| SharingRegistry {
        schema_version: 1,
        host_id: host_id.into(),
        canonical: LocalSnapshot::default(),
        baselines: BTreeMap::new(),
        deletions: BTreeMap::new(),
    });
    validate_registry(&registry, host_id)?;
    if source_owner == target_owner {
        return Ok(SharingPlan {
            document: document.clone(),
            registry,
            summary: SharingSummary {
                no_op: true,
                ..SharingSummary::default()
            },
        });
    }
    let empty = serde_json::json!({});
    let target = inactive
        .get(target_owner)
        .map(|entry| &entry["globals"])
        .unwrap_or(&empty);
    let mut known_projects = BTreeSet::new();
    let mut known_threads = BTreeSet::new();
    for snapshot in std::iter::once(&registry.canonical).chain(registry.baselines.values()) {
        known_projects.extend(snapshot.projects.keys().cloned());
        known_threads.extend(snapshot.thread_memberships.keys().cloned());
    }
    known_projects.extend(
        registry
            .deletions
            .get("projects")
            .into_iter()
            .flatten()
            .cloned(),
    );
    known_threads.extend(
        registry
            .deletions
            .get("threads")
            .into_iter()
            .flatten()
            .cloned(),
    );
    snapshot::validate_managed_thread_scope(document, &known_threads)?;
    snapshot::validate_managed_thread_scope(target, &known_threads)?;
    let source = snapshot::read(document, host_id, &known_threads, &known_projects)?;
    let target_snapshot = snapshot::read(target, host_id, &known_threads, &known_projects)?;
    known_projects.extend(
        source
            .projects
            .keys()
            .chain(target_snapshot.projects.keys())
            .cloned(),
    );
    known_threads.extend(
        source
            .thread_memberships
            .keys()
            .chain(target_snapshot.thread_memberships.keys())
            .cloned(),
    );
    // Importing a project must not revive an unresolved relationship, and an
    // unrelated orphan must not be overwritten by the other account's thread.
    for (globals, owner) in [(document, source_owner), (target, target_owner)] {
        snapshot::validate_unresolved_assignment_scope(
            globals,
            registry.baselines.get(owner),
            &known_projects,
            &known_threads,
        )?;
    }
    let source_base = registry.baselines.get(source_owner).cloned();
    let target_base = registry.baselines.get(target_owner).cloned();
    merge::absorb(&mut registry, &source, source_base.as_ref())?;
    merge::absorb(&mut registry, &target_snapshot, target_base.as_ref())?;
    snapshot::cleanup(&mut registry.canonical);
    snapshot::validate_snapshot(&registry.canonical)?;
    known_projects.extend(registry.canonical.projects.keys().cloned());
    known_threads.extend(registry.canonical.thread_memberships.keys().cloned());
    let applied = snapshot::apply(
        target,
        &registry.canonical,
        host_id,
        &known_projects,
        &known_threads,
    )?;
    let applied_snapshot = snapshot::read(&applied, host_id, &known_threads, &known_projects)?;
    let summary = summarize(&source, &target_snapshot, &applied_snapshot);
    // Baseline A is exactly what was observed, not the canonical union. Only B
    // receives that union now; marking it applied to A would fabricate deletions
    // when the desktop later archives A's still unchanged active globals.
    registry.baselines.insert(source_owner.to_owned(), source);
    registry
        .baselines
        .insert(target_owner.to_owned(), applied_snapshot);
    let mut result = document.clone();
    let entries = result[snapshot::STATE]["inactive"].as_object_mut().unwrap();
    let entry = entries
        .entry(target_owner.to_owned())
        .or_insert_with(|| serde_json::json!({"atoms":{},"globals":{}}));
    entry["globals"] = applied;
    Ok(SharingPlan {
        document: result,
        registry,
        summary,
    })
}

/// Verifies the owner and exact local project/membership projection after the
/// desktop restores an account. Cosmetic UI state and timestamp refreshes are
/// deliberately outside this acceptance check. This function performs no I/O.
pub fn verify_restored(
    document: &Value,
    target_owner: &str,
    host_id: &str,
    expected: &LocalSnapshot,
) -> Result<(), StateError> {
    identity::validate_owner(target_owner)?;
    identity::validate_host(host_id)?;
    let state = account_state(document)?;
    if state["owner"] != target_owner {
        return Err(invalid(
            "restored renderer owner does not match target identity",
        ));
    }
    snapshot::validate_snapshot(expected)?;
    let known_projects = expected.projects.keys().cloned().collect();
    let known_threads = expected.thread_memberships.keys().cloned().collect();
    let actual = snapshot::read_core(document, host_id, &known_threads, &known_projects)?;
    if !actual.projects.keys().eq(expected.projects.keys()) {
        return Err(invalid(
            "restored local project IDs do not match the sharing plan",
        ));
    }
    for (id, project) in &expected.projects {
        if ["id", "name", "rootPaths"]
            .iter()
            .any(|field| actual.projects[id][*field] != project[*field])
        {
            return Err(invalid(format!(
                "restored local project differs from the sharing plan: {id}"
            )));
        }
    }
    if actual.thread_memberships != expected.thread_memberships {
        return Err(invalid("restored local thread memberships or explicit unassignments differ from the sharing plan"));
    }
    Ok(())
}

fn account_state(document: &Value) -> Result<&serde_json::Map<String, Value>, StateError> {
    let root = document
        .as_object()
        .ok_or_else(|| invalid("global state must be an object"))?;
    if root
        .keys()
        .any(|key| key.starts_with("electron-account-renderer-state-") && key != snapshot::STATE)
    {
        return Err(invalid("unsupported account renderer state version"));
    }
    let state = document
        .get(snapshot::STATE)
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("missing account renderer state v1"))?;
    let active_owner = state
        .get("owner")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("missing active account owner"))?;
    identity::validate_owner(active_owner)?;
    let inactive = state
        .get("inactive")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("inactive account state must be an object"))?;
    for (owner, entry) in inactive {
        identity::validate_owner(owner)?;
        if !entry.is_object()
            || !entry.get("atoms").is_some_and(Value::is_object)
            || !entry.get("globals").is_some_and(Value::is_object)
        {
            return Err(invalid("invalid inactive account envelope"));
        }
    }
    Ok(state)
}

fn validate_registry(registry: &SharingRegistry, host: &str) -> Result<(), StateError> {
    if registry.schema_version != 1 {
        return Err(invalid("unsupported project sharing registry version"));
    }
    if registry.host_id != host {
        return Err(invalid(
            "project sharing registry belongs to a different local host",
        ));
    }
    snapshot::validate_snapshot(&registry.canonical)?;
    for (owner, snapshot) in &registry.baselines {
        identity::validate_owner(owner)?;
        snapshot::validate_snapshot(snapshot)?;
    }
    for (kind, ids) in &registry.deletions {
        if !matches!(kind.as_str(), "projects" | "threads") || ids.iter().any(|id| id.is_empty()) {
            return Err(invalid("invalid registry deletion records"));
        }
        if ids.iter().any(|id| {
            if kind == "projects" {
                registry.canonical.projects.contains_key(id)
            } else {
                registry.canonical.thread_memberships.contains_key(id)
            }
        }) {
            return Err(invalid("registry contains an active tombstoned record"));
        }
    }
    Ok(())
}

fn summarize(
    source: &LocalSnapshot,
    before: &LocalSnapshot,
    after: &LocalSnapshot,
) -> SharingSummary {
    SharingSummary {
        no_op: false,
        added_project_ids: after
            .projects
            .keys()
            .filter(|id| !before.projects.contains_key(*id))
            .cloned()
            .collect(),
        updated_project_ids: after
            .projects
            .iter()
            .filter(|(id, value)| before.projects.get(*id).is_some_and(|old| old != *value))
            .map(|(id, _)| id.clone())
            .collect(),
        deleted_project_ids: before
            .projects
            .keys()
            .filter(|id| !after.projects.contains_key(*id))
            .cloned()
            .collect(),
        changed_thread_ids: before
            .thread_memberships
            .keys()
            .chain(after.thread_memberships.keys())
            .cloned()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .filter(|id| before.thread_memberships.get(id) != after.thread_memberships.get(id))
            .collect(),
        retained_target_project_ids: before
            .projects
            .keys()
            .filter(|id| !source.projects.contains_key(*id) && after.projects.contains_key(*id))
            .cloned()
            .collect(),
    }
}

#[cfg(test)]
mod tests;
