//! Per-record three-way merge. Every caller works on a cloned registry: a
//! conflict discards the entire candidate instead of partially committing it.
use super::{invalid, LocalSnapshot, SharingRegistry, StateError};
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn absorb(
    registry: &mut SharingRegistry,
    current: &LocalSnapshot,
    baseline: Option<&LocalSnapshot>,
) -> Result<(), StateError> {
    let empty = LocalSnapshot::default();
    let base = baseline.unwrap_or(&empty);
    registry.canonical.project_order = effective_order(&registry.canonical);
    let canonical = &mut registry.canonical;
    merge_records(
        "projects",
        &mut canonical.projects,
        &current.projects,
        &base.projects,
        Some(registry.deletions.entry("projects".into()).or_default()),
    )?;
    // These tombstones describe removed relationships, not deleted thread
    // entities. An owner whose recorded baseline already lacked the relation
    // can make a fresh assignment; unseen/stale owners cannot restore it.
    if baseline.is_some() {
        let tombstones = registry.deletions.entry("threads".into()).or_default();
        for thread in current.thread_memberships.keys() {
            if !base.thread_memberships.contains_key(thread) {
                tombstones.remove(thread);
            }
        }
    }
    merge_records(
        "threads",
        &mut canonical.thread_memberships,
        &current.thread_memberships,
        &base.thread_memberships,
        Some(registry.deletions.entry("threads".into()).or_default()),
    )?;
    merge_records(
        "appearances",
        &mut canonical.project_appearances,
        &current.project_appearances,
        &base.project_appearances,
        None,
    )?;
    merge_records(
        "thread orders",
        &mut canonical.sidebar_project_thread_orders,
        &current.sidebar_project_thread_orders,
        &base.sidebar_project_thread_orders,
        None,
    )?;
    merge_records(
        "host mappings",
        &mut canonical.app_server_project_ids,
        &current.app_server_project_ids,
        &base.app_server_project_ids,
        None,
    )?;
    let as_map = |set: &BTreeSet<String>| {
        set.iter()
            .map(|id| (id.clone(), true))
            .collect::<BTreeMap<_, _>>()
    };
    let mut pins = as_map(&canonical.pinned_project_ids);
    merge_records(
        "pins",
        &mut pins,
        &as_map(&current.pinned_project_ids),
        &as_map(&base.pinned_project_ids),
        None,
    )?;
    canonical.pinned_project_ids = pins.into_keys().collect();
    merge_order(canonical, current, baseline)
}

fn effective_order(snapshot: &LocalSnapshot) -> Vec<String> {
    let mut order = snapshot.project_order.clone();
    for id in snapshot.projects.keys() {
        if !order.contains(id) {
            order.push(id.clone());
        }
    }
    order
}

fn merge_order(
    canonical: &mut LocalSnapshot,
    current: &LocalSnapshot,
    baseline: Option<&LocalSnapshot>,
) -> Result<(), StateError> {
    let current_order = effective_order(current);
    let Some(base) = baseline else {
        // A first-time participant supplies new records in its stable order,
        // without overriding the order already established by another owner.
        for id in current_order {
            if !canonical.project_order.contains(&id) {
                canonical.project_order.push(id);
            }
        }
        return Ok(());
    };
    // Compare order only for surviving baseline records. Independent additions
    // and deletions are content changes, not competing reorder operations.
    let compared: BTreeSet<_> = base
        .projects
        .keys()
        .filter(|id| canonical.projects.contains_key(*id) && current.projects.contains_key(*id))
        .cloned()
        .collect();
    let subset = |order: &[String]| {
        order
            .iter()
            .filter(|id| compared.contains(*id))
            .cloned()
            .collect::<Vec<_>>()
    };
    let previous = subset(&effective_order(base));
    let desired = subset(&current_order);
    let shared = subset(&canonical.project_order);
    if desired != previous {
        if shared != previous && shared != desired {
            return Err(invalid("conflicting project order"));
        }
        let mut reordered = desired.into_iter();
        for id in &mut canonical.project_order {
            if compared.contains(id) {
                *id = reordered
                    .next()
                    .ok_or_else(|| invalid("invalid canonical project order"))?;
            }
        }
    }
    // Newly created IDs retain the placement explicitly chosen by their owner.
    // Existing IDs unseen in this owner's baseline stay where canonical put them.
    for (index, id) in current_order.iter().enumerate() {
        if canonical.project_order.contains(id) {
            continue;
        }
        let next = current_order[index + 1..].iter().find_map(|next| {
            canonical
                .project_order
                .iter()
                .position(|value| value == next)
        });
        let previous = current_order[..index].iter().rev().find_map(|previous| {
            canonical
                .project_order
                .iter()
                .position(|value| value == previous)
        });
        let insertion = next
            .or_else(|| previous.map(|index| index + 1))
            .unwrap_or(canonical.project_order.len());
        canonical.project_order.insert(insertion, id.clone());
    }
    Ok(())
}

fn merge_records<T: Clone + PartialEq>(
    kind: &str,
    canonical: &mut BTreeMap<String, T>,
    current: &BTreeMap<String, T>,
    base: &BTreeMap<String, T>,
    mut tombstones: Option<&mut BTreeSet<String>>,
) -> Result<(), StateError> {
    let keys: BTreeSet<_> = current.keys().chain(base.keys()).cloned().collect();
    for key in keys {
        let value = current.get(&key);
        let previous = base.get(&key);
        if value == previous {
            continue;
        }
        let shared = canonical.get(&key);
        if shared == value {
            continue;
        }
        if value.is_some()
            && shared.is_none()
            && tombstones.as_ref().is_some_and(|set| set.contains(&key))
        {
            return Err(invalid(format!(
                "deleted {kind} record cannot be resurrected: {key}"
            )));
        }
        if shared != previous {
            return Err(invalid(format!("conflicting {kind} record: {key}")));
        }
        match value {
            Some(value) => {
                canonical.insert(key, value.clone());
            }
            None => {
                canonical.remove(&key);
                if let Some(tombstones) = tombstones.as_mut() {
                    tombstones.insert(key);
                }
            }
        }
    }
    Ok(())
}
