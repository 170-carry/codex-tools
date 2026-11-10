use super::{invalid, StateError};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;

const STATE: &str = "electron-account-renderer-state-v1";
const ATOMS: &str = "electron-persisted-atom-state";

// Account-scoped keys used by the desktop's selectRendererOwner contract.
const GLOBALS: &[&str] = &[
    "open-in-target-preferences",
    "worktree-repository-preferences",
    "desktop-app-beta-preferences-by-account-id",
    "active-workspace-roots",
    "electron-saved-workspace-roots",
    "electron-workspace-root-labels",
    "local-projects",
    "app-server-project-id-by-legacy-project-id-by-host",
    "app-server-projects-migration-by-host",
    "app-server-pending-project-deletions-by-host",
    "selected-project",
    "project-writable-roots",
    "project-appearances",
    "project-files",
    "pinned-thread-ids",
    "app-server-migrated-pinned-thread-ids-by-host",
    "pinned-project-ids",
    "sidebar-project-thread-orders",
    "sidebar-thread-metadata",
    "thread-project-assignments",
    "thread-project-membership-host-ids",
    "thread-writable-roots",
    "thread-workspace-root-hints",
    "thread-projectless-output-directories",
    "projectless-thread-ids",
    "selected-remote-host-id",
    "remote-projects",
    "active-remote-project-id",
    "project-order",
    "connection-group-order",
    "remote-cwds-by-host-and-workspace",
    "host-id-remote-control-allowed",
    "added-remote-control-env-ids",
    "remote-project-connection-backfill-completed",
    "remote-connection-auto-connect-by-host-id",
    "queued-follow-ups",
];
const SHARED_ATOMS: &[&str] = &[
    "last_completed_onboarding",
    "electron:onboarding-projectless-completed",
    "electron:bedrock-govcloud-guidance-acknowledged",
    "ctrl-tab-shortcut-notice-shown",
    "number-shortcut-notice-shown",
    "thread-sort-key",
];

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub(crate) struct SessionSnapshot {
    pub atoms: Map<String, Value>,
    pub globals: Map<String, Value>,
}

pub(crate) struct SessionPlan {
    pub document: Value,
    pub source: SessionSnapshot,
    pub previous_target: Option<Value>,
    pub no_op: bool,
}

fn object(value: Option<&Value>) -> Map<String, Value> {
    value
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default()
}

fn scoped_keys(inactive: &Map<String, Value>) -> BTreeSet<String> {
    GLOBALS
        .iter()
        .map(|key| (*key).to_owned())
        .chain(inactive.values().flat_map(|entry| {
            entry
                .get("globals")
                .and_then(Value::as_object)
                .into_iter()
                .flat_map(|globals| globals.keys().cloned())
        }))
        .collect()
}

pub(crate) fn plan_session_overwrite(
    document: &Value,
    source_owner: &str,
    target_owner: &str,
) -> Result<SessionPlan, StateError> {
    let root = document
        .as_object()
        .ok_or_else(|| invalid("global state must be an object"))?;
    let mut state = object(root.get(STATE));
    let mut inactive = object(state.get("inactive"));
    let keys = scoped_keys(&inactive);
    // Desktop startup/quit can archive the current owner and clear the active
    // globals. A nullable owner is part of its normal persisted format.
    let archived = state
        .get("owner")
        .map_or(true, Value::is_null)
        .then(|| inactive.get(source_owner))
        .flatten()
        .and_then(Value::as_object);
    let source = if let Some(archived) = archived {
        SessionSnapshot {
            atoms: object(archived.get("atoms")),
            globals: object(archived.get("globals")),
        }
    } else {
        SessionSnapshot {
            atoms: object(root.get(ATOMS)),
            globals: keys
                .iter()
                .filter_map(|key| root.get(key).map(|value| (key.clone(), value.clone())))
                .collect(),
        }
    };
    let mut source = source;
    for key in SHARED_ATOMS {
        source.atoms.remove(*key);
    }
    let no_op = source_owner == target_owner;
    let previous_target = inactive.get(target_owner).cloned();
    if no_op {
        return Ok(SessionPlan {
            document: document.clone(),
            source,
            previous_target,
            no_op,
        });
    }

    let mut result = root.clone();
    for key in keys {
        result.remove(&key);
    }
    result.extend(source.globals.clone());
    let mut atoms = source.atoms.clone();
    for key in SHARED_ATOMS {
        if let Some(value) = root.get(ATOMS).and_then(|atoms| atoms.get(*key)) {
            atoms.insert((*key).to_owned(), value.clone());
        }
    }
    result.insert(ATOMS.into(), Value::Object(atoms));
    let saved = json!({"atoms": source.atoms, "globals": source.globals});
    inactive.insert(source_owner.into(), saved.clone());
    inactive.insert(target_owner.into(), saved);
    state.insert("owner".into(), json!(source_owner));
    state.insert("inactive".into(), Value::Object(inactive));
    result.insert(STATE.into(), Value::Object(state));
    Ok(SessionPlan {
        document: Value::Object(result),
        source,
        previous_target,
        no_op,
    })
}

impl SessionSnapshot {
    pub(crate) fn counts(&self) -> (usize, usize, usize) {
        let projects = self
            .globals
            .get("local-projects")
            .and_then(Value::as_object);
        let assignments = self
            .globals
            .get("thread-project-assignments")
            .and_then(Value::as_object);
        let unassigned = self
            .globals
            .get("projectless-thread-ids")
            .and_then(Value::as_array);
        (
            projects.map_or(0, Map::len),
            assignments.map_or(0, Map::len),
            unassigned.map_or(0, Vec::len),
        )
    }

    pub(crate) fn is_restored(&self, document: &Value, target_owner: &str) -> bool {
        document
            .get(STATE)
            .and_then(|state| state.get("owner"))
            .and_then(Value::as_str)
            == Some(target_owner)
            && [
                "local-projects",
                "thread-project-assignments",
                "thread-project-membership-host-ids",
                "projectless-thread-ids",
            ]
            .iter()
            .all(|key| document.get(*key) == self.globals.get(*key))
            && [
                "composer-prompt-drafts-v2",
                "thread-workspace-draft-tabs-v1",
            ]
            .iter()
            .all(|key| {
                document.get(ATOMS).and_then(|atoms| atoms.get(*key)) == self.atoms.get(*key)
            })
    }
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;
