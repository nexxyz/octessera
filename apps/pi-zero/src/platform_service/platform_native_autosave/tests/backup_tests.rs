use super::*;

#[test]
fn backup_only_remains_due_while_default_write_is_pending_without_acknowledging_it() {
    let (service, root) = service_and_root("backup-only");
    let mut playback = PlaybackRuntime::new(playback_runtime::RuntimeConfig::default());
    let mut runner = runner_with_aux_mapping(true, true);
    let mut pending = PendingPiPersistence::default();
    aux_turn(&mut runner, -1);
    std::thread::sleep(Duration::from_millis(160));
    let eligible_at = Instant::now();
    let revision = runner.capture_config_snapshot().revision();
    assert!(runner.register_native_default_write("pending-default", revision, true));
    let intent = runner.persistence_intent_at(eligible_at).unwrap();
    assert!(!intent.default_eligible());
    assert!(intent.backup_eligible());
    let due = eligible_at + Duration::from_secs(2);
    apply_autosave_at(
        &mut pending,
        &service,
        &mut playback,
        &mut runner,
        eligible_at,
    );
    assert!(apply_autosave_at(&mut pending, &service, &mut playback, &mut runner, due).is_empty());
    assert!(service.native_default_write().is_none());
    let results = collect_results(&service, &mut runner, 1);
    assert!(matches!(
        results[0].operation(),
        RuntimeOperation::StoreSaveBackup
    ));
    assert!(!root.join("store/default.patch.json").exists());
    let backup_dir = root.join("store/backups");
    assert_eq!(std::fs::read_dir(backup_dir).unwrap().count(), 1);
    assert!(runner.persistence_intent_at(due).is_none());
    assert!(!runner.register_native_default_write("second-default", revision, true));
    assert!(runner
        .persistence_intent_at(due + Duration::from_secs(300))
        .unwrap()
        .backup_eligible());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn continuous_edits_save_the_default_at_each_first_deadline_and_the_backup_when_due() {
    let (service, root) = service_and_root("continuous-backup");
    let mut playback = PlaybackRuntime::new(playback_runtime::RuntimeConfig::default());
    let mut runner = runner_with_aux_mapping(true, true);
    let mut pending = PendingPiPersistence::default();
    aux_turn(&mut runner, -1);
    std::thread::sleep(Duration::from_millis(160));
    let mut now = Instant::now();
    runner.mark_native_backup_issued_at(now - Duration::from_millis(151));
    let initial_intent = runner.persistence_intent_at(now).unwrap();
    assert!(initial_intent.default_eligible());
    assert!(!initial_intent.backup_eligible());
    apply_autosave_at(&mut pending, &service, &mut playback, &mut runner, now);

    let mut default_saves = 0;
    let mut backup_saved = false;
    for turn in 0..1_501 {
        aux_turn(&mut runner, if turn % 2 == 0 { -1 } else { 1 });
        now += Duration::from_millis(200);
        let intent = runner.persistence_intent_at(now).unwrap();
        assert!(intent.default_eligible());
        let backup_due = intent.backup_eligible();
        assert!(
            apply_autosave_at(&mut pending, &service, &mut playback, &mut runner, now).is_empty()
        );
        let expected =
            usize::from(service.native_default_write().is_some()) + usize::from(backup_due);
        if expected == 0 {
            continue;
        }
        for result in collect_results(&service, &mut runner, expected) {
            match result.operation() {
                RuntimeOperation::StoreSaveDefault => default_saves += 1,
                RuntimeOperation::StoreSaveBackup => backup_saved = true,
                operation => panic!("unexpected {operation:?}"),
            }
        }
        if backup_saved {
            assert!(turn >= 1_490);
            break;
        }
        assert_eq!(default_saves, turn / 11 + 1, "turn {turn}");
    }
    assert!(backup_saved);
    assert!(default_saves > 100);
    assert!(root.join("store/default.patch.json").exists());
    assert_eq!(
        std::fs::read_dir(root.join("store/backups"))
            .unwrap()
            .count(),
        1
    );
    let _ = std::fs::remove_dir_all(root);
}
