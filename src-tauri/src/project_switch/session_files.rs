use super::{persistence, Plan, SwitchPaths};

pub(super) fn checkpoint_changes(
    paths: &SwitchPaths,
    target: &str,
    plan: &Plan,
) -> Result<Vec<persistence::Change>, String> {
    let mut changes = Vec::new();
    if let Some(source) = &plan.source_id {
        changes.push(persistence::change(
            paths,
            format!("profile/{source}/desktop-state.json"),
            serde_json::to_vec(&plan.sharing.source)
                .map_err(|_| "PROJECT_SHARING_INVALID_STATE")?,
        )?);
    }
    let target_snapshot =
        persistence::target(paths, &format!("profile/{target}/desktop-state.json"))?;
    let previous = plan
        .sharing
        .previous_target
        .as_ref()
        .map(serde_json::to_vec)
        .transpose()
        .map_err(|_| "PROJECT_SHARING_INVALID_STATE")?
        .or_else(|| plan.inputs.optional(&target_snapshot).map(<[u8]>::to_vec));
    if let Some(previous) = previous {
        changes.push(persistence::change(
            paths,
            format!("profile/{target}/desktop-state.previous.json"),
            previous,
        )?);
    }
    Ok(changes)
}
