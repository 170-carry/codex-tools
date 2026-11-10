use super::*;

#[test]
fn historical_project_mappings_do_not_block_live_projects() {
    let mut source = globals("p", "Live", "t1");
    source["app-server-project-id-by-legacy-project-id-by-host"][HOST]["old"] = json!("server-old");
    source["sidebar-project-thread-orders"]["old"] = json!({"threadIds":["old-thread"]});
    source["sidebar-project-thread-orders"]["p"]["threadIds"] = json!(["t1", "unindexed-thread"]);
    let original = document(source, vec![]);
    let plan = plan_share(&original, &owner('a'), &owner('b'), HOST, None).unwrap();
    assert_eq!(
        plan.document["app-server-project-id-by-legacy-project-id-by-host"],
        original["app-server-project-id-by-legacy-project-id-by-host"]
    );
    let selected = select_owner(&plan.document, &owner('b'));
    assert_eq!(ids(&selected), BTreeSet::from(["p".into()]));
    assert_eq!(
        selected["sidebar-project-thread-orders"]["p"]["threadIds"],
        json!(["t1"])
    );
    assert!(
        selected["app-server-project-id-by-legacy-project-id-by-host"][HOST]
            .get("old")
            .is_none()
    );
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
        restored["app-server-project-id-by-legacy-project-id-by-host"][HOST]["old"],
        json!("server-old")
    );
    assert_eq!(
        restored["sidebar-project-thread-orders"]["old"],
        original["sidebar-project-thread-orders"]["old"]
    );
}

#[test]
fn importing_a_project_cannot_overwrite_unresolved_target_metadata() {
    for key in [
        "project-appearances",
        "sidebar-project-thread-orders",
        "app-server-project-id-by-legacy-project-id-by-host",
    ] {
        let mut target = globals("q", "Target", "t2");
        target[key] = match key {
            "project-appearances" => json!({"p":{"color":"red"}}),
            "sidebar-project-thread-orders" => json!({"p":{"threadIds":["old"]}}),
            _ => json!({HOST:{"p":"different-server"}}),
        };
        let original = document(globals("p", "Source", "t1"), vec![(owner('b'), target)]);
        assert!(
            plan_share(&original, &owner('a'), &owner('b'), HOST, None).is_err(),
            "{key}"
        );
    }
}
