use super::*;
use std::cell::Cell;

fn switched_account(f: &Fixture) -> crate::models::SwitchAccountResult {
    crate::noop_switch_account_result(&strict_store(&f.paths.store_path).unwrap().accounts[1])
}

#[tokio::test]
async fn broken_session_files_continue_to_account_switch_without_rewriting_them() {
    for body in [b"{\n broken".as_slice(), b"null", b"\xef\xbb\xbf{}"] {
        let f = fixture(1, 1, 1);
        fs::write(f.paths.global(), body).unwrap();
        let original = tracked_tree(f.paths.codex_home.parent().unwrap());
        assert_eq!(
            preview(&f.paths, "b").unwrap_err(),
            diagnostics::SYNC_FAILED
        );
        let attempt = execute(
            &f.paths,
            "b",
            SwitchOptions::default(),
            &mut FakeLifecycle::default(),
        )
        .map(|_| switched_account(&f));
        let calls = Cell::new(0);
        let result = best_effort::finish_switch(attempt, || async {
            calls.set(calls.get() + 1);
            let _lock = legacy_switch_lock(&f.paths)?;
            assert_eq!(original, tracked_tree(f.paths.codex_home.parent().unwrap()));
            Ok(switched_account(&f))
        })
        .await
        .unwrap();
        assert_eq!(calls.get(), 1);
        assert_eq!(result.account_id, "synthetic-account-b");
        assert_eq!(result.local_project_preservation.unwrap().status, "skipped");
        assert_eq!(fs::read(f.paths.global()).unwrap(), body);
    }
}

#[tokio::test]
async fn unreadable_session_checkpoint_does_not_block_account_switch() {
    let f = fixture(1, 1, 1);
    let checkpoint = persistence::target(&f.paths, "profile/b/desktop-state.json").unwrap();
    fs::create_dir_all(&checkpoint).unwrap();
    let original = fs::read(f.paths.global()).unwrap();
    let attempt = execute(
        &f.paths,
        "b",
        SwitchOptions::default(),
        &mut FakeLifecycle::default(),
    )
    .map(|_| switched_account(&f));
    let result = best_effort::finish_switch(attempt, || async { Ok(switched_account(&f)) })
        .await
        .unwrap();
    assert_eq!(result.local_project_preservation.unwrap().status, "skipped");
    assert_eq!(fs::read(f.paths.global()).unwrap(), original);
    assert!(checkpoint.is_dir());
}

#[tokio::test]
async fn session_write_failure_rolls_back_before_continuing_account_switch() {
    let f = fixture(1, 1, 1);
    let original = tracked_tree(f.paths.codex_home.parent().unwrap());
    let changes = vec![
        persistence::change(&f.paths, "auth", serde_json::to_vec(&auth("b")).unwrap()).unwrap(),
        persistence::change(&f.paths, "global", b"synthetic replacement".to_vec()).unwrap(),
    ];
    let attempt = persistence::apply_with_writers(
        &f.paths,
        changes,
        &mut FakeLifecycle::default(),
        false,
        store_lock::atomic_write,
        |path, bytes| {
            if path == f.paths.global() {
                Err("PROJECT_SHARING_IO".into())
            } else {
                store_lock::atomic_write(path, bytes)
            }
        },
    )
    .map(|_| switched_account(&f));
    let result = best_effort::finish_switch(attempt, || async {
        assert_eq!(original, tracked_tree(f.paths.codex_home.parent().unwrap()));
        let _lock = legacy_switch_lock(&f.paths)?;
        Ok(switched_account(&f))
    })
    .await
    .unwrap();
    assert_eq!(result.local_project_preservation.unwrap().status, "skipped");
    assert!(!f.paths.journal().exists());
}

#[tokio::test]
async fn invalid_credentials_and_pending_recovery_still_stop_account_switch() {
    for pending in [false, true] {
        let f = fixture(1, 1, 1);
        if pending {
            fs::create_dir_all(f.paths.journal()).unwrap();
        } else {
            fs::write(
                f.paths.codex_home.join("auth.json"),
                b"{ broken credentials",
            )
            .unwrap();
        }
        let original = tracked_tree(f.paths.codex_home.parent().unwrap());
        let attempt = execute(
            &f.paths,
            "b",
            SwitchOptions::default(),
            &mut FakeLifecycle::default(),
        )
        .map(|_| switched_account(&f));
        let error = best_effort::finish_switch(attempt, || async { panic!("must not switch") })
            .await
            .unwrap_err();
        assert_eq!(
            error,
            if pending {
                "PROJECT_SHARING_RECOVERY_REQUIRED"
            } else {
                "PROJECT_SHARING_INVALID_STATE"
            }
        );
        assert_eq!(original, tracked_tree(f.paths.codex_home.parent().unwrap()));
    }
}

#[tokio::test]
async fn successful_sync_does_not_run_a_second_account_switch() {
    let f = fixture(1, 1, 1);
    let mut result = switched_account(&f);
    result.local_project_preservation = Some(
        execute(
            &f.paths,
            "b",
            SwitchOptions::default(),
            &mut FakeLifecycle::default(),
        )
        .unwrap()
        .result,
    );
    let result =
        best_effort::finish_switch(Ok(result), || async { panic!("must not switch twice") })
            .await
            .unwrap();
    assert_eq!(
        result.local_project_preservation.unwrap().status,
        "prepared"
    );
}

#[tokio::test]
async fn changed_session_skips_copy_but_changed_credentials_still_stop_the_switch() {
    for key in ["global", "auth"] {
        let f = fixture(1, 1, 1);
        let change = persistence::change(&f.paths, key, b"synthetic replacement".to_vec()).unwrap();
        let path = persistence::target(&f.paths, key).unwrap();
        fs::write(&path, b"synthetic external change").unwrap();
        let attempt = persistence::apply(&f.paths, vec![change], &mut FakeLifecycle::default())
            .map(|_| switched_account(&f));
        let called = Cell::new(false);
        let result = best_effort::finish_switch(attempt, || async {
            called.set(true);
            Ok(switched_account(&f))
        })
        .await;
        assert_eq!(called.get(), key == "global");
        if key == "global" {
            assert_eq!(
                result.unwrap().local_project_preservation.unwrap().status,
                "skipped"
            );
        } else {
            assert_eq!(result.unwrap_err(), "PROJECT_SHARING_EXTERNAL_CHANGE");
        }
        assert_eq!(fs::read(path).unwrap(), b"synthetic external change");
        assert!(!f.paths.journal().exists());
    }
}

#[tokio::test]
async fn ordinary_switch_failure_is_not_reported_as_a_successful_skipped_sync() {
    let error = best_effort::finish_switch(Err(diagnostics::SYNC_FAILED.into()), || async {
        Err("synthetic credential failure".into())
    })
    .await
    .unwrap_err();
    assert_eq!(error, "synthetic credential failure");
}
