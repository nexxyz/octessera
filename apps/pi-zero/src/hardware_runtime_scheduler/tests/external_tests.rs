use super::*;

#[test]
fn external_midi_playing_keeps_typed_display_deadline_without_clock_pulses() {
    let now = Instant::now();
    let mut playback = PlaybackRuntime::new(playback_runtime::RuntimeConfig {
        sync_source: SyncSource::External,
        ..playback_runtime::RuntimeConfig::default()
    });
    let mut runner = NativeRunner::new(playback_runtime::NativeRunnerConfig::default()).unwrap();
    runner
        .apply_config_payload(
            serde_json::from_str(include_str!(
                "../../../../../config/generated/pi/default.json"
            ))
            .unwrap(),
        )
        .unwrap();
    runner.skip_startup_splash();
    let mut host = TestHost::default();
    playback
        .dispatch_runner_messages(
            runner.messages_with_snapshot().unwrap(),
            &mut runner,
            &mut host,
        )
        .unwrap();
    playback
        .dispatch_host_message_music_first(
            HostMessage::DeviceInput {
                input: json!({"type":"button_s","pressed":true}),
                request_snapshot: Some(false),
            },
            &mut runner,
            &mut host,
        )
        .unwrap();
    assert!(playback
        .last_status()
        .is_some_and(|status| { status.transport == RuntimeTransportState::Playing }));
    playback
        .dispatch_host_message_music_first(
            HostMessage::RuntimeResult {
                result: RuntimeStoreResult::RecordingStatus {
                    ok: true,
                    message: "a scrolling external-sync display row".into(),
                    active: true,
                },
            },
            &mut runner,
            &mut host,
        )
        .unwrap();
    let scheduler = HardwareRuntimeScheduler::new(now, playback.last_snapshot_revision());

    assert!(
        scheduler
            .display_snapshot_due(now + SNAPSHOT_TICK, &runner, &playback)
            .continuous
    );
}
