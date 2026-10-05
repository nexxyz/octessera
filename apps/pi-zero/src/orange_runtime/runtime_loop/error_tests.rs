use super::*;
use crate::audio::test_service_with_recording_dir;
use crate::hardware_runtime_scheduler::SNAPSHOT_TICK;
use playback_runtime::{
    DrumHit, HostAdapter, HostMessage, MusicalEvent, NativeRunner, NativeRunnerConfig,
    RuntimeAdapterError, RuntimeAudioCommand, RuntimeConfig, RuntimeErrorCode, RuntimeErrorDomain,
    RuntimeErrorFacts, RuntimeOperation, RuntimePlatformRequest, RuntimeStoreResult,
};
use std::sync::Arc;

#[derive(Default)]
struct TestHost;

impl HostAdapter for TestHost {
    fn handle_musical_event(&mut self, _event: &MusicalEvent) -> Result<(), RuntimeAdapterError> {
        Ok(())
    }
    fn handle_drum_hit(&mut self, _hit: &DrumHit) -> Result<(), RuntimeAdapterError> {
        Ok(())
    }
    fn handle_platform_effect(
        &mut self,
        _request: &RuntimePlatformRequest,
    ) -> Result<Vec<HostMessage>, RuntimeAdapterError> {
        Ok(Vec::new())
    }
    fn handle_audio_command(
        &mut self,
        _command: &RuntimeAudioCommand,
    ) -> Result<(), RuntimeAdapterError> {
        Ok(())
    }
    fn handle_midi_message(&mut self, _bytes: &[u8]) -> Result<(), RuntimeAdapterError> {
        Ok(())
    }
    fn silence_internal_audio(&mut self) -> Result<(), RuntimeAdapterError> {
        Ok(())
    }
    fn panic_external_midi(&mut self) -> Result<(), RuntimeAdapterError> {
        Ok(())
    }
}

#[test]
fn orange_loop_publishes_changed_error_and_clear_snapshots_while_playing() {
    let root = crate::test_temp_dir::unique_temp_path("octessera-orange-runtime-error");
    let payload: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../config/generated/pi/default.json"
    ))
    .unwrap();
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.apply_config_payload(payload).unwrap();
    runner.skip_startup_splash();
    let mut playback = PlaybackRuntime::new(RuntimeConfig::default());
    let mut playback_host = TestHost;
    playback
        .dispatch_runner_messages(
            runner.messages_with_snapshot().unwrap(),
            &mut runner,
            &mut playback_host,
        )
        .unwrap();
    let initial_revision = playback.last_snapshot_revision();
    let mut scheduler = HardwareRuntimeScheduler::new(Instant::now(), initial_revision);
    for pressed in [true, false] {
        playback
            .dispatch_host_message_music_first(
                HostMessage::DeviceInput {
                    input: serde_json::json!({"type":"button_s","pressed":pressed}),
                    request_snapshot: None,
                },
                &mut runner,
                &mut playback_host,
            )
            .unwrap();
    }
    playback
        .advance_duration_music_first_with_output(
            std::time::Duration::from_millis(500),
            &mut runner,
            &mut playback_host,
        )
        .unwrap();
    assert!(playback.last_status().is_some_and(|status| {
        status.transport == playback_runtime::RuntimeTransportState::Playing
    }));
    let (audio, _, _, _) = test_service_with_recording_dir(root.join("recordings"));
    let mut host = crate::host_adapter::PiHostAdapter::with_directories(
        audio,
        root.join("store"),
        root.join("samples"),
        Arc::new(|_| {}),
        false,
    )
    .unwrap();
    if let Some(snapshot) = playback.last_snapshot() {
        let mut initial = snapshot.clone();
        initial["oledFrameRevision"] = serde_json::json!(1);
        host.core
            .ingest_oled_frame(&playback_runtime::RunnerMessage::OledFrame {
                revision: 1,
                width: 128,
                height: 128,
                format: "rgb565be".into(),
                pixels: vec![0; crate::render::OLED_FRAME_BYTES],
            });
        host.core.accept_oled_frame_reference(&initial);
    }
    crate::runtime_dispatch::dispatch(
        &mut playback,
        &mut runner,
        &mut host,
        HostMessage::RuntimeResult {
            result: RuntimeStoreResult::RuntimeFailure {
                error: RuntimeErrorFacts::new(
                    RuntimeErrorDomain::Storage,
                    RuntimeErrorCode::OperationFailed,
                    RuntimeOperation::StoreSaveDefault,
                    Some("save failed".into()),
                ),
            },
        },
    )
    .unwrap();
    let error_revision = playback.last_snapshot_revision();
    assert!(error_revision > initial_revision);
    assert!(scheduler.snapshot_publication_due(Instant::now() + SNAPSHOT_TICK, &playback));
    let worker = crate::render_loop::RenderWorker::terminated_for_test();
    assert!(publish_snapshot(
        &mut playback,
        &runner,
        &mut host,
        &worker,
        &mut scheduler,
        false,
    )
    .unwrap());
    assert_eq!(scheduler.published_snapshot_revision(), error_revision);

    crate::runtime_dispatch::dispatch(
        &mut playback,
        &mut runner,
        &mut host,
        HostMessage::DeviceInput {
            input: serde_json::json!({"type":"button_a","pressed":true}),
            request_snapshot: None,
        },
    )
    .unwrap();
    let cleared_revision = playback.last_snapshot_revision();
    assert!(cleared_revision > error_revision);
    assert!(scheduler.snapshot_publication_due(Instant::now() + SNAPSHOT_TICK, &playback));
    assert!(publish_snapshot(
        &mut playback,
        &runner,
        &mut host,
        &worker,
        &mut scheduler,
        false,
    )
    .unwrap());
    assert_eq!(scheduler.published_snapshot_revision(), cleared_revision);
    let _ = std::fs::remove_dir_all(root);
}
