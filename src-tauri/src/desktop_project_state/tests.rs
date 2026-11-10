use super::*;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde_json::json;

#[path = "stale_metadata_tests.rs"]
mod stale_metadata_tests;

fn token(claims: Value) -> String {
    format!(
        "e30.{}.synthetic",
        URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims).unwrap())
    )
}

fn auth(user: &str, account: &str) -> Value {
    json!({"tokens": {
        "account_id": account,
        "id_token": token(json!({"https://api.openai.com/auth": {
            "chatgpt_user_id": user, "chatgpt_account_id": account
        }}))
    }})
}

#[test]
fn owner_uses_compact_javascript_identity_tuple() {
    // SHA-256 of the exact UTF-8 bytes ["user-a","account-a"].
    assert_eq!(
        owner_from_auth(&auth("user-a", "account-a")).unwrap(),
        "567b1ccd06bd57086baaf486a2b8bb6f3bc056178d6ea5109162d06cd42b62f1"
    );
}

const HOST: &str = "local:/synthetic/codex-home";
const STATE: &str = "electron-account-renderer-state-v1";

fn owner(letter: char) -> String {
    letter.to_string().repeat(64)
}

fn project(id: &str, name: &str) -> Value {
    json!({"id": id, "name": name, "rootPaths": [format!("/synthetic/{id}")],
        "createdAt": 1000, "updatedAt": 1000})
}

fn globals(id: &str, name: &str, thread: &str) -> Value {
    json!({
        "local-projects": {id: project(id, name)},
        "thread-project-assignments": {thread: {"projectKind":"local", "projectId":id}},
        "thread-project-membership-host-ids": {thread:"local"},
        "projectless-thread-ids": [],
        "project-order": [id], "pinned-project-ids": [id],
        "project-appearances": {id:{"color":"blue", "icon":"folder"}},
        "sidebar-project-thread-orders": {id: {"threadIds":[thread]}},
        "app-server-project-id-by-legacy-project-id-by-host": {HOST:{id:format!("server-{id}")}},
        "trusted-root-permissions": {format!("/synthetic/{id}"):"read"},
        "migration-complete": true,
        "unrelated": {"keep":id}
    })
}

fn document(source: Value, targets: Vec<(String, Value)>) -> Value {
    let mut document = source;
    let inactive = targets
        .into_iter()
        .map(|(owner, globals)| {
            (
                owner.clone(),
                json!({"atoms":{"accountSecret":owner},"globals":globals,"extra":"keep"}),
            )
        })
        .collect::<serde_json::Map<_, _>>();
    document[STATE] = json!({"owner": owner('a'), "inactive": inactive,"futureField":"retain"});
    document["global-unknown"] = json!({"must":"remain"});
    document
}

// Independent fixture of the observed desktop owner-selection contract. The
// planner never calls this: it must leave active globals/owner untouched.
fn select_owner(document: &Value, next: &str) -> Value {
    let mut selected = document.clone();
    let old = selected[STATE]["owner"].as_str().unwrap().to_owned();
    let scoped_keys = [
        "local-projects",
        "thread-project-assignments",
        "projectless-thread-ids",
        "thread-project-membership-host-ids",
        "project-order",
        "pinned-project-ids",
        "project-appearances",
        "sidebar-project-thread-orders",
        "app-server-project-id-by-legacy-project-id-by-host",
        "trusted-root-permissions",
        "migration-complete",
        "unrelated",
    ];
    let mut archive = serde_json::Map::new();
    for key in scoped_keys {
        if let Some(value) = selected.as_object_mut().unwrap().remove(key) {
            archive.insert(key.to_owned(), value);
        }
    }
    selected[STATE]["inactive"][&old] = json!({"atoms":{},"globals":archive});
    let next_state = selected[STATE]["inactive"]
        .as_object_mut()
        .unwrap()
        .remove(next)
        .unwrap_or_else(|| json!({"atoms":{},"globals":{}}));
    for (key, value) in next_state["globals"].as_object().unwrap() {
        selected[key] = value.clone();
    }
    selected[STATE]["owner"] = json!(next);
    selected
}

fn ids(document: &Value) -> BTreeSet<String> {
    document["local-projects"]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect()
}

#[test]
fn identity_requires_consistent_token_claims_and_never_uses_email() {
    let mut valid = auth("user-a", "account-a");
    valid["tokens"]["access_token"] = json!(token(json!({"https://api.openai.com/auth":{
        "user_id":"user-a", "chatgpt_account_id":"account-a"
    }})));
    assert_eq!(
        owner_from_auth(&valid).unwrap(),
        owner_from_auth(&auth("user-a", "account-a")).unwrap()
    );
    valid["tokens"]["access_token"] = json!(token(json!({"https://api.openai.com/auth":{
        "user_id":"different", "chatgpt_account_id":"account-a"
    }})));
    assert!(owner_from_auth(&valid).is_err());
    assert!(
        owner_from_auth(&json!({"email":"a@synthetic.test","account_id":"account-a"})).is_err()
    );
    assert!(
        owner_from_auth(&json!({"tokens":{"id_token":"malformed","account_id":"account-a"}}))
            .is_err()
    );
    let mut mismatch = auth("user-a", "account-a");
    mismatch["tokens"]["account_id"] = json!("different");
    assert!(owner_from_auth(&mismatch).is_err());
}

#[test]
fn first_share_preserves_active_state_atoms_and_unrelated_target_values() {
    let original = document(
        globals("p", "Alpha", "t1"),
        vec![
            (owner('b'), globals("q", "Beta", "t2")),
            (owner('c'), globals("z", "Uninvolved", "t3")),
        ],
    );
    let plan = plan_share(&original, &owner('a'), &owner('b'), HOST, None).unwrap();
    for (key, value) in original.as_object().unwrap() {
        if key != STATE {
            assert_eq!(&plan.document[key], value, "active key {key}");
        }
    }
    assert_eq!(plan.document[STATE]["owner"], original[STATE]["owner"]);
    assert_eq!(
        plan.document[STATE]["inactive"][owner('c')],
        original[STATE]["inactive"][owner('c')]
    );
    assert_eq!(
        plan.document[STATE]["inactive"][owner('b')]["atoms"],
        original[STATE]["inactive"][owner('b')]["atoms"]
    );
    let target = &plan.document[STATE]["inactive"][owner('b')]["globals"];
    assert_eq!(
        target["trusted-root-permissions"],
        original[STATE]["inactive"][owner('b')]["globals"]["trusted-root-permissions"]
    );
    assert_eq!(target["unrelated"], json!({"keep":"q"}));
    let selected = select_owner(&plan.document, &owner('b'));
    assert_eq!(ids(&selected), BTreeSet::from(["p".into(), "q".into()]));
    assert_eq!(
        selected["thread-project-assignments"],
        json!({
        "t1":{"projectKind":"local","projectId":"p"},
        "t2":{"projectKind":"local","projectId":"q"}})
    );
    // A was only read, so its baseline must not pretend q has been applied.
    assert_eq!(plan.registry.baselines[&owner('a')].projects.len(), 1);
    assert_eq!(plan.registry.baselines[&owner('b')].projects.len(), 2);
    assert_eq!(plan.summary.retained_target_project_ids, vec!["q"]);
}

#[test]
fn blank_target_is_seeded_and_a_b_c_a_roundtrip_keeps_exact_memberships() {
    let mut source = globals("p", "Alpha", "t1");
    source["local-projects"]["p"]["rootPaths"] = json!(["/synthetic/p", "/synthetic/second-root"]);
    source["thread-project-assignments"]["legacy-thread"] =
        json!({"projectKind":"local", "projectId":"p"});
    let original = document(source, vec![]);
    let ab = plan_share(&original, &owner('a'), &owner('b'), HOST, None).unwrap();
    let b = select_owner(&ab.document, &owner('b'));
    let bc = plan_share(&b, &owner('b'), &owner('c'), HOST, Some(&ab.registry)).unwrap();
    let c = select_owner(&bc.document, &owner('c'));
    let ca = plan_share(&c, &owner('c'), &owner('a'), HOST, Some(&bc.registry)).unwrap();
    let a = select_owner(&ca.document, &owner('a'));
    assert_eq!(ids(&a), ids(&original));
    assert_eq!(
        a["local-projects"]["p"]["rootPaths"],
        original["local-projects"]["p"]["rootPaths"]
    );
    assert_eq!(
        a["thread-project-assignments"],
        original["thread-project-assignments"]
    );
    assert_eq!(
        a["thread-project-membership-host-ids"],
        original["thread-project-membership-host-ids"]
    );
    assert_eq!(
        a["thread-project-membership-host-ids"]["t1"],
        json!("local")
    );
    assert!(a["thread-project-membership-host-ids"]
        .get("legacy-thread")
        .is_none());
    assert_eq!(a["projectless-thread-ids"], json!([]));
}

#[test]
fn source_create_rename_move_unassign_and_delete_propagate_without_resurrection() {
    let original = document(globals("p", "Alpha", "t1"), vec![]);
    let ab = plan_share(&original, &owner('a'), &owner('b'), HOST, None).unwrap();
    let mut b = select_owner(&ab.document, &owner('b'));
    b["local-projects"]["p"]["name"] = json!("Renamed");
    b["local-projects"]["p"]["updatedAt"] = json!(2000);
    b["local-projects"]["q"] = project("q", "Created in B");
    b["thread-project-assignments"]["t1"]["projectId"] = json!("q");
    b["sidebar-project-thread-orders"] = json!({"q":{"threadIds":["t1"]}});
    b["project-order"] = json!(["q", "p"]);
    let bc = plan_share(&b, &owner('b'), &owner('c'), HOST, Some(&ab.registry)).unwrap();
    let mut c = select_owner(&bc.document, &owner('c'));
    assert_eq!(c["local-projects"]["p"]["name"], json!("Renamed"));
    assert_eq!(
        c["thread-project-assignments"]["t1"]["projectId"],
        json!("q")
    );
    assert_eq!(c["project-order"], json!(["q", "p"]));
    c["thread-project-assignments"] = json!({});
    c["projectless-thread-ids"] = json!(["t1"]);
    c["sidebar-project-thread-orders"] = json!({});
    c["local-projects"].as_object_mut().unwrap().remove("p");
    c["project-order"] = json!(["q"]);
    // Stale UI metadata for a deleted known project is cleaned in the target.
    let ca = plan_share(&c, &owner('c'), &owner('a'), HOST, Some(&bc.registry)).unwrap();
    let a = select_owner(&ca.document, &owner('a'));
    assert_eq!(ids(&a), BTreeSet::from(["q".into()]));
    assert_eq!(a["thread-project-assignments"], json!({}));
    assert_eq!(a["projectless-thread-ids"], json!(["t1"]));
    assert!(a["project-appearances"].get("p").is_none());
    assert!(
        a["app-server-project-id-by-legacy-project-id-by-host"][HOST]
            .get("p")
            .is_none()
    );
    assert!(ca.registry.deletions["projects"].contains("p"));
    let mut with_stale_new_target = a.clone();
    with_stale_new_target[STATE]["inactive"][owner('d')] =
        json!({"atoms":{},"globals":globals("p","Alpha","t1")});
    assert!(plan_share(
        &with_stale_new_target,
        &owner('a'),
        &owner('d'),
        HOST,
        Some(&ca.registry)
    )
    .is_err());
}

#[test]
fn project_deletion_cleans_memberships_and_preserves_unrelated_cloud_records() {
    let mut source = globals("p", "Alpha", "t1");
    source["local-projects"]["cloud"] = project("cloud", "Cloud");
    source["local-projects"]["cloud"]["chatGptBacking"] = json!({"id":"cloud-binding"});
    source["thread-project-assignments"]["cloud-thread"] =
        json!({"projectKind":"local","projectId":"cloud"});
    let original = document(source, vec![(owner('b'), globals("q", "Beta", "t2"))]);
    let ab = plan_share(&original, &owner('a'), &owner('b'), HOST, None).unwrap();
    let mut b = select_owner(&ab.document, &owner('b'));
    b["local-projects"].as_object_mut().unwrap().remove("p");
    b["thread-project-assignments"]
        .as_object_mut()
        .unwrap()
        .remove("t1");
    b["projectless-thread-ids"] = json!(["t1"]);
    let ba = plan_share(&b, &owner('b'), &owner('a'), HOST, Some(&ab.registry)).unwrap();
    let a = select_owner(&ba.document, &owner('a'));
    assert_eq!(
        a["local-projects"]["cloud"],
        original["local-projects"]["cloud"]
    );
    assert_eq!(
        a["thread-project-assignments"]["cloud-thread"],
        original["thread-project-assignments"]["cloud-thread"]
    );
    assert!(a["thread-project-assignments"].get("t1").is_none());
    assert_eq!(a["projectless-thread-ids"], json!(["t1"]));
}

#[test]
fn conflicting_same_id_and_divergent_edits_fail_without_changing_inputs() {
    let original = document(
        globals("p", "Alpha", "t1"),
        vec![(owner('b'), globals("p", "Other", "t1"))],
    );
    assert!(plan_share(&original, &owner('a'), &owner('b'), HOST, None).is_err());
    let original = document(globals("p", "Alpha", "t1"), vec![]);
    let ab = plan_share(&original, &owner('a'), &owner('b'), HOST, None).unwrap();
    let mut b = select_owner(&ab.document, &owner('b'));
    b["local-projects"]["p"]["name"] = json!("B changed");
    b[STATE]["inactive"][owner('a')]["globals"]["local-projects"]["p"]["name"] = json!("A changed");
    let before = b.clone();
    let registry_before = ab.registry.clone();
    assert!(plan_share(&b, &owner('b'), &owner('a'), HOST, Some(&ab.registry)).is_err());
    assert_eq!(b, before);
    assert_eq!(ab.registry, registry_before);
}

#[test]
fn distinct_ids_with_equal_names_and_paths_remain_distinct() {
    let mut target = globals("q", "Same", "t2");
    target["local-projects"]["q"]["rootPaths"] = json!(["/synthetic/p"]);
    let original = document(globals("p", "Same", "t1"), vec![(owner('b'), target)]);
    let plan = plan_share(&original, &owner('a'), &owner('b'), HOST, None).unwrap();
    assert_eq!(ids(&select_owner(&plan.document, &owner('b'))).len(), 2);
}

#[test]
fn only_pure_local_whitelist_and_current_host_mapping_are_shared() {
    let mut source = globals("p", "Alpha", "t1");
    source["local-projects"]["p"]["writableRoots"] = json!(["/synthetic/secret"]);
    source["local-projects"]["p"]["permission"] = json!("full");
    for field in [
        "chatGptBacking",
        "chatGptBackingCreation",
        "retiredChatGptBacking",
    ] {
        source["local-projects"][field] = project(field, field);
        source["local-projects"][field][field] = json!({"binding":"private"});
    }
    source["thread-project-assignments"]["remote-thread"] =
        json!({"projectKind":"remote","projectId":"remote-project"});
    source["thread-project-membership-host-ids"]["remote-thread"] = json!("ssh:somewhere");
    source["app-server-project-id-by-legacy-project-id-by-host"]["ssh:somewhere"] =
        json!({"p":"remote-server"});
    source["app-server-project-migration-complete-by-host"] = json!({HOST:true});
    let original = document(source, vec![]);
    let plan = plan_share(&original, &owner('a'), &owner('b'), HOST, None).unwrap();
    let target = &plan.document[STATE]["inactive"][owner('b')]["globals"];
    assert_eq!(ids(target), BTreeSet::from(["p".into()]));
    assert_eq!(target["local-projects"]["p"], project("p", "Alpha"));
    assert_eq!(
        target["thread-project-assignments"]
            .as_object()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        target["app-server-project-id-by-legacy-project-id-by-host"],
        json!({HOST:{"p":"server-p"}})
    );
    assert!(target.get("trusted-root-permissions").is_none());
    assert!(target.get("migration-complete").is_none());
    assert!(target
        .get("app-server-project-migration-complete-by-host")
        .is_none());
}

#[test]
fn target_remote_host_links_are_never_overwritten_by_same_thread_id() {
    let mut target = globals("q", "Beta", "t2");
    target["thread-project-assignments"]["t1"] =
        json!({"projectKind":"remote","projectId":"remote-p"});
    target["thread-project-membership-host-ids"]["t1"] = json!("ssh:other");
    let original = document(globals("p", "Alpha", "t1"), vec![(owner('b'), target)]);
    assert!(plan_share(&original, &owner('a'), &owner('b'), HOST, None).is_err());
}

#[test]
fn rejects_corrupt_known_containers_versions_owners_references_and_pending_deletion() {
    let original = document(globals("p", "Alpha", "t1"), vec![]);
    for (key, bad) in [
        ("local-projects", json!([])),
        ("project-order", json!({})),
        ("thread-project-assignments", json!([])),
        ("project-appearances", json!([])),
        ("projectless-thread-ids", json!("t1")),
        ("sidebar-project-thread-orders", json!([])),
    ] {
        let mut doc = original.clone();
        doc[key] = bad;
        assert!(
            plan_share(&doc, &owner('a'), &owner('b'), HOST, None).is_err(),
            "{key}"
        );
    }
    let mut future = original.clone();
    future["electron-account-renderer-state-v2"] = json!({});
    assert!(plan_share(&future, &owner('a'), &owner('b'), HOST, None).is_err());
    assert!(plan_share(&original, "not-owner", &owner('b'), HOST, None).is_err());
    assert!(plan_share(&original, &owner('c'), &owner('b'), HOST, None).is_err());
    assert!(plan_share(&original, &owner('a'), &owner('b'), "ssh:elsewhere", None).is_err());
    let mut pending = original.clone();
    pending["app-server-pending-project-deletions-by-host"] = json!({HOST:{"p":{}}});
    assert!(plan_share(&pending, &owner('a'), &owner('b'), HOST, None).is_err());
}

#[test]
fn unresolved_unmanaged_assignments_stay_with_their_owner_across_roundtrip() {
    let mut source = globals("p", "Alpha", "t1");
    let mut target = globals("q", "Beta", "t2");
    let unresolved =
        json!({"projectKind":"local", "projectId":"missing-project", "unknownField":"keep"});
    source["thread-project-assignments"]["source-unresolved"] = unresolved.clone();
    target["thread-project-assignments"]["target-unresolved"] = unresolved.clone();
    let original = document(source, vec![(owner('b'), target)]);
    let before = original.clone();
    let plan = plan_share(&original, &owner('a'), &owner('b'), HOST, None).unwrap();
    assert_eq!(original, before);
    assert_eq!(plan.registry.canonical.projects.len(), 2);
    assert_eq!(plan.registry.canonical.thread_memberships.len(), 2);
    assert_eq!(
        plan.document["thread-project-assignments"]["source-unresolved"],
        unresolved
    );
    let selected = select_owner(&plan.document, &owner('b'));
    assert_eq!(
        selected["thread-project-assignments"]["target-unresolved"],
        unresolved
    );
    assert!(selected["thread-project-assignments"]
        .get("source-unresolved")
        .is_none());
    verify_restored(&selected, &owner('b'), HOST, &plan.registry.canonical).unwrap();
    let back = plan_share(
        &selected,
        &owner('b'),
        &owner('a'),
        HOST,
        Some(&plan.registry),
    )
    .unwrap();
    let restored = select_owner(&back.document, &owner('a'));
    assert_eq!(
        restored["thread-project-assignments"]["source-unresolved"],
        unresolved
    );
    assert!(restored["thread-project-assignments"]
        .get("target-unresolved")
        .is_none());
    assert_eq!(ids(&restored), BTreeSet::from(["p".into(), "q".into()]));
    verify_restored(&restored, &owner('a'), HOST, &back.registry.canonical).unwrap();
}

#[test]
fn unresolved_assignments_for_shared_threads_remain_conflicts() {
    let original = document(globals("p", "Alpha", "t1"), vec![]);
    let plan = plan_share(&original, &owner('a'), &owner('b'), HOST, None).unwrap();
    let mut selected = select_owner(&plan.document, &owner('b'));
    selected["thread-project-assignments"]["t1"]["projectId"] = json!("unknown-project");
    assert!(plan_share(
        &selected,
        &owner('b'),
        &owner('a'),
        HOST,
        Some(&plan.registry)
    )
    .is_err());
    let mut target = globals("q", "Beta", "t2");
    target["thread-project-assignments"]["t1"] =
        json!({"projectKind":"local","projectId":"unknown-project"});
    let collision = document(globals("p", "Alpha", "t1"), vec![(owner('b'), target)]);
    assert!(plan_share(&collision, &owner('a'), &owner('b'), HOST, None).is_err());
}

#[test]
fn unresolved_target_assignment_cannot_be_overwritten_by_imported_project() {
    let mut source = globals("p", "Alpha", "t1");
    source["local-projects"]["q"] = project("q", "Other source project");
    let target =
        json!({"thread-project-assignments":{"t1":{"projectKind":"local","projectId":"q"}}});
    let original = document(source, vec![(owner('b'), target)]);
    assert!(plan_share(&original, &owner('a'), &owner('b'), HOST, None).is_err());
}

#[test]
fn unresolved_target_assignment_cannot_be_revived_by_imported_project() {
    let target = json!({"thread-project-assignments":{"unresolved":{"projectKind":"local","projectId":"p"}}});
    let original = document(globals("p", "Alpha", "t1"), vec![(owner('b'), target)]);
    assert!(plan_share(&original, &owner('a'), &owner('b'), HOST, None).is_err());
}

#[test]
fn unresolved_source_assignment_cannot_collide_with_target_membership() {
    let mut source = globals("p", "Alpha", "t1");
    source["thread-project-assignments"]["t2"] =
        json!({"projectKind":"local","projectId":"unknown"});
    let original = document(source, vec![(owner('b'), globals("q", "Beta", "t2"))]);
    assert!(plan_share(&original, &owner('a'), &owner('b'), HOST, None).is_err());
}

#[test]
fn third_account_cannot_adopt_or_overwrite_another_owners_unresolved_assignment() {
    for collision in [false, true] {
        let mut source = globals("p", "Alpha", "t1");
        source["thread-project-assignments"]["unresolved"] =
            json!({"projectKind":"local","projectId":"q"});
        let mut third = globals("q", "Third", "t3");
        third["local-projects"]["r"] = project("r", "Third other");
        if collision {
            third["thread-project-assignments"]["unresolved"] =
                json!({"projectKind":"local","projectId":"r"});
        }
        let initial = document(source, vec![(owner('b'), json!({})), (owner('c'), third)]);
        let first = plan_share(&initial, &owner('a'), &owner('b'), HOST, None).unwrap();
        let selected_b = select_owner(&first.document, &owner('b'));
        let second = plan_share(
            &selected_b,
            &owner('b'),
            &owner('c'),
            HOST,
            Some(&first.registry),
        )
        .unwrap();
        let selected_c = select_owner(&second.document, &owner('c'));
        assert!(
            plan_share(
                &selected_c,
                &owner('c'),
                &owner('a'),
                HOST,
                Some(&second.registry)
            )
            .is_err(),
            "collision={collision}"
        );
    }
}

#[test]
fn registry_roundtrips_but_wrong_host_or_version_is_rejected() {
    let original = document(globals("p", "Alpha", "t1"), vec![]);
    let plan = plan_share(&original, &owner('a'), &owner('b'), HOST, None).unwrap();
    let serialized = serde_json::to_value(&plan.registry).unwrap();
    assert_eq!(serialized["schemaVersion"], json!(1));
    let registry: SharingRegistry = serde_json::from_value(serialized).unwrap();
    assert_eq!(registry, plan.registry);
    let mut bad = registry.clone();
    bad.schema_version = 2;
    assert!(plan_share(&original, &owner('a'), &owner('b'), HOST, Some(&bad)).is_err());
    assert!(plan_share(
        &original,
        &owner('a'),
        &owner('b'),
        "local:/synthetic/other-home",
        Some(&registry)
    )
    .is_err());
}

#[test]
fn same_owner_is_a_document_and_registry_noop() {
    let original = document(globals("p", "Alpha", "t1"), vec![]);
    let first = plan_share(&original, &owner('a'), &owner('b'), HOST, None).unwrap();
    let same = plan_share(
        &original,
        &owner('a'),
        &owner('a'),
        HOST,
        Some(&first.registry),
    )
    .unwrap();
    assert_eq!(same.document, original);
    assert_eq!(same.registry, first.registry);
    assert!(same.summary.no_op);
}

#[test]
fn absent_membership_host_is_preserved_and_cloud_id_collisions_fail() {
    let mut source = globals("p", "Alpha", "t1");
    source
        .as_object_mut()
        .unwrap()
        .remove("thread-project-membership-host-ids");
    let original = document(source, vec![]);
    let plan = plan_share(&original, &owner('a'), &owner('b'), HOST, None).unwrap();
    let target = &plan.document[STATE]["inactive"][owner('b')]["globals"];
    assert!(target["thread-project-membership-host-ids"]
        .get("t1")
        .is_none());
    let mut collision = original.clone();
    let mut cloud = globals("p", "Cloud", "ct");
    cloud["local-projects"]["p"]["chatGptBacking"] = json!({"id":"private"});
    collision[STATE]["inactive"][owner('b')] = json!({"atoms":{},"globals":cloud});
    assert!(plan_share(&collision, &owner('a'), &owner('b'), HOST, None).is_err());
}

#[test]
fn first_share_preserves_explicit_unassigned_threads_without_previous_project_links() {
    let mut source = globals("p", "Alpha", "t1");
    source["projectless-thread-ids"] = json!(["never-assigned"]);
    let mut target = globals("q", "Beta", "t2");
    target["projectless-thread-ids"] = json!(["target-unassigned"]);
    let original = document(source, vec![(owner('b'), target)]);
    let plan = plan_share(&original, &owner('a'), &owner('b'), HOST, None).unwrap();
    let selected = select_owner(&plan.document, &owner('b'));
    assert_eq!(
        selected["projectless-thread-ids"],
        json!(["never-assigned", "target-unassigned"])
    );
    assert_eq!(
        plan.registry.canonical.thread_memberships["never-assigned"].project_id,
        None
    );
}

#[test]
fn shared_project_thread_order_preserves_other_host_threads_in_relative_order() {
    let mut target = globals("p", "Alpha", "t1");
    for thread in ["remote-1", "remote-2"] {
        target["thread-project-assignments"][thread] =
            json!({"projectKind":"local","projectId":"p"});
        target["thread-project-membership-host-ids"][thread] = json!("ssh:other");
    }
    target["sidebar-project-thread-orders"]["p"] = json!({
        "threadIds":["remote-2","t1","remote-1"], "targetOnly":"keep"
    });
    let original = document(
        globals("p", "Alpha", "t1"),
        vec![(owner('b'), target.clone())],
    );
    let plan = plan_share(&original, &owner('a'), &owner('b'), HOST, None).unwrap();
    let selected = select_owner(&plan.document, &owner('b'));
    let order = selected["sidebar-project-thread-orders"]["p"]["threadIds"]
        .as_array()
        .unwrap();
    let remote: Vec<_> = order
        .iter()
        .filter(|value| value.as_str().unwrap().starts_with("remote-"))
        .cloned()
        .collect();
    assert_eq!(remote, vec![json!("remote-2"), json!("remote-1")]);
    assert_eq!(
        order.iter().filter(|value| value == &&json!("t1")).count(),
        1
    );
    assert_eq!(
        selected["sidebar-project-thread-orders"]["p"]["targetOnly"],
        json!("keep")
    );
    for thread in ["remote-1", "remote-2"] {
        assert_eq!(
            selected["thread-project-assignments"][thread],
            target["thread-project-assignments"][thread]
        );
        assert_eq!(
            selected["thread-project-membership-host-ids"][thread],
            json!("ssh:other")
        );
    }
}

#[test]
fn previously_shared_thread_moved_to_remote_host_conflicts_without_losing_order() {
    let original = document(globals("p", "Alpha", "t1"), vec![]);
    let first = plan_share(&original, &owner('a'), &owner('b'), HOST, None).unwrap();
    let mut changed = first.document.clone();
    let target = &mut changed[STATE]["inactive"][owner('b')]["globals"];
    target["thread-project-membership-host-ids"]["t1"] = json!("ssh:other");
    assert_eq!(
        target["sidebar-project-thread-orders"]["p"]["threadIds"],
        json!(["t1"])
    );
    let before = changed.clone();
    let baseline = first.registry.clone();
    let result = plan_share(
        &changed,
        &owner('a'),
        &owner('b'),
        HOST,
        Some(&first.registry),
    );
    assert!(
        result.is_err(),
        "a previously shared local thread cannot silently become remote"
    );
    assert_eq!(changed, before);
    assert_eq!(first.registry, baseline);
}

#[test]
fn previously_shared_source_thread_moved_to_remote_host_conflicts_without_propagating_deletion() {
    let original = document(globals("p", "Alpha", "t1"), vec![]);
    let first = plan_share(&original, &owner('a'), &owner('b'), HOST, None).unwrap();
    let mut changed = first.document.clone();
    changed["thread-project-membership-host-ids"]["t1"] = json!("ssh:other");
    let before = changed.clone();
    let baseline = first.registry.clone();
    assert!(
        plan_share(
            &changed,
            &owner('a'),
            &owner('b'),
            HOST,
            Some(&first.registry)
        )
        .is_err(),
        "source remote-host transition must not be mistaken for local relationship deletion"
    );
    assert_eq!(changed, before);
    assert_eq!(first.registry, baseline);
}

#[test]
fn deleted_relationship_can_be_reassigned_after_owner_baseline_observed_its_removal() {
    let original = document(globals("p", "Alpha", "t1"), vec![]);
    let first = plan_share(&original, &owner('a'), &owner('b'), HOST, None).unwrap();
    let mut removed = first.document.clone();
    removed["local-projects"]
        .as_object_mut()
        .unwrap()
        .remove("p");
    removed["thread-project-assignments"]
        .as_object_mut()
        .unwrap()
        .remove("t1");
    let deletion = plan_share(
        &removed,
        &owner('a'),
        &owner('b'),
        HOST,
        Some(&first.registry),
    )
    .unwrap();
    assert!(deletion.registry.deletions["threads"].contains("t1"));
    assert!(!deletion.registry.baselines[&owner('a')]
        .thread_memberships
        .contains_key("t1"));
    let mut reassigned = deletion.document.clone();
    reassigned["local-projects"]["q"] = project("q", "New project");
    reassigned["thread-project-assignments"]["t1"] =
        json!({"projectKind":"local", "projectId":"q"});
    reassigned["sidebar-project-thread-orders"] = json!({"q":{"threadIds":["t1"]}});
    reassigned["project-order"] = json!(["q"]);
    let next = plan_share(
        &reassigned,
        &owner('a'),
        &owner('b'),
        HOST,
        Some(&deletion.registry),
    )
    .unwrap();
    let target = &next.document[STATE]["inactive"][owner('b')]["globals"];
    assert_eq!(
        target["thread-project-assignments"]["t1"],
        json!({"projectKind":"local", "projectId":"q"})
    );
    assert_eq!(target["local-projects"]["q"], project("q", "New project"));
    assert!(!next.registry.deletions["threads"].contains("t1"));
    assert!(next.registry.deletions["projects"].contains("p"));
}

#[test]
fn deleted_relationship_is_not_resurrected_by_first_time_or_stale_baseline_accounts() {
    let original = document(globals("p", "Alpha", "t1"), vec![]);
    let ab = plan_share(&original, &owner('a'), &owner('b'), HOST, None).unwrap();
    let ac = plan_share(
        &ab.document,
        &owner('a'),
        &owner('c'),
        HOST,
        Some(&ab.registry),
    )
    .unwrap();
    let mut removed = ac.document.clone();
    removed["thread-project-assignments"]
        .as_object_mut()
        .unwrap()
        .remove("t1");
    removed["sidebar-project-thread-orders"]["p"]["threadIds"] = json!([]);
    let deletion =
        plan_share(&removed, &owner('a'), &owner('b'), HOST, Some(&ac.registry)).unwrap();
    assert!(deletion.registry.deletions["threads"].contains("t1"));

    let mut first_time_stale = deletion.document.clone();
    first_time_stale[STATE]["inactive"][owner('d')] =
        json!({"atoms":{},"globals":globals("p","Alpha","t1")});
    assert!(plan_share(
        &first_time_stale,
        &owner('a'),
        &owner('d'),
        HOST,
        Some(&deletion.registry)
    )
    .is_err());

    let stale_unchanged = plan_share(
        &deletion.document,
        &owner('a'),
        &owner('c'),
        HOST,
        Some(&deletion.registry),
    )
    .unwrap();
    assert!(
        stale_unchanged.document[STATE]["inactive"][owner('c')]["globals"]
            ["thread-project-assignments"]
            .get("t1")
            .is_none()
    );
    assert!(stale_unchanged.registry.deletions["threads"].contains("t1"));

    let mut stale_changed = deletion.document.clone();
    let target = &mut stale_changed[STATE]["inactive"][owner('c')]["globals"];
    target["local-projects"]["q"] = project("q", "New in stale account");
    target["thread-project-assignments"]["t1"]["projectId"] = json!("q");
    target["sidebar-project-thread-orders"] = json!({"q":{"threadIds":["t1"]}});
    assert!(plan_share(
        &stale_changed,
        &owner('a'),
        &owner('c'),
        HOST,
        Some(&deletion.registry)
    )
    .is_err());
}

#[test]
fn deleting_shared_project_with_unmanaged_remote_threads_in_target_conflicts() {
    let original = document(globals("p", "Alpha", "t1"), vec![]);
    let first = plan_share(&original, &owner('a'), &owner('b'), HOST, None).unwrap();
    let mut changed = first.document.clone();
    changed["local-projects"]
        .as_object_mut()
        .unwrap()
        .remove("p");
    changed["thread-project-assignments"]
        .as_object_mut()
        .unwrap()
        .remove("t1");
    changed["projectless-thread-ids"] = json!(["t1"]);
    let target = &mut changed[STATE]["inactive"][owner('b')]["globals"];
    target["thread-project-assignments"]["remote-thread"] =
        json!({"projectKind":"local", "projectId":"p"});
    target["thread-project-membership-host-ids"]["remote-thread"] = json!("ssh:other");
    target["sidebar-project-thread-orders"]["p"]["threadIds"] = json!(["t1", "remote-thread"]);
    let before = changed.clone();
    let baseline = first.registry.clone();
    assert!(
        plan_share(
            &changed,
            &owner('a'),
            &owner('b'),
            HOST,
            Some(&first.registry)
        )
        .is_err(),
        "deletion must not leave an unrelated remote membership referencing a missing project"
    );
    assert_eq!(changed, before);
    assert_eq!(first.registry, baseline);
}

fn three_projects() -> Value {
    let mut source = globals("p", "Alpha", "t1");
    source["local-projects"]["q"] = project("q", "Beta");
    source["local-projects"]["r"] = project("r", "Gamma");
    source["project-order"] = json!(["p", "q", "r"]);
    source
}

#[test]
fn target_only_project_reordering_is_preserved_against_unchanged_source_baseline() {
    let original = document(three_projects(), vec![]);
    let first = plan_share(&original, &owner('a'), &owner('b'), HOST, None).unwrap();
    let mut changed = first.document.clone();
    changed[STATE]["inactive"][owner('b')]["globals"]["project-order"] = json!(["q", "p", "r"]);
    let merged = plan_share(
        &changed,
        &owner('a'),
        &owner('b'),
        HOST,
        Some(&first.registry),
    )
    .unwrap();
    assert_eq!(
        merged.document[STATE]["inactive"][owner('b')]["globals"]["project-order"],
        json!(["q", "p", "r"])
    );
    assert_eq!(merged.registry.canonical.project_order, vec!["q", "p", "r"]);
}

#[test]
fn conflicting_source_and_target_project_reordering_is_rejected() {
    let original = document(three_projects(), vec![]);
    let first = plan_share(&original, &owner('a'), &owner('b'), HOST, None).unwrap();
    let mut changed = first.document.clone();
    changed["project-order"] = json!(["q", "p", "r"]);
    changed[STATE]["inactive"][owner('b')]["globals"]["project-order"] = json!(["p", "r", "q"]);
    assert!(plan_share(
        &changed,
        &owner('a'),
        &owner('b'),
        HOST,
        Some(&first.registry)
    )
    .is_err());
}

#[test]
fn initial_order_merge_keeps_source_order_then_independent_target_order() {
    let mut source = globals("p", "Alpha", "t1");
    source["local-projects"]["q"] = project("q", "Beta");
    source["project-order"] = json!(["q", "p"]);
    let mut target = globals("r", "Gamma", "t2");
    target["local-projects"]["s"] = project("s", "Delta");
    target["project-order"] = json!(["s", "r"]);
    let original = document(source, vec![(owner('b'), target)]);
    let merged = plan_share(&original, &owner('a'), &owner('b'), HOST, None).unwrap();
    assert_eq!(
        merged.registry.canonical.project_order,
        vec!["q", "p", "s", "r"]
    );
}

#[test]
fn historical_metadata_stays_with_its_owner_without_blocking_live_projects() {
    let original = document(globals("p", "Alpha", "t1"), vec![]);
    for (key, value) in [
        ("project-order", json!(["p", "never-known"])),
        ("pinned-project-ids", json!(["never-known"])),
        (
            "project-appearances",
            json!({"never-known":{"color":"red"}}),
        ),
        (
            "sidebar-project-thread-orders",
            json!({"never-known":{"threadIds":["t1"]}}),
        ),
        (
            "sidebar-project-thread-orders",
            json!({"p":{"threadIds":["unknown-thread"]}}),
        ),
        (
            "app-server-project-id-by-legacy-project-id-by-host",
            json!({HOST:{"never-known":"server-unknown"}}),
        ),
    ] {
        let mut source = original.clone();
        source[key] = value;
        let plan = plan_share(&source, &owner('a'), &owner('b'), HOST, None).unwrap();
        assert_eq!(plan.document[key], source[key], "{key}");
        assert_eq!(plan.registry.canonical.projects.len(), 1, "{key}");
        assert_eq!(plan.registry.canonical.thread_memberships.len(), 1, "{key}");
    }
}

#[test]
fn known_excluded_project_references_are_preserved_without_sharing() {
    let mut target = globals("q", "Beta", "t2");
    for id in ["cloud", "remote"] {
        target["local-projects"][id] = project(id, id);
        target["thread-project-assignments"][id] = json!({"projectKind":"local","projectId":id});
        target["project-appearances"][id] = json!({"color":"red"});
        target["sidebar-project-thread-orders"][id] = json!({"threadIds":[id]});
        target["app-server-project-id-by-legacy-project-id-by-host"][HOST][id] =
            json!(format!("server-{id}"));
    }
    target["local-projects"]["cloud"]["chatGptBacking"] = json!({"id":"private"});
    target["local-projects"]["remote"]["projectKind"] = json!("remote");
    target["thread-project-membership-host-ids"]["remote"] = json!("ssh:other");
    target["project-order"] = json!(["cloud", "q", "remote"]);
    target["pinned-project-ids"] = json!(["cloud", "remote", "q"]);
    let original = document(
        globals("p", "Alpha", "t1"),
        vec![(owner('b'), target.clone())],
    );
    let merged = plan_share(&original, &owner('a'), &owner('b'), HOST, None).unwrap();
    let selected = select_owner(&merged.document, &owner('b'));
    for id in ["cloud", "remote"] {
        assert!(!merged.registry.canonical.projects.contains_key(id));
        for key in [
            "local-projects",
            "thread-project-assignments",
            "project-appearances",
            "sidebar-project-thread-orders",
        ] {
            assert_eq!(selected[key][id], target[key][id], "{key}/{id}");
        }
        assert_eq!(
            selected["app-server-project-id-by-legacy-project-id-by-host"][HOST][id],
            target["app-server-project-id-by-legacy-project-id-by-host"][HOST][id]
        );
    }
}

#[test]
fn verify_restored_checks_core_records_and_ignores_ui_and_timestamp_changes() {
    let mut source = globals("p", "Alpha", "t1");
    source["thread-project-assignments"]["legacy-thread"] =
        json!({"projectKind":"local","projectId":"p"});
    source["projectless-thread-ids"] = json!(["loose-thread"]);
    let original = document(source, vec![]);
    let plan = plan_share(&original, &owner('a'), &owner('b'), HOST, None).unwrap();
    let mut selected = select_owner(&plan.document, &owner('b'));
    selected["local-projects"]["p"]["updatedAt"] = json!(9000);
    selected["project-order"] = json!([]);
    selected["project-appearances"]["p"] = json!({"color":"changed"});
    selected["global-unknown"] = json!({"new":"ui-value"});
    let expected = &plan.registry.baselines[&owner('b')];
    verify_restored(&selected, &owner('b'), HOST, expected).unwrap();
    for (pointer, value) in [
        (
            "/electron-account-renderer-state-v1/owner",
            json!(owner('a')),
        ),
        ("/local-projects/p/name", json!("Wrong name")),
        (
            "/local-projects/p/rootPaths",
            json!(["/synthetic/other-root"]),
        ),
        ("/thread-project-assignments/t1/projectId", json!("missing")),
        ("/thread-project-membership-host-ids/t1", json!("ssh:other")),
        ("/projectless-thread-ids", json!([])),
    ] {
        let mut changed = selected.clone();
        *changed.pointer_mut(pointer).unwrap() = value;
        assert!(
            verify_restored(&changed, &owner('b'), HOST, expected).is_err(),
            "{pointer}"
        );
    }
    let mut unexpected_host = selected.clone();
    unexpected_host["thread-project-membership-host-ids"]["legacy-thread"] = json!("local");
    assert!(verify_restored(&unexpected_host, &owner('b'), HOST, expected).is_err());
    let mut missing_host = selected.clone();
    missing_host["thread-project-membership-host-ids"]
        .as_object_mut()
        .unwrap()
        .remove("t1");
    assert!(verify_restored(&missing_host, &owner('b'), HOST, expected).is_err());
    let mut extra_project = selected.clone();
    extra_project["local-projects"]["extra"] = project("extra", "Unexpected");
    assert!(verify_restored(&extra_project, &owner('b'), HOST, expected).is_err());
    let mut extra_thread = selected.clone();
    extra_thread["thread-project-assignments"]["extra-thread"] =
        json!({"projectKind":"local","projectId":"p"});
    assert!(verify_restored(&extra_thread, &owner('b'), HOST, expected).is_err());
}
