use super::*;

fn owner(letter: char) -> String {
    letter.to_string().repeat(64)
}

fn document() -> Value {
    json!({
        STATE: {"owner": owner('a'), "inactive": {
            owner('b'): {"atoms": {"composer-prompt-drafts-v2": "old draft"},
                "globals": {"local-projects": {"target-only": {"name": "Old"}}}},
            owner('c'): {"atoms": {"keep": true}, "globals": {"keep": true}}
        }},
        ATOMS: {"composer-prompt-drafts-v2": {"draft": "Current draft"},
            "thread-tab-routes-v1": {"thread-a": "files"},
            "last_completed_onboarding": "device setting"},
        "local-projects": {"p": {"id": "legacy-id", "name": "Source", "futureField": true}},
        "thread-project-assignments": {"thread-a": {"projectId": "missing", "futureField": 7}},
        "projectless-thread-ids": ["thread-a"],
        "project-order": ["p", "p", "old"],
        "pinned-thread-ids": ["thread-a"],
        "electron-main-window-bounds": {"width": 1200}
    })
}

#[test]
fn source_session_overwrites_target_without_schema_or_merge_rejection() {
    let original = document();
    let plan = plan_session_overwrite(&original, &owner('a'), &owner('b')).unwrap();
    let target = &plan.document[STATE]["inactive"][owner('b')];
    assert_eq!(target["atoms"], json!(plan.source.atoms));
    for key in [
        "local-projects",
        "thread-project-assignments",
        "projectless-thread-ids",
        "project-order",
        "pinned-thread-ids",
    ] {
        assert_eq!(target["globals"][key], original[key]);
    }
    assert!(target["globals"]["local-projects"]
        .get("target-only")
        .is_none());
    assert_eq!(
        plan.previous_target,
        Some(original[STATE]["inactive"][owner('b')].clone())
    );
    assert_eq!(
        plan.document[STATE]["inactive"][owner('c')],
        original[STATE]["inactive"][owner('c')]
    );
    assert_eq!(
        plan.document["electron-main-window-bounds"],
        original["electron-main-window-bounds"]
    );
    assert_eq!(
        plan.document[ATOMS]["last_completed_onboarding"],
        "device setting"
    );
    assert!(!plan.source.atoms.contains_key("last_completed_onboarding"));
}

#[test]
fn nullable_owner_restores_the_source_archived_by_desktop_shutdown() {
    let mut original = document();
    original[STATE]["owner"] = Value::Null;
    original[STATE]["inactive"][owner('a')] = json!({
        "atoms": {"composer-prompt-drafts-v2": {"draft": "Saved on exit"}},
        "globals": {"local-projects": {"archived": {"futureShape": [1,2]}},
            "future-session-field": {"keep": true}}
    });
    original.as_object_mut().unwrap().remove("local-projects");
    original
        .as_object_mut()
        .unwrap()
        .remove("thread-project-assignments");
    let plan = plan_session_overwrite(&original, &owner('a'), &owner('b')).unwrap();
    assert_eq!(
        plan.source.globals["local-projects"],
        original[STATE]["inactive"][owner('a')]["globals"]["local-projects"]
    );
    assert_eq!(
        plan.source.atoms["composer-prompt-drafts-v2"]["draft"],
        "Saved on exit"
    );
    assert_eq!(
        plan.document["local-projects"],
        plan.source.globals["local-projects"]
    );
    assert_eq!(plan.document[STATE]["owner"], owner('a'));
    assert_eq!(
        plan.document[STATE]["inactive"][owner('a')],
        plan.document[STATE]["inactive"][owner('b')]
    );
}

#[test]
fn old_missing_envelope_and_malformed_target_are_replaced_with_valid_envelopes() {
    let mut original = document();
    original.as_object_mut().unwrap().remove(STATE);
    original["electron-account-renderer-state-v2"] = json!({"keep": "opaque"});
    let plan = plan_session_overwrite(&original, &owner('a'), &owner('b')).unwrap();
    assert_eq!(plan.document[STATE]["owner"], owner('a'));
    assert_eq!(
        plan.document[STATE]["inactive"][owner('b')]["globals"]["local-projects"],
        original["local-projects"]
    );
    assert_eq!(
        plan.document["electron-account-renderer-state-v2"],
        original["electron-account-renderer-state-v2"]
    );
    let mut original = document();
    original[STATE]["inactive"][owner('b')] = json!("legacy target state");
    let plan = plan_session_overwrite(&original, &owner('a'), &owner('b')).unwrap();
    assert_eq!(plan.previous_target, Some(json!("legacy target state")));
    assert!(plan.document[STATE]["inactive"][owner('b')]["atoms"].is_object());
}

#[test]
fn selecting_the_current_owner_is_a_noop() {
    let original = document();
    let plan = plan_session_overwrite(&original, &owner('a'), &owner('a')).unwrap();
    assert!(plan.no_op);
    assert_eq!(plan.document, original);
}

#[test]
fn restored_check_accepts_opaque_project_data_and_checks_drafts() {
    let plan = plan_session_overwrite(&document(), &owner('a'), &owner('b')).unwrap();
    let mut restored = plan.document;
    restored[STATE]["owner"] = json!(owner('b'));
    assert!(plan.source.is_restored(&restored, &owner('b')));
    restored[ATOMS]["composer-prompt-drafts-v2"] = json!({"draft": "different"});
    assert!(!plan.source.is_restored(&restored, &owner('b')));
}
