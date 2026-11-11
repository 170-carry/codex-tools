use super::*;

#[test]
fn preview_uses_one_snapshot_while_desktop_updates_globals() {
    let f = fixture(2, 4, 1);
    let original = json_file(&f.paths.global()).unwrap();
    let mut updated = original.clone();
    updated["electron-persisted-atom-state"] = json!({"draft": "synthetic latest draft"});
    let mut reads = BTreeMap::new();
    let plan = make_plan_with_reader(&f.paths, "b", None, &mut |path| {
        *reads.entry(path.to_path_buf()).or_insert(0) += 1;
        let bytes = store_lock::read_optional(path)?;
        if path == f.paths.global() {
            fs::write(path, serde_json::to_vec(&updated).unwrap()).unwrap();
        }
        Ok(bytes)
    })
    .unwrap();

    assert!(reads.values().all(|count| *count == 1));
    assert_eq!(plan.preview.project_count, 2);
    assert_eq!(plan.preview.thread_link_count, 4);
    assert_eq!(plan.preview.unassigned_thread_count, 1);
    assert_eq!(plan.sharing.document["unrelated"], original["unrelated"]);
    assert_eq!(
        plan.sharing.document["electron-persisted-atom-state"],
        json!({})
    );
    assert_eq!(json_file(&f.paths.global()).unwrap(), updated);
    assert_eq!(
        plan.inputs.ensure_unchanged().unwrap_err(),
        "PROJECT_SHARING_SYNC_FAILED"
    );
    assert_eq!(
        preview(&f.paths, "b").unwrap().preview_fingerprint,
        plan.preview.preview_fingerprint
    );
    assert!(!f.paths.journal().exists());
}

struct FlushOnStop<F: FnMut()> {
    action: F,
    inner: FakeLifecycle,
}

impl<F: FnMut()> DesktopLifecycle for FlushOnStop<F> {
    fn stop_and_verify(&mut self) -> Result<(), String> {
        self.inner.stop_and_verify()?;
        (self.action)();
        Ok(())
    }

    fn assert_stopped(&mut self) -> Result<(), String> {
        self.inner.assert_stopped()
    }
}

#[test]
fn switch_preserves_final_desktop_flush_and_rotated_source_credentials() {
    let f = fixture(2, 4, 1);
    let preview = preview(&f.paths, "b").unwrap();
    let mut latest_auth = auth("a");
    latest_auth["last_refresh"] = json!("synthetic final refresh");
    let mut lifecycle = FlushOnStop {
        action: || {
            let mut document = json_file(&f.paths.global()).unwrap();
            document["electron-persisted-atom-state"] = json!({"draft": "synthetic final draft"});
            fs::write(f.paths.global(), serde_json::to_vec(&document).unwrap()).unwrap();
            fs::write(
                f.paths.codex_home.join("auth.json"),
                serde_json::to_vec(&latest_auth).unwrap(),
            )
            .unwrap();
        },
        inner: FakeLifecycle::default(),
    };
    let done = execute(
        &f.paths,
        "b",
        SwitchOptions {
            preview_fingerprint: Some(preview.preview_fingerprint),
            ..Default::default()
        },
        &mut lifecycle,
    )
    .unwrap();

    assert_eq!(lifecycle.inner.stops, 1);
    assert_eq!(done.result.status, "prepared");
    assert_eq!(
        json_file(&f.paths.global()).unwrap()["electron-persisted-atom-state"]["draft"],
        "synthetic final draft"
    );
    assert_eq!(
        json_file(&persistence::target(&f.paths, "profile/a/auth.json").unwrap()).unwrap(),
        latest_auth
    );
    assert_eq!(
        json_file(&f.paths.codex_home.join("auth.json")).unwrap(),
        auth("b")
    );
    assert!(!f.paths.journal().exists());
}

#[test]
fn final_project_changes_are_saved_and_copied_in_force_mode() {
    let f = fixture(2, 4, 1);
    let preview = preview(&f.paths, "b").unwrap();
    let mut latest = json_file(&f.paths.global()).unwrap();
    latest["local-projects"]["synthetic-project-0"]["name"] = json!("Final project name");
    let mut lifecycle = FlushOnStop {
        action: || {
            fs::write(f.paths.global(), serde_json::to_vec(&latest).unwrap()).unwrap();
        },
        inner: FakeLifecycle::default(),
    };
    let result = execute(
        &f.paths,
        "b",
        SwitchOptions {
            preview_fingerprint: Some(preview.preview_fingerprint),
            ..Default::default()
        },
        &mut lifecycle,
    );

    assert_eq!(result.unwrap().result.status, "prepared");
    let written = json_file(&f.paths.global()).unwrap();
    assert_eq!(
        written["electron-account-renderer-state-v1"]["inactive"][&f.owners[1]]["globals"]
            ["local-projects"],
        latest["local-projects"]
    );
    let saved =
        json_file(&persistence::target(&f.paths, "profile/a/desktop-state.json").unwrap()).unwrap();
    assert_eq!(saved["globals"]["local-projects"], latest["local-projects"]);
    assert_eq!(
        json_file(&f.paths.codex_home.join("auth.json")).unwrap(),
        auth("b")
    );
    assert!(!f.paths.journal().exists());
}
