//! Projection and surgical patching of the supported renderer globals.
use super::{invalid, LocalSnapshot, StateError, ThreadMembership};
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;

pub(super) const STATE: &str = "electron-account-renderer-state-v1";
const PROJECTS: &str = "local-projects";
const ASSIGNMENTS: &str = "thread-project-assignments";
const HOSTS: &str = "thread-project-membership-host-ids";
const UNASSIGNED: &str = "projectless-thread-ids";
const ORDER: &str = "project-order";
const PINNED: &str = "pinned-project-ids";
const APPEARANCES: &str = "project-appearances";
const THREAD_ORDER: &str = "sidebar-project-thread-orders";
const MAPPINGS: &str = "app-server-project-id-by-legacy-project-id-by-host";
const PENDING: &str = "app-server-pending-project-deletions-by-host";

fn nonempty(value: &Value, field: &str) -> Result<String, StateError> {
    value
        .as_str()
        .filter(|s| !s.trim().is_empty())
        .map(str::to_owned)
        .ok_or_else(|| invalid(format!("{field} must be a nonempty string")))
}

fn object<'a>(value: &'a Value, field: &str) -> Result<&'a Map<String, Value>, StateError> {
    value
        .as_object()
        .ok_or_else(|| invalid(format!("{field} must be an object")))
}

fn map(globals: &Value, key: &str) -> Result<Map<String, Value>, StateError> {
    match globals.get(key) {
        Some(value) => Ok(object(value, key)?.clone()),
        None => Ok(Map::new()),
    }
}

fn strings(value: &Value, key: &str) -> Result<Vec<String>, StateError> {
    let values = value
        .as_array()
        .ok_or_else(|| invalid(format!("{key} must be an array")))?;
    let mut result = Vec::with_capacity(values.len());
    let mut seen = BTreeSet::new();
    for value in values {
        let value = nonempty(value, key)?;
        if !seen.insert(value.clone()) {
            return Err(invalid(format!("duplicate ID in {key}")));
        }
        result.push(value);
    }
    Ok(result)
}

fn list(globals: &Value, key: &str) -> Result<Vec<String>, StateError> {
    globals
        .get(key)
        .map(|value| strings(value, key))
        .transpose()
        .map(Option::unwrap_or_default)
}

pub(super) fn pure_project(id: &str, value: &Value) -> Result<Option<Value>, StateError> {
    if id.is_empty() {
        return Err(invalid("empty local project ID"));
    }
    let project = object(value, PROJECTS)?;
    if nonempty(project.get("id").unwrap_or(&Value::Null), "project.id")? != id {
        return Err(invalid("project ID does not match its map key"));
    }
    if project.iter().any(|(key, value)| {
        (key.starts_with("chatGptBacking") || key.starts_with("retiredChatGptBacking"))
            && !value.is_null()
    }) {
        return Ok(None);
    }
    if project
        .get("projectKind")
        .is_some_and(|kind| kind != "local")
    {
        return Ok(None);
    }
    if !project.get("name").is_some_and(Value::is_string) {
        return Err(invalid("project.name must be a string"));
    }
    strings(
        project.get("rootPaths").unwrap_or(&Value::Null),
        "project.rootPaths",
    )?;
    for field in ["createdAt", "updatedAt"] {
        if !project
            .get(field)
            .and_then(Value::as_f64)
            .is_some_and(|value| value.is_finite() && value >= 0.0)
        {
            return Err(invalid(format!(
                "project.{field} must be a nonnegative timestamp number"
            )));
        }
    }
    let safe = ["id", "name", "rootPaths", "createdAt", "updatedAt"]
        .into_iter()
        .map(|key| (key.to_owned(), project[key].clone()))
        .collect();
    Ok(Some(Value::Object(safe)))
}

/// A historical shared relationship entering a protected scope is a conflict,
/// not a deletion. Check both participants before projection can filter it out.
pub(super) fn validate_managed_thread_scope(
    globals: &Value,
    managed_threads: &BTreeSet<String>,
) -> Result<(), StateError> {
    let projects = map(globals, PROJECTS)?;
    let assignments = map(globals, ASSIGNMENTS)?;
    let hosts = map(globals, HOSTS)?;
    for thread in managed_threads {
        if let Some(host) = hosts.get(thread) {
            if nonempty(host, HOSTS)? != "local" {
                return Err(invalid(
                    "historically shared thread now belongs to a nonlocal host",
                ));
            }
        }
        if let Some(assignment) = assignments.get(thread) {
            let assignment = object(assignment, ASSIGNMENTS)?;
            let kind = nonempty(
                assignment.get("projectKind").unwrap_or(&Value::Null),
                "assignment.projectKind",
            )?;
            let id = nonempty(
                assignment.get("projectId").unwrap_or(&Value::Null),
                "assignment.projectId",
            )?;
            let protected_project = projects
                .get(&id)
                .map(|project| pure_project(&id, project))
                .transpose()?
                .is_some_and(|project| project.is_none());
            if kind != "local" || protected_project {
                return Err(invalid(
                    "historically shared thread now has a protected project assignment",
                ));
            }
        }
    }
    Ok(())
}

/// Rechecks excluded references against the complete cross-account scope.
pub(super) fn validate_unresolved_assignment_scope(
    globals: &Value,
    owner_baseline: Option<&LocalSnapshot>,
    managed_projects: &BTreeSet<String>,
    managed_threads: &BTreeSet<String>,
    host: &str,
    deleted_projects: &BTreeSet<String>,
) -> Result<(), StateError> {
    let projects = map(globals, PROJECTS)?;
    // Old desktop versions leave project IDs in cosmetic metadata and server
    // mappings after removing their definitions. Keep those records with their
    // owner, but do not overwrite them by importing a colliding project ID.
    let mut metadata_ids: BTreeSet<_> = list(globals, ORDER)?
        .into_iter()
        .chain(list(globals, PINNED)?)
        .collect();
    for key in [APPEARANCES, THREAD_ORDER] {
        metadata_ids.extend(map(globals, key)?.into_iter().map(|(id, _)| id));
    }
    if let Some(mappings) = map(globals, MAPPINGS)?.get(host) {
        metadata_ids.extend(object(mappings, MAPPINGS)?.keys().cloned());
    }
    for id in metadata_ids {
        if !projects.contains_key(&id)
            && managed_projects.contains(&id)
            && !deleted_projects.contains(&id)
            && !owner_baseline.is_some_and(|base| base.projects.contains_key(&id))
        {
            return Err(invalid(
                "unresolved project metadata collides with the shared scope",
            ));
        }
    }
    for (thread, assignment) in map(globals, ASSIGNMENTS)? {
        let id = nonempty(&assignment["projectId"], "assignment.projectId")?;
        let previously_managed = owner_baseline.is_some_and(|baseline| {
            baseline.projects.contains_key(&id)
                && baseline
                    .thread_memberships
                    .get(&thread)
                    .is_some_and(|membership| membership.project_id.as_deref() == Some(id.as_str()))
        });
        if assignment["projectKind"] == "local"
            && !projects.contains_key(&id)
            && !previously_managed
            && (managed_projects.contains(&id) || managed_threads.contains(&thread))
        {
            return Err(invalid(
                "unresolved assignment collides with the shared scope",
            ));
        }
    }
    Ok(())
}

/// Validates known containers even when entries are excluded from sharing.
pub(super) fn read(
    globals: &Value,
    host: &str,
    known_threads: &BTreeSet<String>,
    known_projects: &BTreeSet<String>,
) -> Result<LocalSnapshot, StateError> {
    object(globals, "globals")?;
    let raw_projects = map(globals, PROJECTS)?;
    let mut result = LocalSnapshot::default();
    for (id, value) in &raw_projects {
        if let Some(project) = pure_project(id, value)? {
            result.projects.insert(id.clone(), project);
        }
    }
    let assignments = map(globals, ASSIGNMENTS)?;
    let hosts = map(globals, HOSTS)?;
    for (thread, host) in &hosts {
        if thread.is_empty() {
            return Err(invalid("empty thread ID"));
        }
        nonempty(host, HOSTS)?;
    }
    let unassigned: BTreeSet<_> = list(globals, UNASSIGNED)?.into_iter().collect();
    for (thread, value) in &assignments {
        if thread.is_empty() {
            return Err(invalid("empty thread ID"));
        }
        let assignment = object(value, ASSIGNMENTS)?;
        let kind = nonempty(
            assignment.get("projectKind").unwrap_or(&Value::Null),
            "assignment.projectKind",
        )?;
        let id = nonempty(
            assignment.get("projectId").unwrap_or(&Value::Null),
            "assignment.projectId",
        )?;
        if kind != "local" {
            continue;
        }
        if !raw_projects.contains_key(&id) && !known_projects.contains(&id) {
            if known_threads.contains(thread) {
                return Err(invalid(
                    "shared local assignment references an unknown project",
                ));
            }
            // Desktop state can retain links whose project is no longer in the
            // project map. They are outside our managed projection: leave them
            // untouched in their owner's globals, without inventing a project
            // or copying an unresolved relationship to another account.
            continue;
        }
        if !result.projects.contains_key(&id) {
            continue;
        }
        let host_id = hosts.get(thread).and_then(Value::as_str).map(str::to_owned);
        if host_id.as_deref().is_some_and(|host| host != "local") {
            continue;
        }
        if unassigned.contains(thread) {
            return Err(invalid("thread is both assigned and explicitly unassigned"));
        }
        result.thread_memberships.insert(
            thread.clone(),
            ThreadMembership {
                project_id: Some(id),
                host_id,
            },
        );
    }
    for thread in unassigned {
        let host_id = hosts
            .get(&thread)
            .and_then(Value::as_str)
            .map(str::to_owned);
        if host_id.as_deref().is_some_and(|host| host != "local") {
            continue;
        }
        if assignments.contains_key(&thread) {
            return Err(invalid("shared thread has a contradictory assignment"));
        }
        result.thread_memberships.insert(
            thread,
            ThreadMembership {
                project_id: None,
                host_id,
            },
        );
    }
    let order = list(globals, ORDER)?;
    let pinned = list(globals, PINNED)?;
    result.project_order = order
        .into_iter()
        .filter(|id| result.projects.contains_key(id))
        .collect();
    result.pinned_project_ids = pinned
        .into_iter()
        .filter(|id| result.projects.contains_key(id))
        .collect();
    for (id, value) in map(globals, APPEARANCES)? {
        object(&value, APPEARANCES)?;
        if result.projects.contains_key(&id) {
            result.project_appearances.insert(id, value);
        }
    }
    for (id, value) in map(globals, THREAD_ORDER)? {
        let entry = object(&value, THREAD_ORDER)?;
        let threads = strings(entry.get("threadIds").unwrap_or(&Value::Null), "threadIds")?;
        if result.projects.contains_key(&id) {
            let threads = threads
                .into_iter()
                .filter(|thread| {
                    result
                        .thread_memberships
                        .get(thread)
                        .is_some_and(|membership| membership.project_id.as_ref() == Some(&id))
                })
                .collect();
            result.sidebar_project_thread_orders.insert(id, threads);
        }
    }
    for (mapping_host, mappings) in map(globals, MAPPINGS)? {
        for (id, value) in object(&mappings, MAPPINGS)? {
            let server_id = nonempty(value, MAPPINGS)?;
            if mapping_host == host && result.projects.contains_key(id) {
                result.app_server_project_ids.insert(id.clone(), server_id);
            }
        }
    }
    for (pending_host, pending) in map(globals, PENDING)? {
        let pending = object(&pending, PENDING)?;
        if pending_host == host && pending.keys().any(|id| result.projects.contains_key(id)) {
            return Err(invalid("a shared local project has a pending deletion"));
        }
    }
    Ok(result)
}

/// Uses the same project and membership parser while excluding UI metadata
/// from post-launch verification. UI writes need not settle before acceptance.
pub(super) fn read_core(
    globals: &Value,
    host: &str,
    known_threads: &BTreeSet<String>,
    known_projects: &BTreeSet<String>,
) -> Result<LocalSnapshot, StateError> {
    let globals = object(globals, "globals")?;
    let core = [PROJECTS, ASSIGNMENTS, HOSTS, UNASSIGNED]
        .into_iter()
        .filter_map(|key| {
            globals
                .get(key)
                .map(|value| (key.to_owned(), value.clone()))
        })
        .collect();
    read(&Value::Object(core), host, known_threads, known_projects)
}

pub(super) fn validate_snapshot(snapshot: &LocalSnapshot) -> Result<(), StateError> {
    for (id, project) in &snapshot.projects {
        if pure_project(id, project)?.as_ref() != Some(project) {
            return Err(invalid("registry contains a non-whitelisted project"));
        }
    }
    for (thread, membership) in &snapshot.thread_memberships {
        if thread.is_empty()
            || membership
                .host_id
                .as_deref()
                .is_some_and(|host| host != "local")
            || membership
                .project_id
                .as_ref()
                .is_some_and(|id| !snapshot.projects.contains_key(id))
        {
            return Err(invalid("invalid registry thread membership"));
        }
    }
    let mut ordered = BTreeSet::new();
    for id in &snapshot.project_order {
        if !snapshot.projects.contains_key(id) || !ordered.insert(id) {
            return Err(invalid("invalid registry project order"));
        }
    }
    if snapshot
        .pinned_project_ids
        .iter()
        .any(|id| !snapshot.projects.contains_key(id))
        || snapshot
            .project_appearances
            .iter()
            .any(|(id, value)| !snapshot.projects.contains_key(id) || !value.is_object())
        || snapshot
            .app_server_project_ids
            .iter()
            .any(|(id, value)| !snapshot.projects.contains_key(id) || value.is_empty())
    {
        return Err(invalid("invalid registry project metadata"));
    }
    for (id, threads) in &snapshot.sidebar_project_thread_orders {
        let mut seen = BTreeSet::new();
        if !snapshot.projects.contains_key(id)
            || threads.iter().any(|thread| {
                !seen.insert(thread)
                    || !snapshot
                        .thread_memberships
                        .get(thread)
                        .is_some_and(|membership| membership.project_id.as_ref() == Some(id))
            })
        {
            return Err(invalid("invalid registry project thread order"));
        }
    }
    Ok(())
}

/// Rewrites managed records only, using the target's objects as the base so
/// unrelated metadata, cloud records and other hosts remain byte-value equal.
pub(super) fn apply(
    target: &Value,
    canonical: &LocalSnapshot,
    host: &str,
    managed_projects: &BTreeSet<String>,
    managed_threads: &BTreeSet<String>,
) -> Result<Value, StateError> {
    let mut result = target.clone();
    let mut projects = map(target, PROJECTS)?;
    let protected: BTreeSet<_> = projects
        .iter()
        .filter_map(|(id, value)| match pure_project(id, value) {
            Ok(None) => Some(id.clone()),
            _ => None,
        })
        .collect();
    if canonical.projects.keys().any(|id| protected.contains(id)) {
        return Err(invalid(
            "shared project ID collides with a cloud-backed or nonlocal project",
        ));
    }
    for id in managed_projects {
        if !protected.contains(id) {
            projects.remove(id);
        }
    }
    let original_projects = map(target, PROJECTS)?;
    for (id, project) in &canonical.projects {
        let mut entry = original_projects
            .get(id)
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        entry.extend(project.as_object().unwrap().clone());
        projects.insert(id.clone(), Value::Object(entry));
    }
    result[PROJECTS] = Value::Object(projects);
    let mut assignments = map(target, ASSIGNMENTS)?;
    let mut hosts = map(target, HOSTS)?;
    for (thread, assignment) in &assignments {
        let deletes_referenced_project = assignment["projectId"].as_str().is_some_and(|id| {
            original_projects.contains_key(id)
                && managed_projects.contains(id)
                && !protected.contains(id)
                && !canonical.projects.contains_key(id)
        });
        let protected_membership = assignment["projectKind"] != "local"
            || hosts.get(thread).is_some_and(|host| host != "local");
        if deletes_referenced_project && protected_membership {
            return Err(invalid(
                "cannot delete a shared project referenced by an unrelated target membership",
            ));
        }
    }
    let mut unassigned: BTreeSet<_> = list(target, UNASSIGNED)?.into_iter().collect();
    for thread in managed_threads {
        let protected_assignment = assignments.get(thread).is_some_and(|assignment| {
            assignment["projectKind"] != "local"
                || assignment["projectId"]
                    .as_str()
                    .is_some_and(|id| protected.contains(id) || !managed_projects.contains(id))
        });
        let protected_host = hosts.get(thread).is_some_and(|host| host != "local");
        if protected_assignment || protected_host {
            return Err(invalid(
                "shared thread collides with an unrelated target membership",
            ));
        }
        assignments.remove(thread);
        hosts.remove(thread);
        unassigned.remove(thread);
        if let Some(membership) = canonical.thread_memberships.get(thread) {
            if let Some(id) = &membership.project_id {
                let mut entry = target
                    .get(ASSIGNMENTS)
                    .and_then(|v| v.get(thread))
                    .and_then(Value::as_object)
                    .cloned()
                    .unwrap_or_default();
                entry.insert("projectKind".into(), json!("local"));
                entry.insert("projectId".into(), json!(id));
                assignments.insert(thread.clone(), Value::Object(entry));
            } else {
                unassigned.insert(thread.clone());
            }
            if let Some(host_id) = &membership.host_id {
                hosts.insert(thread.clone(), json!(host_id));
            }
        }
    }
    result[ASSIGNMENTS] = Value::Object(assignments);
    result[UNASSIGNED] = json!(unassigned);
    result[HOSTS] = Value::Object(hosts);
    let writable_projects: BTreeSet<_> = managed_projects.difference(&protected).cloned().collect();
    result[ORDER] = json!(combine_order(
        &list(target, ORDER)?,
        &canonical.project_order,
        &writable_projects
    ));
    let pins: Vec<_> = canonical.pinned_project_ids.iter().cloned().collect();
    result[PINNED] = json!(combine_order(
        &list(target, PINNED)?,
        &pins,
        &writable_projects
    ));
    let mut appearances = map(target, APPEARANCES)?;
    let mut thread_orders = map(target, THREAD_ORDER)?;
    for id in &writable_projects {
        appearances.remove(id);
        thread_orders.remove(id);
    }
    appearances.extend(canonical.project_appearances.clone());
    for id in canonical.projects.keys() {
        let existing = target.get(THREAD_ORDER).and_then(|value| value.get(id));
        let shared = canonical.sidebar_project_thread_orders.get(id);
        if existing.is_none() && shared.is_none() {
            continue;
        }
        let mut entry = existing
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        let existing_threads = entry
            .get("threadIds")
            .map(|value| strings(value, "threadIds"))
            .transpose()?
            .unwrap_or_default();
        let mut threads: Vec<_> = existing_threads
            .into_iter()
            .filter(|thread| !managed_threads.contains(thread))
            .collect();
        threads.extend(shared.into_iter().flatten().cloned());
        entry.insert("threadIds".into(), json!(threads));
        thread_orders.insert(id.clone(), Value::Object(entry));
    }
    result[APPEARANCES] = Value::Object(appearances);
    result[THREAD_ORDER] = Value::Object(thread_orders);
    let mut mappings = map(target, MAPPINGS)?;
    let mut local = mappings
        .get(host)
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    for id in &writable_projects {
        local.remove(id);
    }
    local.extend(
        canonical
            .app_server_project_ids
            .iter()
            .map(|(id, value)| (id.clone(), json!(value))),
    );
    if !local.is_empty() || mappings.contains_key(host) {
        mappings.insert(host.to_owned(), Value::Object(local));
    }
    result[MAPPINGS] = Value::Object(mappings);
    // A tombstoned project may also have an outstanding deletion. Those pending
    // flags are retained; selected canonical projects may never contradict them.
    for (pending_host, pending) in map(target, PENDING)? {
        if pending_host == host
            && object(&pending, PENDING)?
                .keys()
                .any(|id| canonical.projects.contains_key(id))
        {
            return Err(invalid(
                "target has a pending deletion for a shared project",
            ));
        }
    }
    Ok(result)
}

fn combine_order(
    existing: &[String],
    shared: &[String],
    managed: &BTreeSet<String>,
) -> Vec<String> {
    existing
        .iter()
        .filter(|id| !managed.contains(*id))
        .chain(shared)
        .cloned()
        .collect()
}

pub(super) fn cleanup(snapshot: &mut LocalSnapshot) {
    let projects = &snapshot.projects;
    for membership in snapshot.thread_memberships.values_mut() {
        if membership
            .project_id
            .as_ref()
            .is_some_and(|id| !projects.contains_key(id))
        {
            membership.project_id = None;
        }
    }
    snapshot
        .project_order
        .retain(|id| projects.contains_key(id));
    for id in projects.keys() {
        if !snapshot.project_order.contains(id) {
            snapshot.project_order.push(id.clone());
        }
    }
    snapshot
        .pinned_project_ids
        .retain(|id| projects.contains_key(id));
    snapshot
        .project_appearances
        .retain(|id, _| projects.contains_key(id));
    snapshot
        .app_server_project_ids
        .retain(|id, _| projects.contains_key(id));
    snapshot
        .sidebar_project_thread_orders
        .retain(|id, threads| {
            threads.retain(|thread| {
                snapshot
                    .thread_memberships
                    .get(thread)
                    .is_some_and(|membership| membership.project_id.as_ref() == Some(id))
            });
            projects.contains_key(id)
        });
}
