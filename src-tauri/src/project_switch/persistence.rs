use super::*;
use std::fs;
use store_lock::{atomic_write, read_optional, FileLock};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Change {
    pub key: String,
    pub before: Option<Vec<u8>>,
    pub after: Vec<u8>,
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Journal {
    version: u8,
    home: PathBuf,
    store: PathBuf,
    committed: bool,
    #[serde(default)]
    complete_on_recovery: bool,
    changes: Vec<Change>,
}
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum PendingPhase {
    Staging,
    Ready,
    Finished,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PendingMarker {
    version: u8,
    home: PathBuf,
    store: PathBuf,
    journal: PathBuf,
    phase: PendingPhase,
}
fn read_pending(paths: &SwitchPaths) -> Result<Option<PendingMarker>, String> {
    let Some(bytes) = read_optional(&store_lock::pending_marker_path(&paths.store_path)?)? else {
        return Ok(None);
    };
    let marker: PendingMarker =
        serde_json::from_slice(&bytes).map_err(|_| "PROJECT_SHARING_RECOVERY_REQUIRED")?;
    if marker.version != 1
        || marker.home != paths.codex_home
        || marker.store != paths.store_path
        || marker.journal != paths.journal()
    {
        return Err("PROJECT_SHARING_RECOVERY_REQUIRED".into());
    }
    Ok(Some(marker))
}
fn write_pending(paths: &SwitchPaths, phase: PendingPhase) -> Result<(), String> {
    read_pending(paths)?;
    let marker = PendingMarker {
        version: 1,
        home: paths.codex_home.clone(),
        store: paths.store_path.clone(),
        journal: paths.journal(),
        phase,
    };
    atomic_write(
        &store_lock::pending_marker_path(&paths.store_path)?,
        &serde_json::to_vec(&marker).map_err(|_| "PROJECT_SHARING_INVALID_STATE")?,
    )
}
fn remove_pending(paths: &SwitchPaths) -> Result<(), String> {
    if read_pending(paths)?.is_some() {
        fs::remove_file(store_lock::pending_marker_path(&paths.store_path)?)
            .map_err(|_| "PROJECT_SHARING_IO")?;
        store_lock::sync_dir(
            paths
                .store_path
                .parent()
                .ok_or("PROJECT_SHARING_INVALID_STATE")?,
        )?;
    }
    Ok(())
}
pub(super) struct Locks {
    _desktop: FileLock,
    _store: FileLock,
}
pub(super) fn locks(paths: &SwitchPaths) -> Result<Locks, String> {
    let desktop = FileLock::acquire(&paths.codex_home.join(".codex-tools-project-switch.lock"))?;
    let store = FileLock::acquire(
        &paths
            .store_path
            .parent()
            .ok_or("PROJECT_SHARING_INVALID_STATE")?
            .join(".accounts-store.lock"),
    )?;
    Ok(Locks {
        _desktop: desktop,
        _store: store,
    })
}
pub(super) fn target(paths: &SwitchPaths, key: &str) -> Result<PathBuf, String> {
    let path = match key {
        "auth" => paths.codex_home.join("auth.json"),
        "config" => paths.codex_home.join("config.toml"),
        "global" => paths.global(),
        "global-backup" => paths.codex_home.join(".codex-global-state.json.bak"),
        "registry" => paths.registry(),
        "store" => paths.store_path.clone(),
        "last-good" => paths
            .store_path
            .with_file_name("accounts.json.last-good.json"),
        "prev-good" => paths
            .store_path
            .with_file_name("accounts.json.prev-good.json"),
        _ => {
            let bits: Vec<_> = key.split('/').collect();
            if bits.len() != 3
                || bits[0] != "profile"
                || !safe_id(bits[1])
                || !matches!(bits[2], "auth.json" | "config.toml")
            {
                return Err("PROJECT_SHARING_INVALID_STATE".into());
            }
            paths
                .store_path
                .parent()
                .ok_or("PROJECT_SHARING_INVALID_STATE")?
                .join("profiles")
                .join(bits[1])
                .join(bits[2])
        }
    };
    // Derived profile paths must not follow a persisted symlink outside the store.
    let root = if path.starts_with(&paths.codex_home) {
        &paths.codex_home
    } else {
        paths
            .store_path
            .parent()
            .ok_or("PROJECT_SHARING_INVALID_STATE")?
    };
    for parent in path
        .parent()
        .into_iter()
        .flat_map(Path::ancestors)
        .take_while(|p| *p != root)
    {
        match fs::symlink_metadata(parent) {
            Ok(m) if !m.file_type().is_dir() => return Err("PROJECT_SHARING_INVALID_STATE".into()),
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                return Err("PROJECT_SHARING_IO".into())
            }
            _ => {}
        }
    }
    Ok(path)
}
pub(super) fn safe_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 200
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
}
pub(super) fn change(
    paths: &SwitchPaths,
    key: impl Into<String>,
    after: Vec<u8>,
) -> Result<Change, String> {
    let key = key.into();
    let path = target(paths, &key)?;
    Ok(Change {
        key,
        before: read_optional(&path)?,
        after,
    })
}
fn consistent(paths: &SwitchPaths, changes: &[Change], allow_after: bool) -> Result<(), String> {
    for c in changes {
        let current = read_optional(&target(paths, &c.key)?)?;
        if current != c.before && !(allow_after && current.as_deref() == Some(c.after.as_slice())) {
            return Err("PROJECT_SHARING_EXTERNAL_CHANGE".into());
        }
    }
    Ok(())
}
fn rollback(
    paths: &SwitchPaths,
    changes: &[Change],
    lifecycle: &mut impl DesktopLifecycle,
) -> Result<(), String> {
    lifecycle.assert_stopped()?;
    consistent(paths, changes, true)?;
    for c in changes.iter().rev() {
        lifecycle.assert_stopped()?;
        let path = target(paths, &c.key)?;
        let current = read_optional(&path)?;
        if current == c.before {
            continue;
        }
        if current.as_deref() != Some(c.after.as_slice()) {
            return Err("PROJECT_SHARING_EXTERNAL_CHANGE".into());
        }
        match &c.before {
            Some(bytes) => atomic_write(&path, bytes)?,
            None => {
                fs::remove_file(&path).map_err(|_| "PROJECT_SHARING_IO")?;
                store_lock::sync_dir(path.parent().unwrap())?;
            }
        }
    }
    Ok(())
}
fn owned_atomic_temp(name: &str) -> bool {
    name.strip_prefix(".project-switch-")
        .and_then(|s| s.strip_suffix(".tmp"))
        .is_some_and(|id| uuid::Uuid::parse_str(id).is_ok_and(|uuid| uuid.to_string() == id))
}
fn clear(paths: &SwitchPaths) -> Result<(), String> {
    let directory = paths.journal();
    let manifest = directory.join("journal.json");
    let original = read_optional(&manifest)?.ok_or("PROJECT_SHARING_RECOVERY_REQUIRED")?;
    // User files are now fully committed or restored; keep the gate through cleanup.
    write_pending(paths, PendingPhase::Finished)?;
    let mut temps = Vec::new();
    // Validate the entire directory before deleting anything. Never recurse,
    // follow symlinks, or remove unrelated contents beside the journal.
    for entry in fs::read_dir(&directory).map_err(|_| "PROJECT_SHARING_IO")? {
        let entry = entry.map_err(|_| "PROJECT_SHARING_IO")?;
        let name = entry.file_name();
        let name = name.to_str().ok_or("PROJECT_SHARING_RECOVERY_REQUIRED")?;
        if !entry
            .file_type()
            .map_err(|_| "PROJECT_SHARING_IO")?
            .is_file()
        {
            return Err("PROJECT_SHARING_RECOVERY_REQUIRED".into());
        }
        if name == "journal.json" {
            continue;
        }
        if !owned_atomic_temp(name) {
            return Err("PROJECT_SHARING_RECOVERY_REQUIRED".into());
        }
        temps.push(entry.path());
    }
    // A failed temp cleanup leaves the only recovery record intact.
    for temp in temps {
        fs::remove_file(temp).map_err(|_| "PROJECT_SHARING_IO")?;
    }
    store_lock::sync_dir(&directory)?;
    fs::remove_file(&manifest).map_err(|_| "PROJECT_SHARING_IO")?;
    if fs::remove_dir(&directory).is_err() {
        // An unexpected late directory failure must not strand a nonempty
        // journal directory without its recovery record.
        atomic_write(&manifest, &original).map_err(|_| "PROJECT_SHARING_RECOVERY_REQUIRED")?;
        return Err("PROJECT_SHARING_IO".into());
    }
    store_lock::sync_dir(&paths.codex_home)?;
    remove_pending(paths)
}
pub(super) fn apply(
    paths: &SwitchPaths,
    changes: Vec<Change>,
    lifecycle: &mut impl DesktopLifecycle,
) -> Result<(), String> {
    apply_with_policy(paths, changes, lifecycle, false)
}
pub(super) fn apply_refreshed_credentials(
    paths: &SwitchPaths,
    changes: Vec<Change>,
    lifecycle: &mut impl DesktopLifecycle,
) -> Result<(), String> {
    apply_with_policy(paths, changes, lifecycle, true)
}
fn apply_with_policy(
    paths: &SwitchPaths,
    changes: Vec<Change>,
    lifecycle: &mut impl DesktopLifecycle,
    complete_on_recovery: bool,
) -> Result<(), String> {
    apply_with_manifest_writer(
        paths,
        changes,
        lifecycle,
        complete_on_recovery,
        atomic_write,
    )
}
pub(super) fn apply_with_manifest_writer(
    paths: &SwitchPaths,
    changes: Vec<Change>,
    lifecycle: &mut impl DesktopLifecycle,
    complete_on_recovery: bool,
    mut write_manifest: impl FnMut(&Path, &[u8]) -> Result<(), String>,
) -> Result<(), String> {
    store_lock::ensure_store_available(&paths.store_path)?;
    if paths.journal().exists() {
        return Err("PROJECT_SHARING_RECOVERY_REQUIRED".into());
    }
    if !complete_on_recovery {
        consistent(paths, &changes, false)?;
        lifecycle.assert_stopped()?;
    }
    // Rotated OAuth credentials must become durable even if an unrelated
    // writer appeared during the network request. Only the journal is written
    // before CAS/stopped checks; live credentials remain guarded below.
    write_pending(paths, PendingPhase::Staging)?;
    store_lock::private_dir(&paths.journal())?;
    let mut journal = Journal {
        version: 1,
        home: paths.codex_home.clone(),
        store: paths.store_path.clone(),
        committed: false,
        complete_on_recovery,
        changes,
    };
    write_manifest(
        &paths.journal().join("journal.json"),
        &serde_json::to_vec(&journal).map_err(|_| "PROJECT_SHARING_INVALID_STATE")?,
    )?;
    write_pending(paths, PendingPhase::Ready)?;
    let outcome = (|| {
        for c in &journal.changes {
            lifecycle.assert_stopped()?;
            consistent(paths, &journal.changes, true)?;
            if read_optional(&target(paths, &c.key)?)? != c.before {
                return Err("PROJECT_SHARING_EXTERNAL_CHANGE".into());
            }
            atomic_write(&target(paths, &c.key)?, &c.after)?;
        }
        for c in &journal.changes {
            if read_optional(&target(paths, &c.key)?)?.as_deref() != Some(c.after.as_slice()) {
                return Err("PROJECT_SHARING_EXTERNAL_CHANGE".into());
            }
        }
        Ok(())
    })();
    if let Err(error) = outcome {
        if complete_on_recovery {
            return Err("PROJECT_SHARING_RECOVERY_REQUIRED".into());
        }
        return match rollback(paths, &journal.changes, lifecycle) {
            Ok(()) => {
                clear(paths)?;
                Err(error)
            }
            Err(_) => Err("PROJECT_SHARING_RECOVERY_REQUIRED".into()),
        };
    }
    journal.committed = true;
    write_manifest(
        &paths.journal().join("journal.json"),
        &serde_json::to_vec(&journal).map_err(|_| "PROJECT_SHARING_INVALID_STATE")?,
    )?;
    clear(paths)
}
fn validate_journal(paths: &SwitchPaths, j: &Journal) -> Result<(), String> {
    if j.version != 1
        || j.home != paths.codex_home
        || j.store != paths.store_path
        || j.changes.is_empty()
    {
        return Err("PROJECT_SHARING_INVALID_STATE".into());
    }
    let mut keys = std::collections::BTreeSet::new();
    for c in &j.changes {
        target(paths, &c.key)?;
        if !keys.insert(&c.key) {
            return Err("PROJECT_SHARING_INVALID_STATE".into());
        }
    }
    Ok(())
}
fn parse_journal(paths: &SwitchPaths, bytes: &[u8]) -> Result<Journal, String> {
    let j: Journal = serde_json::from_slice(bytes).map_err(|_| "PROJECT_SHARING_INVALID_STATE")?;
    validate_journal(paths, &j)?;
    Ok(j)
}
/// A completed first atomic-write temp may be the only durable copy of rotated
/// credentials. Promote only complete, path-bound and mutually equal manifests.
fn first_manifest_candidate(paths: &SwitchPaths) -> Result<Option<Journal>, String> {
    let mut candidate: Option<Journal> = None;
    for entry in fs::read_dir(paths.journal()).map_err(|_| "PROJECT_SHARING_IO")? {
        let entry = entry.map_err(|_| "PROJECT_SHARING_IO")?;
        let name = entry.file_name();
        if !name.to_str().is_some_and(owned_atomic_temp)
            || !entry
                .file_type()
                .map_err(|_| "PROJECT_SHARING_IO")?
                .is_file()
        {
            return Err("PROJECT_SHARING_RECOVERY_REQUIRED".into());
        }
        let bytes = read_optional(&entry.path())?.ok_or("PROJECT_SHARING_RECOVERY_REQUIRED")?;
        let value = parse_journal(paths, &bytes)?;
        if candidate.as_ref().is_some_and(|other| other != &value) {
            return Err("PROJECT_SHARING_RECOVERY_REQUIRED".into());
        }
        candidate = Some(value);
    }
    Ok(candidate)
}
pub(super) fn recover_locked(
    paths: &SwitchPaths,
    lifecycle: &mut impl DesktopLifecycle,
) -> Result<bool, String> {
    let pending = read_pending(paths)?;
    if !paths.journal().exists() {
        return match pending {
            Some(marker) if marker.phase != PendingPhase::Ready => {
                lifecycle.assert_stopped()?;
                remove_pending(paths)?;
                Ok(true)
            }
            Some(_) => Err("PROJECT_SHARING_RECOVERY_REQUIRED".into()),
            None => Ok(false),
        };
    }
    if !fs::symlink_metadata(paths.journal())
        .map_err(|_| "PROJECT_SHARING_IO")?
        .file_type()
        .is_dir()
    {
        return Err("PROJECT_SHARING_INVALID_STATE".into());
    }
    let manifest = paths.journal().join("journal.json");
    let (j, promote) = match read_optional(&manifest)? {
        Some(bytes) => (parse_journal(paths, &bytes)?, false),
        None => match first_manifest_candidate(paths)? {
            Some(candidate) => (candidate, true),
            None => {
                if pending
                    .as_ref()
                    .is_some_and(|p| p.phase == PendingPhase::Ready)
                {
                    return Err("PROJECT_SHARING_RECOVERY_REQUIRED".into());
                }
                lifecycle.assert_stopped()?;
                fs::remove_dir(paths.journal()).map_err(|_| "PROJECT_SHARING_IO")?;
                store_lock::sync_dir(&paths.codex_home)?;
                remove_pending(paths)?;
                return Ok(true);
            }
        },
    };
    lifecycle.assert_stopped()?;
    consistent(paths, &j.changes, true)?;
    if pending
        .as_ref()
        .map_or(true, |p| p.phase == PendingPhase::Staging)
    {
        write_pending(paths, PendingPhase::Ready)?;
    }
    if promote {
        atomic_write(
            &manifest,
            &serde_json::to_vec(&j).map_err(|_| "PROJECT_SHARING_INVALID_STATE")?,
        )?;
    }
    if j.committed {
        for c in &j.changes {
            if read_optional(&target(paths, &c.key)?)?.as_deref() != Some(c.after.as_slice()) {
                return Err("PROJECT_SHARING_EXTERNAL_CHANGE".into());
            }
        }
    } else if j.complete_on_recovery {
        for c in &j.changes {
            lifecycle.assert_stopped()?;
            let path = target(paths, &c.key)?;
            let current = read_optional(&path)?;
            if current.as_deref() == Some(c.after.as_slice()) {
                continue;
            }
            if current != c.before {
                return Err("PROJECT_SHARING_EXTERNAL_CHANGE".into());
            }
            atomic_write(&path, &c.after)?;
        }
    } else {
        rollback(paths, &j.changes, lifecycle)?;
    }
    clear(paths)?;
    Ok(true)
}
