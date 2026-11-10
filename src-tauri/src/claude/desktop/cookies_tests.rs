use super::*;

#[test]
fn replaces_only_claude_auth_cookies_and_preserves_binary_values() {
    let root = std::env::temp_dir().join(format!("claude-cookies-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("Cookies");
    let db = Connection::open(&path).unwrap();
    // Tiny, isolated fixture: three rows. Schema creation and indexed mutations
    // never touch the real desktop database.
    db.execute_batch("CREATE TABLE cookies(host_key TEXT, name TEXT, value TEXT, encrypted_value BLOB); CREATE UNIQUE INDEX cookies_unique_index ON cookies(host_key,name);").unwrap();
    db.execute(
        "INSERT INTO cookies VALUES (?1,?2,?3,?4)",
        (".claude.ai", "sessionKey", "", vec![0u8, 255, 1, 2]),
    )
    .unwrap();
    db.execute(
        "INSERT INTO cookies VALUES (?1,?2,?3,?4)",
        (".claude.ai", "theme", "dark", Vec::<u8>::new()),
    )
    .unwrap();
    db.execute(
        "INSERT INTO cookies VALUES (?1,?2,?3,?4)",
        ("example.org", "sessionKey", "other-site", Vec::<u8>::new()),
    )
    .unwrap();
    let snapshot = read(&path).unwrap();
    assert!(has_session(&snapshot));
    assert_eq!(snapshot.rows.len(), 1);
    db.execute(
        "UPDATE cookies SET encrypted_value=?1 WHERE host_key=?2 AND name=?3",
        (vec![8u8], ".claude.ai", "sessionKey"),
    )
    .unwrap();
    restore(&path, &snapshot).unwrap();
    let binary: Vec<u8> = db
        .query_row(
            "SELECT encrypted_value FROM cookies WHERE host_key=?1 AND name=?2",
            (".claude.ai", "sessionKey"),
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(binary, vec![0, 255, 1, 2]);
    let theme: String = db
        .query_row(
            "SELECT value FROM cookies WHERE host_key=?1 AND name=?2",
            (".claude.ai", "theme"),
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(theme, "dark");
    let other: String = db
        .query_row(
            "SELECT value FROM cookies WHERE host_key=?1 AND name=?2",
            ("example.org", "sessionKey"),
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(other, "other-site");
    drop(db);
    std::fs::remove_dir_all(root).unwrap();
}
