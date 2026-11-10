use super::*;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde_json::json;
use std::{collections::BTreeMap, fs};
struct Fixture {
    paths: SwitchPaths,
    owners: Vec<String>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(self.paths.codex_home.parent().unwrap());
    }
}
fn auth(id: &str) -> Value {
    let claims = json!({"exp":4102444800u64,"https://api.openai.com/auth":{"chatgpt_user_id":format!("synthetic-user-{id}"),"chatgpt_account_id":format!("synthetic-account-{id}")}});
    let token = format!(
        "synthetic.{}.signature",
        URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims).unwrap())
    );
    json!({"tokens":{"id_token":token,"access_token":token,"refresh_token":"synthetic-not-a-credential","account_id":format!("synthetic-account-{id}")}})
}
fn fixture(projects: usize, links: usize, unassigned: usize) -> Fixture {
    let dir = std::env::temp_dir().join(format!("project-switch-{}", uuid::Uuid::new_v4()));
    let home = dir.join("codex");
    let data = dir.join("data");
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(&data).unwrap();
    let paths = SwitchPaths::new(home, data.join("accounts.json")).unwrap();
    let mut store = AccountsStore::default();
    store.settings.active_account_id = Some("a".into());
    let owners: Vec<_> = ["a", "b", "c"]
        .iter()
        .map(|id| engine::owner_from_auth(&auth(id)).unwrap())
        .collect();
    for id in ["a", "b", "c"] {
        store.accounts.push(serde_json::from_value(json!({"id":id,"label":format!("Synthetic {id}"),"accountId":format!("synthetic-account-{id}"),"authJson":auth(id),"addedAt":1,"updatedAt":1})).unwrap());
    }
    fs::write(
        &paths.store_path,
        serde_json::to_vec_pretty(&store).unwrap(),
    )
    .unwrap();
    fs::write(
        paths.codex_home.join("auth.json"),
        serde_json::to_vec(&auth("a")).unwrap(),
    )
    .unwrap();
    fs::write(
        paths.codex_home.join("config.toml"),
        "model_provider = \"openai\"\napproval_policy = \"on-request\"\n",
    )
    .unwrap();
    fs::write(
        paths.codex_home.join("synthetic-session.jsonl"),
        "{\"synthetic\":true}\n",
    )
    .unwrap();
    let db = rusqlite::Connection::open(paths.codex_home.join("state_5.sqlite")).unwrap();
    db.execute_batch("CREATE TABLE projects(id TEXT PRIMARY KEY,name TEXT,metadata TEXT DEFAULT '{}'); CREATE TABLE project_roots(project_id TEXT,position INTEGER,path TEXT); CREATE TABLE threads(id TEXT PRIMARY KEY,project_id TEXT);").unwrap();
    let mut local = serde_json::Map::new();
    let mut mappings = serde_json::Map::new();
    let mut order = Vec::new();
    for n in 0..projects {
        let id = format!("synthetic-project-{n}");
        let server = format!("synthetic-server-{n}");
        let root = format!("/synthetic-only/workspace-{n}");
        local.insert(id.clone(),json!({"id":id,"name":format!("Fictional project {n}"),"rootPaths":[root],"createdAt":1,"updatedAt":1}));
        order.push(id.clone());
        mappings.insert(id, json!(server));
        db.execute(
            "INSERT INTO projects(id,name) VALUES(?1,?2)",
            rusqlite::params![server, format!("Fictional project {n}")],
        )
        .unwrap();
        db.execute(
            "INSERT INTO project_roots VALUES(?1,0,?2)",
            rusqlite::params![server, root],
        )
        .unwrap();
    }
    let mut assignments = serde_json::Map::new();
    for n in 0..links {
        let id = format!("synthetic-thread-{n}");
        assignments.insert(
            id.clone(),
            json!({"projectKind":"local","projectId":format!("synthetic-project-{}",n%projects)}),
        );
        db.execute("INSERT INTO threads(id) VALUES(?1)", [id])
            .unwrap();
    }
    let mut ungrouped = Vec::new();
    for n in 0..unassigned {
        let id = format!("synthetic-unassigned-{n}");
        db.execute("INSERT INTO threads(id) VALUES(?1)", [&id])
            .unwrap();
        ungrouped.push(id);
    }
    let doc = json!({"electron-account-renderer-state-v1":{"owner":owners[0],"inactive":{}},"local-projects":local,"project-order":order,"thread-project-assignments":assignments,"projectless-thread-ids":ungrouped,"app-server-project-id-by-legacy-project-id-by-host":{paths.host():mappings},"unrelated":{"permission":"keep"}});
    fs::write(paths.global(), serde_json::to_vec_pretty(&doc).unwrap()).unwrap();
    fs::write(
        paths.codex_home.join(".codex-global-state.json.bak"),
        b"synthetic-original-backup",
    )
    .unwrap();
    Fixture { paths, owners }
}
#[derive(Default)]
struct FakeLifecycle {
    stops: usize,
    checks: usize,
    refuse: bool,
    fail_check: Option<usize>,
}
impl DesktopLifecycle for FakeLifecycle {
    fn stop_and_verify(&mut self) -> Result<(), String> {
        self.stops += 1;
        if self.refuse {
            Err("PROJECT_SHARING_DESKTOP_RUNNING".into())
        } else {
            Ok(())
        }
    }
    fn assert_stopped(&mut self) -> Result<(), String> {
        self.checks += 1;
        if self.fail_check == Some(self.checks) {
            Err("PROJECT_SHARING_DESKTOP_RUNNING".into())
        } else {
            Ok(())
        }
    }
}
fn tree(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    tree_with_locks(root, true)
}
fn tree_with_locks(root: &Path, include_locks: bool) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut out = BTreeMap::new();
    for e in fs::read_dir(root).unwrap() {
        let p = e.unwrap().path();
        // Windows lock handles deny reads. Filter them before opening files.
        if !include_locks && p.file_name().unwrap().to_string_lossy().ends_with(".lock") {
            continue;
        }
        if p.is_dir() {
            out.extend(tree_with_locks(&p, include_locks));
        } else {
            out.insert(p.clone(), fs::read(p).unwrap());
        }
    }
    out
}
#[test]
fn preview_is_read_only_and_reports_membership_counts() {
    let f = fixture(24, 81, 3);
    let before = tree(f.paths.codex_home.parent().unwrap());
    let p = preview(&f.paths, "b").unwrap();
    assert_eq!(
        (
            p.project_count,
            p.thread_link_count,
            p.unassigned_thread_count
        ),
        (24, 81, 3)
    );
    assert_eq!(before, tree(f.paths.codex_home.parent().unwrap()));
}

#[tokio::test]
async fn cached_memberships_survive_absent_thread_index_rows_without_database_writes() {
    let f = fixture(2, 2, 1);
    let file = f.paths.codex_home.join("state_5.sqlite");
    let db = rusqlite::Connection::open(&file).unwrap();
    db.execute(
        "DELETE FROM threads WHERE id IN ('synthetic-thread-0','synthetic-unassigned-0')",
        [],
    )
    .unwrap();
    drop(db);
    let before = tree(f.paths.codex_home.parent().unwrap());
    let p = preview(&f.paths, "b").unwrap();
    assert_eq!(
        (
            p.project_count,
            p.thread_link_count,
            p.unassigned_thread_count
        ),
        (2, 2, 1)
    );
    assert_eq!(before, tree(f.paths.codex_home.parent().unwrap()));
    let original = json_file(&f.paths.global()).unwrap();
    let db_before = fs::read(&file).unwrap();
    let session_before = fs::read(f.paths.codex_home.join("synthetic-session.jsonl")).unwrap();
    let mut lifecycle = FakeLifecycle::default();
    let mut done = execute(&f.paths, "b", SwitchOptions::default(), &mut lifecycle).unwrap();
    let written = json_file(&f.paths.global()).unwrap();
    let target =
        &written["electron-account-renderer-state-v1"]["inactive"][&f.owners[1]]["globals"];
    assert_eq!(
        target["thread-project-assignments"],
        original["thread-project-assignments"]
    );
    assert_eq!(
        target["projectless-thread-ids"],
        original["projectless-thread-ids"]
    );
    assert_eq!(
        written["thread-project-assignments"],
        original["thread-project-assignments"]
    );
    desktop_restore(&f.paths, &f.owners[1]);
    verify_after_launch(&f.paths, &mut done).await;
    assert_eq!(done.result.status, "verified");
    drop(done);
    let mut back = execute(
        &f.paths,
        "a",
        SwitchOptions {
            accept_merge: true,
            ..SwitchOptions::default()
        },
        &mut lifecycle,
    )
    .unwrap();
    desktop_restore(&f.paths, &f.owners[0]);
    verify_after_launch(&f.paths, &mut back).await;
    assert_eq!(back.result.status, "verified");
    let restored = json_file(&f.paths.global()).unwrap();
    assert_eq!(
        restored["thread-project-assignments"],
        original["thread-project-assignments"]
    );
    assert_eq!(
        restored["projectless-thread-ids"],
        original["projectless-thread-ids"]
    );
    assert_eq!(fs::read(&file).unwrap(), db_before);
    assert_eq!(
        fs::read(f.paths.codex_home.join("synthetic-session.jsonl")).unwrap(),
        session_before
    );
}
#[test]
fn preparation_preserves_source_and_never_writes_sqlite_or_transcripts() {
    let f = fixture(2, 5, 1);
    let db = fs::read(f.paths.codex_home.join("state_5.sqlite")).unwrap();
    let original: Value = serde_json::from_slice(&fs::read(f.paths.global()).unwrap()).unwrap();
    let mut fake = FakeLifecycle::default();
    let done = execute(&f.paths, "b", SwitchOptions::default(), &mut fake).unwrap();
    assert_eq!(done.result.status, "prepared");
    assert_eq!(fake.stops, 1);
    let after: Value = serde_json::from_slice(&fs::read(f.paths.global()).unwrap()).unwrap();
    assert_eq!(after["local-projects"], original["local-projects"]);
    assert_eq!(
        after["electron-account-renderer-state-v1"]["owner"],
        f.owners[0]
    );
    assert_eq!(
        db,
        fs::read(f.paths.codex_home.join("state_5.sqlite")).unwrap()
    );
    assert_eq!(
        strict_store(&f.paths.store_path)
            .unwrap()
            .settings
            .active_account_id
            .as_deref(),
        Some("b")
    );
}
#[test]
fn same_owner_is_noop_without_lifecycle() {
    let f = fixture(2, 4, 0);
    let before = tree(f.paths.codex_home.parent().unwrap());
    let mut fake = FakeLifecycle::default();
    let done = execute(&f.paths, "a", SwitchOptions::default(), &mut fake).unwrap();
    assert_eq!(done.result.status, "noOp");
    assert_eq!(fake.stops, 0);
    assert_eq!(before, tree(f.paths.codex_home.parent().unwrap()));
}
#[test]
fn refusing_stop_leaves_active_files_untouched() {
    let f = fixture(2, 3, 1);
    let before = fs::read(f.paths.global()).unwrap();
    let auth = fs::read(f.paths.codex_home.join("auth.json")).unwrap();
    let mut fake = FakeLifecycle {
        refuse: true,
        ..Default::default()
    };
    assert!(execute(&f.paths, "b", SwitchOptions::default(), &mut fake).is_err());
    assert_eq!(before, fs::read(f.paths.global()).unwrap());
    assert_eq!(
        auth,
        fs::read(f.paths.codex_home.join("auth.json")).unwrap()
    );
}
fn tracked_tree(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    tree_with_locks(root, false)
}
// Independent simulation of the observed desktop archive/restore protocol.
fn desktop_restore(paths: &SwitchPaths, target: &str) {
    let mut doc = json_file(&paths.global()).unwrap();
    let source = doc["electron-account-renderer-state-v1"]["owner"]
        .as_str()
        .unwrap()
        .to_owned();
    let keys = [
        "local-projects",
        "thread-project-assignments",
        "thread-project-membership-host-ids",
        "projectless-thread-ids",
        "project-order",
        "pinned-project-ids",
        "project-appearances",
        "sidebar-project-thread-orders",
        "app-server-project-id-by-legacy-project-id-by-host",
    ];
    let mut archived = serde_json::Map::new();
    for key in keys {
        if let Some(v) = doc.get(key) {
            archived.insert(key.into(), v.clone());
        }
    }
    let restored = doc["electron-account-renderer-state-v1"]["inactive"][target]["globals"].clone();
    doc["electron-account-renderer-state-v1"]["inactive"][&source] =
        json!({"atoms":{},"globals":archived});
    for key in keys {
        doc.as_object_mut().unwrap().remove(key);
        if let Some(v) = restored.get(key) {
            doc[key] = v.clone();
        }
    }
    doc["electron-account-renderer-state-v1"]["owner"] = json!(target);
    fs::write(paths.global(), serde_json::to_vec_pretty(&doc).unwrap()).unwrap();
}
#[tokio::test]
async fn synthetic_24_projects_81_links_roundtrip_a_b_c_a() {
    let f = fixture(24, 81, 4);
    let db = fs::read(f.paths.codex_home.join("state_5.sqlite")).unwrap();
    let session = fs::read(f.paths.codex_home.join("synthetic-session.jsonl")).unwrap();
    let initial = json_file(&f.paths.global()).unwrap();
    for (id, owner) in [
        ("b", &f.owners[1]),
        ("c", &f.owners[2]),
        ("a", &f.owners[0]),
    ] {
        let preview = preview(&f.paths, id).unwrap();
        let mut done = execute(
            &f.paths,
            id,
            SwitchOptions {
                accept_merge: true,
                preview_fingerprint: Some(preview.preview_fingerprint),
                target_auth: None,
            },
            &mut FakeLifecycle::default(),
        )
        .unwrap();
        assert_eq!(
            (
                done.result.project_count,
                done.result.thread_link_count,
                done.result.unassigned_thread_count
            ),
            (24, 81, 4)
        );
        assert!(
            persistence::locks(&f.paths).is_err(),
            "prepared holds interprocess locks through verification"
        );
        desktop_restore(&f.paths, owner);
        verify_after_launch(&f.paths, &mut done).await;
        assert_eq!(done.result.status, "verified");
    }
    let final_doc = json_file(&f.paths.global()).unwrap();
    assert_eq!(final_doc["local-projects"], initial["local-projects"]);
    assert_eq!(
        final_doc["thread-project-assignments"],
        initial["thread-project-assignments"]
    );
    assert_eq!(
        final_doc["projectless-thread-ids"],
        initial["projectless-thread-ids"]
    );
    assert_eq!(
        db,
        fs::read(f.paths.codex_home.join("state_5.sqlite")).unwrap()
    );
    assert_eq!(
        session,
        fs::read(f.paths.codex_home.join("synthetic-session.jsonl")).unwrap()
    );
}
#[test]
fn stopped_gate_failure_rolls_back_all_file_bytes() {
    let f = fixture(2, 5, 1);
    let before = tracked_tree(f.paths.codex_home.parent().unwrap());
    let mut fake = FakeLifecycle {
        fail_check: Some(7),
        ..Default::default()
    };
    assert!(execute(&f.paths, "b", SwitchOptions::default(), &mut fake).is_err());
    assert!(fake.checks >= 7);
    assert_eq!(before, tracked_tree(f.paths.codex_home.parent().unwrap()));
    assert!(!f.paths.journal().exists());
}
struct CrashingLifecycle {
    checks: usize,
}
impl DesktopLifecycle for CrashingLifecycle {
    fn stop_and_verify(&mut self) -> Result<(), String> {
        Ok(())
    }
    fn assert_stopped(&mut self) -> Result<(), String> {
        self.checks += 1;
        if self.checks == 7 {
            panic!("synthetic crash");
        }
        Ok(())
    }
}
fn interrupted(f: &Fixture) {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = execute(
            &f.paths,
            "b",
            SwitchOptions::default(),
            &mut CrashingLifecycle { checks: 0 },
        );
    }));
    assert!(result.is_err());
    assert!(f.paths.journal().join("journal.json").exists());
}
#[test]
fn interrupted_journal_recovers_original_bytes() {
    let f = fixture(2, 5, 1);
    let before = tracked_tree(f.paths.codex_home.parent().unwrap());
    interrupted(&f);
    assert!(recover(&f.paths, &mut FakeLifecycle::default()).unwrap());
    assert_eq!(before, tracked_tree(f.paths.codex_home.parent().unwrap()));
}
#[test]
fn recovery_refuses_new_external_state_without_writing() {
    let f = fixture(2, 5, 1);
    interrupted(&f);
    fs::write(f.paths.global(), b"synthetic-external-change").unwrap();
    let before = tracked_tree(f.paths.codex_home.parent().unwrap());
    assert_eq!(
        recover(&f.paths, &mut FakeLifecycle::default()).unwrap_err(),
        "PROJECT_SHARING_EXTERNAL_CHANGE"
    );
    assert_eq!(before, tracked_tree(f.paths.codex_home.parent().unwrap()));
}
#[test]
fn stale_active_label_never_contaminates_another_profile() {
    let f = fixture(2, 4, 1);
    let mut store = strict_store(&f.paths.store_path).unwrap();
    store.settings.active_account_id = Some("b".into());
    fs::write(
        &f.paths.store_path,
        serde_json::to_vec_pretty(&store).unwrap(),
    )
    .unwrap();
    let unrelated = persistence::target(&f.paths, "profile/b/auth.json").unwrap();
    fs::create_dir_all(unrelated.parent().unwrap()).unwrap();
    fs::write(&unrelated, b"untouched synthetic B profile").unwrap();
    let done = execute(
        &f.paths,
        "c",
        SwitchOptions::default(),
        &mut FakeLifecycle::default(),
    )
    .unwrap();
    assert_eq!(done.result.status, "prepared");
    assert_eq!(
        fs::read(unrelated).unwrap(),
        b"untouched synthetic B profile"
    );
    assert_eq!(
        json_file(&persistence::target(&f.paths, "profile/a/auth.json").unwrap()).unwrap(),
        auth("a")
    );
}
#[test]
fn nonempty_target_requires_acceptance_of_current_preview() {
    let f = fixture(2, 4, 0);
    let mut doc = json_file(&f.paths.global()).unwrap();
    let mut target_project = doc["local-projects"]["synthetic-project-0"].clone();
    target_project["id"] = json!("target-only");
    doc["electron-account-renderer-state-v1"]["inactive"][&f.owners[1]] = json!({"atoms":{"private":"preserved"},"globals":{"local-projects":{"target-only":target_project}}});
    fs::write(f.paths.global(), serde_json::to_vec_pretty(&doc).unwrap()).unwrap();
    let p = preview(&f.paths, "b").unwrap();
    assert!(p.requires_merge_confirmation);
    assert_eq!(p.retained_target_project_count, 1);
    let mut fake = FakeLifecycle::default();
    assert!(execute(&f.paths, "b", SwitchOptions::default(), &mut fake).is_err());
    assert_eq!(fake.stops, 0);
    assert!(execute(
        &f.paths,
        "b",
        SwitchOptions {
            accept_merge: true,
            preview_fingerprint: Some("stale".into()),
            target_auth: None
        },
        &mut fake
    )
    .is_err());
    assert_eq!(fake.stops, 0);
    assert_eq!(
        execute(
            &f.paths,
            "b",
            SwitchOptions {
                accept_merge: true,
                preview_fingerprint: Some(p.preview_fingerprint),
                target_auth: None
            },
            &mut fake
        )
        .unwrap()
        .result
        .project_count,
        3
    );
}
#[test]
fn invalid_database_mapping_fails_before_any_process_or_write() {
    let f = fixture(2, 4, 0);
    let db = rusqlite::Connection::open(f.paths.codex_home.join("state_5.sqlite")).unwrap();
    db.execute("UPDATE project_roots SET path='/synthetic-wrong'", [])
        .unwrap();
    drop(db);
    let before = tracked_tree(f.paths.codex_home.parent().unwrap());
    let mut fake = FakeLifecycle::default();
    assert!(execute(&f.paths, "b", SwitchOptions::default(), &mut fake).is_err());
    assert_eq!(fake.stops, 0);
    assert_eq!(before, tracked_tree(f.paths.codex_home.parent().unwrap()));
}
#[test]
fn persisted_external_profile_paths_are_not_trusted() {
    let f = fixture(2, 4, 0);
    let mut store = strict_store(&f.paths.store_path).unwrap();
    let outside = f
        .paths
        .codex_home
        .parent()
        .unwrap()
        .join("synthetic-external-profile");
    fs::write(&outside, b"keep").unwrap();
    store.accounts[1].profile_auth_path = Some(outside.to_string_lossy().into_owned());
    store.accounts[1].profile_config_path = Some(outside.to_string_lossy().into_owned());
    fs::write(
        &f.paths.store_path,
        serde_json::to_vec_pretty(&store).unwrap(),
    )
    .unwrap();
    let _done = execute(
        &f.paths,
        "b",
        SwitchOptions::default(),
        &mut FakeLifecycle::default(),
    )
    .unwrap();
    assert_eq!(fs::read(outside).unwrap(), b"keep");
}
#[cfg(unix)]
#[test]
fn symlinked_profile_directory_cannot_escape_explicit_store() {
    let f = fixture(2, 4, 0);
    let outside = f
        .paths
        .codex_home
        .parent()
        .unwrap()
        .join("synthetic-external-directory");
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("auth.json"), b"keep").unwrap();
    let profile = persistence::target(&f.paths, "profile/b/auth.json").unwrap();
    fs::create_dir_all(profile.parent().unwrap().parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(&outside, profile.parent().unwrap()).unwrap();
    assert!(execute(
        &f.paths,
        "b",
        SwitchOptions::default(),
        &mut FakeLifecycle::default()
    )
    .is_err());
    assert_eq!(fs::read(outside.join("auth.json")).unwrap(), b"keep");
}
#[test]
fn wal_snapshot_reads_committed_records_without_source_writes() {
    let f = fixture(2, 4, 0);
    let db = rusqlite::Connection::open(f.paths.codex_home.join("state_5.sqlite")).unwrap();
    db.pragma_update(None, "journal_mode", "WAL").unwrap();
    db.execute("INSERT INTO threads(id) VALUES('synthetic-new-in-wal')", [])
        .unwrap();
    let mut document = json_file(&f.paths.global()).unwrap();
    document["thread-project-assignments"]["synthetic-new-in-wal"] =
        json!({"projectKind":"local","projectId":"synthetic-project-0"});
    fs::write(
        f.paths.global(),
        serde_json::to_vec_pretty(&document).unwrap(),
    )
    .unwrap();
    let before = tree(f.paths.codex_home.parent().unwrap());
    assert_eq!(preview(&f.paths, "b").unwrap().thread_link_count, 5);
    assert_eq!(before, tree(f.paths.codex_home.parent().unwrap()));
    drop(db);
}
#[tokio::test]
async fn refreshed_target_credentials_survive_subsequent_switch_failure() {
    let f = fixture(2, 4, 1);
    let source_auth = fs::read(f.paths.codex_home.join("auth.json")).unwrap();
    let global = fs::read(f.paths.global()).unwrap();
    let mut old = auth("b");
    let mut claims: Value = serde_json::from_slice(
        &URL_SAFE_NO_PAD
            .decode(
                old["tokens"]["access_token"]
                    .as_str()
                    .unwrap()
                    .split('.')
                    .nth(1)
                    .unwrap(),
            )
            .unwrap(),
    )
    .unwrap();
    claims["exp"] = json!(1);
    let token = format!(
        "synthetic.{}.signature",
        URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims).unwrap())
    );
    old["tokens"]["access_token"] = json!(token);
    old["tokens"]["id_token"] = json!(token);
    let mut store = strict_store(&f.paths.store_path).unwrap();
    store.accounts[1].auth_json = old.clone();
    fs::write(
        &f.paths.store_path,
        serde_json::to_vec_pretty(&store).unwrap(),
    )
    .unwrap();
    let profile = persistence::target(&f.paths, "profile/b/auth.json").unwrap();
    fs::create_dir_all(profile.parent().unwrap()).unwrap();
    fs::write(&profile, serde_json::to_vec(&old).unwrap()).unwrap();
    let mut fresh = auth("b");
    fresh["tokens"]["refresh_token"] = json!("synthetic-rotated-new-token");
    let expected = fresh.clone();
    let called = std::cell::Cell::new(false);
    let mut fake = FakeLifecycle {
        fail_check: Some(7),
        ..Default::default()
    };
    let result =
        execute_with_injected_refresh(&f.paths, "b", SwitchOptions::default(), &mut fake, |_| {
            called.set(true);
            std::future::ready(Ok(fresh))
        })
        .await;
    assert!(result.is_err());
    assert!(called.get());
    assert!(fake.checks >= 7);
    let stored = strict_store(&f.paths.store_path).unwrap();
    assert_eq!(stored.settings.active_account_id.as_deref(), Some("a"));
    assert_eq!(stored.accounts[1].auth_json, expected);
    assert_eq!(json_file(&profile).unwrap(), expected);
    assert_eq!(
        source_auth,
        fs::read(f.paths.codex_home.join("auth.json")).unwrap()
    );
    assert_eq!(global, fs::read(f.paths.global()).unwrap());
}
#[test]
fn refreshed_identity_mismatch_is_never_persisted() {
    let f = fixture(2, 4, 0);
    let before = tracked_tree(f.paths.codex_home.parent().unwrap());
    let result = execute(
        &f.paths,
        "b",
        SwitchOptions {
            target_auth: Some(auth("c")),
            ..Default::default()
        },
        &mut FakeLifecycle::default(),
    );
    assert!(matches!(result,Err(ref e) if e=="PROJECT_SHARING_IDENTITY_CHANGED"));
    assert_eq!(before, tracked_tree(f.paths.codex_home.parent().unwrap()));
}
#[test]
fn interrupted_refresh_checkpoint_recovers_forward_only() {
    let f = fixture(2, 4, 0);
    let global = fs::read(f.paths.global()).unwrap();
    let source = fs::read(f.paths.codex_home.join("auth.json")).unwrap();
    let plan = make_plan(&f.paths, "b", None).unwrap();
    let mut new_auth = auth("b");
    new_auth["tokens"]["refresh_token"] = json!("synthetic-new-durable-token");
    struct CrashAfterFirst {
        checks: usize,
    }
    impl DesktopLifecycle for CrashAfterFirst {
        fn stop_and_verify(&mut self) -> Result<(), String> {
            Ok(())
        }
        fn assert_stopped(&mut self) -> Result<(), String> {
            self.checks += 1;
            if self.checks == 2 {
                panic!("synthetic checkpoint crash");
            }
            Ok(())
        }
    }
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = persist_refreshed_target(
            &f.paths,
            &plan,
            &new_auth,
            &mut CrashAfterFirst { checks: 0 },
        );
    }));
    assert!(result.is_err());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(f.paths.journal())
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(f.paths.journal().join("journal.json"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
    assert!(recover(&f.paths, &mut FakeLifecycle::default()).unwrap());
    let store = strict_store(&f.paths.store_path).unwrap();
    assert_eq!(store.accounts[1].auth_json, new_auth);
    assert_eq!(store.settings.active_account_id.as_deref(), Some("a"));
    assert_eq!(
        json_file(&persistence::target(&f.paths, "profile/b/auth.json").unwrap()).unwrap(),
        new_auth
    );
    assert_eq!(global, fs::read(f.paths.global()).unwrap());
    assert_eq!(
        source,
        fs::read(f.paths.codex_home.join("auth.json")).unwrap()
    );
}
#[tokio::test]
async fn verification_timeout_never_rolls_back_running_state() {
    let f = fixture(2, 4, 1);
    let mut done = execute(
        &f.paths,
        "b",
        SwitchOptions::default(),
        &mut FakeLifecycle::default(),
    )
    .unwrap();
    let mut newer = json_file(&f.paths.global()).unwrap();
    newer["synthetic-new-ui-state"] = json!("keep");
    fs::write(f.paths.global(), serde_json::to_vec_pretty(&newer).unwrap()).unwrap();
    let before = tracked_tree(f.paths.codex_home.parent().unwrap());
    verify_after_launch(&f.paths, &mut done).await;
    assert_eq!(done.result.status, "pending");
    assert_eq!(
        done.result.warning.as_deref(),
        Some("PROJECT_SHARING_RESTORE_PENDING")
    );
    assert_eq!(before, tracked_tree(f.paths.codex_home.parent().unwrap()));
}
#[test]
fn invalid_store_preview_has_no_repair_side_effects() {
    let f = fixture(2, 4, 0);
    fs::write(&f.paths.store_path, b"{broken synthetic store").unwrap();
    let before = tree(f.paths.codex_home.parent().unwrap());
    assert!(preview(&f.paths, "b").is_err());
    assert_eq!(before, tree(f.paths.codex_home.parent().unwrap()));
    assert!(crate::models::AppSettings::default().preserve_local_projects);
}

#[test]
fn preservation_defaults_on_for_missing_settings_but_respects_explicit_off() {
    assert!(crate::models::AppSettings::default().preserve_local_projects);
    let settings: crate::models::AppSettings = serde_json::from_value(json!({})).unwrap();
    assert!(settings.preserve_local_projects);
    let f = fixture(1, 1, 0);
    for settings in [
        json!({}),
        json!({"preserveLocalProjects":true}),
        json!({"preserveLocalProjects":false}),
    ] {
        let expected = settings
            .get("preserveLocalProjects")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        fs::write(
            &f.paths.store_path,
            serde_json::to_vec(&json!({"settings":settings})).unwrap(),
        )
        .unwrap();
        assert_eq!(enabled_at_path(&f.paths.store_path).unwrap(), expected);
        let decoded: crate::models::AppSettings = serde_json::from_value(settings).unwrap();
        assert_eq!(decoded.preserve_local_projects, expected);
    }
}
#[test]
fn recovery_cleans_only_owned_atomic_temps_and_is_idempotent() {
    let f = fixture(2, 4, 1);
    let before = tracked_tree(f.paths.codex_home.parent().unwrap());
    interrupted(&f);
    let temp = f
        .paths
        .journal()
        .join(format!(".project-switch-{}.tmp", uuid::Uuid::new_v4()));
    fs::write(&temp, b"synthetic partial atomic write").unwrap();
    assert!(recover(&f.paths, &mut FakeLifecycle::default()).unwrap());
    assert!(!f.paths.journal().exists());
    assert!(!recover(&f.paths, &mut FakeLifecycle::default()).unwrap());
    assert_eq!(before, tracked_tree(f.paths.codex_home.parent().unwrap()));
}
#[test]
fn cleanup_rejection_preserves_journal_and_unrecognized_files() {
    let f = fixture(2, 4, 1);
    interrupted(&f);
    let unknown = f
        .paths
        .journal()
        .join(".project-switch-not-a-valid-uuid.tmp");
    fs::write(&unknown, b"synthetic unrelated contents").unwrap();
    assert!(recover(&f.paths, &mut FakeLifecycle::default()).is_err());
    assert!(f.paths.journal().join("journal.json").is_file());
    assert_eq!(fs::read(&unknown).unwrap(), b"synthetic unrelated contents");
    fs::remove_file(unknown).unwrap();
    assert!(recover(&f.paths, &mut FakeLifecycle::default()).unwrap());
    assert!(!recover(&f.paths, &mut FakeLifecycle::default()).unwrap());
}
#[test]
fn recovery_of_empty_journal_directory_is_idempotent() {
    let f = fixture(2, 4, 1);
    let before = tracked_tree(f.paths.codex_home.parent().unwrap());
    fs::create_dir(f.paths.journal()).unwrap();
    assert!(recover(&f.paths, &mut FakeLifecycle::default()).unwrap());
    assert!(!recover(&f.paths, &mut FakeLifecycle::default()).unwrap());
    assert_eq!(before, tracked_tree(f.paths.codex_home.parent().unwrap()));
}
#[test]
fn explicit_openai_route_overrides_fail_without_source_mutation() {
    for config in ["model_provider = \"openai\"\nchatgpt_base_url = \"https://synthetic.invalid/\"\n","model_provider = \"openai\"\n[model_providers.openai]\nbase_url = \"https://synthetic.invalid/\"\n"] {
        let f=fixture(2,4,1);fs::write(f.paths.codex_home.join("config.toml"),config).unwrap();let before=tree(f.paths.codex_home.parent().unwrap());
        assert!(matches!(preview(&f.paths,"b"),Err(ref e) if e=="PROJECT_SHARING_UNSUPPORTED_ROUTE"));
        let mut lifecycle=FakeLifecycle::default();assert!(matches!(execute(&f.paths,"b",SwitchOptions::default(),&mut lifecycle),Err(ref e) if e=="PROJECT_SHARING_UNSUPPORTED_ROUTE"));assert_eq!(lifecycle.stops,0);assert_eq!(before,tree(f.paths.codex_home.parent().unwrap()));
    }
}
#[test]
fn unselected_provider_definition_is_preserved() {
    let f = fixture(2, 4, 1);
    fs::write(f.paths.codex_home.join("config.toml"),"model_provider = \"openai\"\n[model_providers.synthetic_unused]\nbase_url = \"https://synthetic.invalid/\"\n").unwrap();
    assert!(preview(&f.paths, "b").is_ok());
    let _prepared = execute(
        &f.paths,
        "b",
        SwitchOptions::default(),
        &mut FakeLifecycle::default(),
    )
    .unwrap();
    let config = fs::read_to_string(f.paths.codex_home.join("config.toml"))
        .unwrap()
        .parse::<toml_edit::DocumentMut>()
        .unwrap();
    assert_eq!(
        config["model_providers"]["synthetic_unused"]["base_url"].as_str(),
        Some("https://synthetic.invalid/")
    );
}
#[test]
fn pending_transaction_blocks_normal_restart_sync_load_and_save_until_recovery() {
    let f = fixture(2, 4, 1);
    let mut actual = auth("a");
    actual["tokens"]["refresh_token"] = json!("synthetic-source-rotated-before-exit");
    actual["last_refresh"] = json!("2099-01-01T00:00:00Z");
    assert!(crate::auth::has_newer_auth_refresh_snapshot(
        &actual,
        &auth("a")
    ));
    fs::write(
        f.paths.codex_home.join("auth.json"),
        serde_json::to_vec_pretty(&actual).unwrap(),
    )
    .unwrap();
    let original = tracked_tree(f.paths.codex_home.parent().unwrap());
    let original_store = strict_store(&f.paths.store_path).unwrap();
    interrupted(&f);
    assert_eq!(
        json_file(&persistence::target(&f.paths, "profile/a/auth.json").unwrap()).unwrap(),
        actual
    );
    assert_eq!(
        strict_store(&f.paths.store_path).unwrap().accounts[0].auth_json,
        auth("a")
    );
    let interrupted_state = tracked_tree(f.paths.codex_home.parent().unwrap());
    let startup = crate::store::sync_current_auth_account_on_startup_with_auth(
        &f.paths.store_path,
        actual.clone(),
    );
    let loaded = crate::store::load_store_from_path(&f.paths.store_path);
    let saved = crate::store::save_store_to_path(&f.paths.store_path, &original_store);
    assert!(matches!(startup,Err(ref e) if e=="PROJECT_SHARING_RECOVERY_REQUIRED"));
    assert!(matches!(loaded,Err(ref e) if e=="PROJECT_SHARING_RECOVERY_REQUIRED"));
    assert!(matches!(saved,Err(ref e) if e=="PROJECT_SHARING_RECOVERY_REQUIRED"));
    assert_eq!(
        interrupted_state,
        tracked_tree(f.paths.codex_home.parent().unwrap())
    );
    assert!(recover(&f.paths, &mut FakeLifecycle::default()).unwrap());
    assert_eq!(original, tracked_tree(f.paths.codex_home.parent().unwrap()));
}
fn crash_before_first_manifest_rename(f: &Fixture, forward: bool) -> Value {
    let mut next = auth("b");
    next["tokens"]["refresh_token"] = json!("synthetic-durable-forward-token");
    let mut store = strict_store(&f.paths.store_path).unwrap();
    store.accounts[1].auth_json = next.clone();
    if !forward {
        store.settings.active_account_id = Some("b".into());
    }
    let mut changes = vec![
        persistence::change(
            &f.paths,
            "profile/b/auth.json",
            serde_json::to_vec_pretty(&next).unwrap(),
        )
        .unwrap(),
        persistence::change(
            &f.paths,
            "store",
            serde_json::to_vec_pretty(&store).unwrap(),
        )
        .unwrap(),
    ];
    if !forward {
        changes.push(
            persistence::change(&f.paths, "auth", serde_json::to_vec_pretty(&next).unwrap())
                .unwrap(),
        );
    }
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _locks = persistence::locks(&f.paths).unwrap();
        let _ = persistence::apply_with_manifest_writer(
            &f.paths,
            changes,
            &mut FakeLifecycle::default(),
            forward,
            |path, bytes| {
                let temp = path
                    .parent()
                    .unwrap()
                    .join(format!(".project-switch-{}.tmp", uuid::Uuid::new_v4()));
                store_lock::atomic_write(&temp, bytes).unwrap();
                panic!("synthetic crash after manifest fsync before first rename");
            },
        );
    }));
    assert!(result.is_err());
    assert!(!f.paths.journal().join("journal.json").exists());
    next
}
#[test]
fn first_manifest_temp_restores_ordinary_transaction_safely() {
    let f = fixture(2, 4, 1);
    let before = tracked_tree(f.paths.codex_home.parent().unwrap());
    crash_before_first_manifest_rename(&f, false);
    assert!(recover(&f.paths, &mut FakeLifecycle::default()).unwrap());
    assert_eq!(before, tracked_tree(f.paths.codex_home.parent().unwrap()));
    assert!(!recover(&f.paths, &mut FakeLifecycle::default()).unwrap());
}
#[test]
fn first_manifest_temp_preserves_rotated_credentials_by_finishing_forward() {
    let f = fixture(2, 4, 1);
    let source = fs::read(f.paths.codex_home.join("auth.json")).unwrap();
    let global = fs::read(f.paths.global()).unwrap();
    let next = crash_before_first_manifest_rename(&f, true);
    assert!(recover(&f.paths, &mut FakeLifecycle::default()).unwrap());
    let store = strict_store(&f.paths.store_path).unwrap();
    assert_eq!(store.accounts[1].auth_json, next);
    assert_eq!(store.settings.active_account_id.as_deref(), Some("a"));
    assert_eq!(
        json_file(&persistence::target(&f.paths, "profile/b/auth.json").unwrap()).unwrap(),
        next
    );
    assert_eq!(
        source,
        fs::read(f.paths.codex_home.join("auth.json")).unwrap()
    );
    assert_eq!(global, fs::read(f.paths.global()).unwrap());
}
#[test]
fn invalid_or_conflicting_first_manifest_candidates_fail_closed() {
    for conflicting in [false, true] {
        let f = fixture(2, 4, 1);
        crash_before_first_manifest_rename(&f, true);
        let first = fs::read_dir(f.paths.journal())
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let second = f
            .paths
            .journal()
            .join(format!(".project-switch-{}.tmp", uuid::Uuid::new_v4()));
        if conflicting {
            let mut doc: Value = serde_json::from_slice(&fs::read(&first).unwrap()).unwrap();
            doc["complete_on_recovery"] = json!(false);
            store_lock::atomic_write(&second, &serde_json::to_vec(&doc).unwrap()).unwrap();
        } else {
            store_lock::atomic_write(&second, b"synthetic truncated manifest").unwrap();
        }
        let before = tracked_tree(f.paths.codex_home.parent().unwrap());
        assert!(recover(&f.paths, &mut FakeLifecycle::default()).is_err());
        assert_eq!(before, tracked_tree(f.paths.codex_home.parent().unwrap()));
    }
}
#[test]
fn pending_marker_is_bound_to_the_explicit_home_and_store() {
    let f = fixture(2, 4, 1);
    interrupted(&f);
    let other_home = f
        .paths
        .codex_home
        .parent()
        .unwrap()
        .join("synthetic-other-codex-home");
    fs::create_dir(&other_home).unwrap();
    let wrong_home = SwitchPaths::new(other_home, f.paths.store_path.clone()).unwrap();
    let other_data = f
        .paths
        .codex_home
        .parent()
        .unwrap()
        .join("synthetic-other-data-dir");
    fs::create_dir(&other_data).unwrap();
    let other_store = other_data.join("accounts.json");
    fs::write(&other_store, fs::read(&f.paths.store_path).unwrap()).unwrap();
    let wrong_store = SwitchPaths::new(f.paths.codex_home.clone(), other_store).unwrap();
    let before = tracked_tree(f.paths.codex_home.parent().unwrap());
    assert!(recover(&wrong_home, &mut FakeLifecycle::default()).is_err());
    assert!(recover(&wrong_store, &mut FakeLifecycle::default()).is_err());
    assert_eq!(before, tracked_tree(f.paths.codex_home.parent().unwrap()));
    assert!(
        matches!(crate::store::sync_current_auth_account_on_startup_in_path(&f.paths.store_path),Err(ref e) if e=="PROJECT_SHARING_RECOVERY_REQUIRED")
    );
    assert!(recover(&f.paths, &mut FakeLifecycle::default()).unwrap());
    assert!(store_lock::ensure_store_available(&f.paths.store_path).is_ok());
}

fn assert_database_scope_rejected_without_writes(f: &Fixture) {
    let before = tree(f.paths.codex_home.parent().unwrap());
    assert!(matches!(preview(&f.paths,"b"),Err(ref e) if e=="PROJECT_SHARING_DATABASE_MISMATCH"));
    let mut lifecycle = FakeLifecycle::default();
    assert!(
        matches!(execute(&f.paths,"b",SwitchOptions::default(),&mut lifecycle),Err(ref e) if e=="PROJECT_SHARING_DATABASE_MISMATCH")
    );
    assert_eq!(lifecycle.stops, 0);
    assert_eq!(before, tree(f.paths.codex_home.parent().unwrap()));
}

#[test]
fn missing_database_projects_and_unknown_thread_schema_still_fail_closed() {
    for sql in [
        "DELETE FROM projects WHERE id='synthetic-server-0'",
        "ALTER TABLE threads RENAME COLUMN id TO unsupported_id",
        "DROP TABLE threads",
    ] {
        let f = fixture(2, 4, 1);
        let db = rusqlite::Connection::open(f.paths.codex_home.join("state_5.sqlite")).unwrap();
        db.execute_batch(sql).unwrap();
        drop(db);
        assert_database_scope_rejected_without_writes(&f);
    }
}
#[test]
fn database_cloud_metadata_cannot_rehydrate_bindings_into_a_pure_json_project() {
    for key in [
        "codex.chatGptBacking",
        "codex.chatGptBackingCreation",
        "codex.retiredChatGptBacking",
    ] {
        let f = fixture(2, 4, 1);
        let db = rusqlite::Connection::open(f.paths.codex_home.join("state_5.sqlite")).unwrap();
        let metadata = json!({key:{"accountId":"synthetic-other-account","projectId":"synthetic-cloud-project"}});
        db.execute(
            "UPDATE projects SET metadata=?1 WHERE id='synthetic-server-0'",
            [metadata.to_string()],
        )
        .unwrap();
        drop(db);
        let project =
            &json_file(&f.paths.global()).unwrap()["local-projects"]["synthetic-project-0"];
        assert!(project.get("chatGptBacking").is_none());
        assert_database_scope_rejected_without_writes(&f);
    }
}
#[test]
fn database_metadata_requires_known_object_schema_and_matching_legacy_id() {
    for metadata in [
        "{synthetic broken JSON",
        "[]",
        "null",
        r#"{"codex.localProjectId":"synthetic-wrong-legacy"}"#,
    ] {
        let f = fixture(2, 4, 1);
        let db = rusqlite::Connection::open(f.paths.codex_home.join("state_5.sqlite")).unwrap();
        db.execute(
            "UPDATE projects SET metadata=?1 WHERE id='synthetic-server-0'",
            [metadata],
        )
        .unwrap();
        drop(db);
        assert_database_scope_rejected_without_writes(&f);
    }
    let f = fixture(2, 4, 1);
    let db = rusqlite::Connection::open(f.paths.codex_home.join("state_5.sqlite")).unwrap();
    db.execute("ALTER TABLE projects DROP COLUMN metadata", [])
        .unwrap();
    drop(db);
    assert_database_scope_rejected_without_writes(&f);
}
#[test]
fn matching_local_id_and_cosmetic_database_metadata_remain_unchanged() {
    let f = fixture(2, 4, 1);
    let db_path = f.paths.codex_home.join("state_5.sqlite");
    let db = rusqlite::Connection::open(&db_path).unwrap();
    let metadata = json!({"codex.localProjectId":"synthetic-project-0","color":"synthetic-purple","custom":{"label":"synthetic"},"codex.chatGptBacking":null});
    db.execute(
        "UPDATE projects SET metadata=?1 WHERE id='synthetic-server-0'",
        [metadata.to_string()],
    )
    .unwrap();
    drop(db);
    let before = fs::read(&db_path).unwrap();
    assert!(preview(&f.paths, "b").is_ok());
    let _prepared = execute(
        &f.paths,
        "b",
        SwitchOptions::default(),
        &mut FakeLifecycle::default(),
    )
    .unwrap();
    assert_eq!(before, fs::read(db_path).unwrap());
}
