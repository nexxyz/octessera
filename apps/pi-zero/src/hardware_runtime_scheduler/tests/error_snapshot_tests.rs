use super::*;

#[test]
fn runtime_error_and_dismissal_snapshots_stay_unpresented_until_captured() {
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
    assert!(scheduler.snapshot_unpresented(&playback));
    scheduler.record_native_scene_capture(now + SNAPSHOT_TICK, error_revision);
    assert!(!scheduler.snapshot_unpresented(&playback));

    playback.clear_error(playback_runtime::RuntimeOperation::StoreSaveDefault);
    let clear_revision = playback.last_snapshot_revision();
    assert!(clear_revision > error_revision);
    assert!(playback
        .last_snapshot()
        .and_then(|snapshot| snapshot.get("runtimeError"))
        .is_none_or(serde_json::Value::is_null));
    assert!(scheduler.snapshot_unpresented(&playback));
}
