use super::*;

#[test]
fn changed_legacy_error_and_dismissal_snapshots_publish_once_while_playing() {
    let now = Instant::now();
    let (mut playback, mut runner, mut host) = playing_playback();
    let baseline_revision = playback.last_snapshot_revision();
    let mut scheduler = HardwareRuntimeScheduler::new(now, baseline_revision);
    playback
        .dispatch_host_message_music_first(
            HostMessage::RuntimeResult {
                result: RuntimeStoreResult::RuntimeFailure {
                    error: playback_runtime::RuntimeErrorFacts::new(
                        playback_runtime::RuntimeErrorDomain::Storage,
                        playback_runtime::RuntimeErrorCode::OperationFailed,
                        playback_runtime::RuntimeOperation::StoreSaveDefault,
                        Some("save failed".into()),
                    ),
                },
            },
            &mut runner,
            &mut host,
        )
        .unwrap();
    let error_revision = playback.last_snapshot_revision();
    assert!(error_revision > baseline_revision);
    assert!(playback
        .last_snapshot()
        .and_then(|snapshot| snapshot.get("runtimeError"))
        .is_some_and(|error| !error.is_null()));
    assert!(scheduler.snapshot_publication_due(now + SNAPSHOT_TICK, &playback));
    scheduler.record_snapshot_publication_attempt(now + SNAPSHOT_TICK);
    scheduler.record_snapshot_publication_accepted(error_revision);

    playback.clear_error(playback_runtime::RuntimeOperation::StoreSaveDefault);
    let clear_revision = playback.last_snapshot_revision();
    assert!(clear_revision > error_revision);
    assert!(playback
        .last_snapshot()
        .and_then(|snapshot| snapshot.get("runtimeError"))
        .is_none_or(serde_json::Value::is_null));
    assert!(scheduler.snapshot_publication_due(now + SNAPSHOT_TICK * 2, &playback));
}
