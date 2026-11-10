use super::*;

#[test]
fn nullable_owner_and_incompatible_legacy_registry_do_not_block_saved_session_copy() {
    let f = fixture(2, 4, 1);
    let mut original = json_file(&f.paths.global()).unwrap();
    let globals = original
        .as_object()
        .unwrap()
        .iter()
        .filter(|(key, _)| key.as_str() != "electron-account-renderer-state-v1")
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<serde_json::Map<_, _>>();
    let source = json!({"atoms": {"composer-prompt-drafts-v2": {"draft": "Final draft"}}, "globals": globals});
    let previous_target =
        json!({"atoms": {"target": true}, "globals": {"local-projects": ["old shape"]}});
    original["electron-account-renderer-state-v1"] =
        json!({"owner":null,"inactive":{&f.owners[0]:source,&f.owners[1]:previous_target}});
    for key in globals.keys() {
        original.as_object_mut().unwrap().remove(key);
    }
    fs::write(f.paths.global(), serde_json::to_vec(&original).unwrap()).unwrap();
    fs::write(f.paths.registry(), b"unsupported legacy registry").unwrap();
    let preview = preview(&f.paths, "b").unwrap();
    assert_eq!(preview.project_count, 2);
    assert!(!preview.requires_merge_confirmation);
    let result = execute(
        &f.paths,
        "b",
        SwitchOptions::default(),
        &mut FakeLifecycle::default(),
    )
    .unwrap();
    assert_eq!(result.result.status, "prepared");
    let written = json_file(&f.paths.global()).unwrap();
    assert_eq!(
        written["electron-account-renderer-state-v1"]["inactive"][&f.owners[1]],
        source
    );
    assert_eq!(
        json_file(&persistence::target(&f.paths, "profile/a/desktop-state.json").unwrap()).unwrap(),
        source
    );
    assert_eq!(
        json_file(&persistence::target(&f.paths, "profile/b/desktop-state.previous.json").unwrap())
            .unwrap(),
        previous_target
    );
    assert_eq!(
        fs::read(f.paths.registry()).unwrap(),
        b"unsupported legacy registry"
    );
}

#[test]
fn missing_renderer_envelope_copies_existing_state() {
    let f = fixture(2, 4, 1);
    let mut original = json_file(&f.paths.global()).unwrap();
    original
        .as_object_mut()
        .unwrap()
        .remove("electron-account-renderer-state-v1");
    original["local-projects"]["synthetic-project-0"]["new-schema-field"] = json!([1, 2]);
    original["project-order"] = json!(["synthetic-project-0", "synthetic-project-0", "orphan"]);
    fs::write(f.paths.global(), serde_json::to_vec(&original).unwrap()).unwrap();
    execute(
        &f.paths,
        "b",
        SwitchOptions::default(),
        &mut FakeLifecycle::default(),
    )
    .unwrap();
    let written = json_file(&f.paths.global()).unwrap();
    let copied =
        &written["electron-account-renderer-state-v1"]["inactive"][&f.owners[1]]["globals"];
    assert_eq!(copied["local-projects"], original["local-projects"]);
    assert_eq!(copied["project-order"], original["project-order"]);
}

#[test]
fn missing_global_state_does_not_block_account_switch() {
    let f = fixture(0, 0, 0);
    fs::remove_file(f.paths.global()).unwrap();
    let preview = preview(&f.paths, "b").unwrap();
    assert_eq!(preview.project_count, 0);
    execute(
        &f.paths,
        "b",
        SwitchOptions::default(),
        &mut FakeLifecycle::default(),
    )
    .unwrap();
    let written = json_file(&f.paths.global()).unwrap();
    assert_eq!(
        written["electron-account-renderer-state-v1"]["inactive"][&f.owners[1]],
        json!({"atoms":{},"globals":{}})
    );
}

struct LifecycleProbe<S: FnMut(), C: FnMut()> {
    stop: S,
    check: C,
}

impl<S: FnMut(), C: FnMut()> DesktopLifecycle for LifecycleProbe<S, C> {
    fn stop_and_verify(&mut self) -> Result<(), String> {
        (self.stop)();
        Ok(())
    }
    fn assert_stopped(&mut self) -> Result<(), String> {
        (self.check)();
        Ok(())
    }
}

#[test]
fn shutdown_archived_owner_is_saved_and_copied_after_the_final_flush() {
    let f = fixture(2, 4, 1);
    let preview = preview(&f.paths, "b").unwrap();
    let mut final_document = json_file(&f.paths.global()).unwrap();
    let globals = final_document
        .as_object()
        .unwrap()
        .iter()
        .filter(|(key, _)| key.as_str() != "electron-account-renderer-state-v1")
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<serde_json::Map<_, _>>();
    let final_session = json!({"atoms":{"composer-prompt-drafts-v2":{"draft":"Saved while exiting"}},"globals":globals});
    for key in globals.keys() {
        final_document.as_object_mut().unwrap().remove(key);
    }
    final_document["electron-account-renderer-state-v1"]["owner"] = Value::Null;
    final_document["electron-account-renderer-state-v1"]["inactive"][&f.owners[0]] =
        final_session.clone();
    let mut lifecycle = LifecycleProbe {
        stop: || {
            fs::write(
                f.paths.global(),
                serde_json::to_vec(&final_document).unwrap(),
            )
            .unwrap()
        },
        check: || {},
    };
    execute(
        &f.paths,
        "b",
        SwitchOptions {
            preview_fingerprint: Some(preview.preview_fingerprint),
            ..Default::default()
        },
        &mut lifecycle,
    )
    .unwrap();
    assert_eq!(
        json_file(&persistence::target(&f.paths, "profile/a/desktop-state.json").unwrap()).unwrap(),
        final_session
    );
    assert_eq!(
        json_file(&f.paths.global()).unwrap()["electron-account-renderer-state-v1"]["inactive"]
            [&f.owners[1]],
        final_session
    );
}

#[test]
fn source_and_target_checkpoints_precede_global_state_overwrite() {
    let f = fixture(2, 4, 1);
    let mut original = json_file(&f.paths.global()).unwrap();
    let previous_target = json!({"atoms":{"draft":"Target draft"},"globals":{"local-projects":{}}});
    original["electron-account-renderer-state-v1"]["inactive"][&f.owners[1]] =
        previous_target.clone();
    let original_bytes = serde_json::to_vec(&original).unwrap();
    fs::write(f.paths.global(), &original_bytes).unwrap();
    let observed = std::cell::Cell::new(false);
    let mut lifecycle = LifecycleProbe {
        stop: || {},
        check: || {
            if fs::read(f.paths.global()).unwrap() != original_bytes {
                observed.set(true);
                let source_path =
                    persistence::target(&f.paths, "profile/a/desktop-state.json").unwrap();
                assert!(source_path.is_file());
                assert_eq!(
                    json_file(
                        &persistence::target(&f.paths, "profile/b/desktop-state.previous.json")
                            .unwrap()
                    )
                    .unwrap(),
                    previous_target
                );
            }
        },
    };
    execute(&f.paths, "b", SwitchOptions::default(), &mut lifecycle).unwrap();
    assert!(observed.get());
}
