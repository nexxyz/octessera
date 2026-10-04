use super::*;
use crate::hardware_runtime_scheduler::{HardwareRuntimeScheduler, PLAYBACK_TICK, SNAPSHOT_TICK};
use crate::host_adapter::PiHostAdapter;
use crate::render_loop::RenderWorker;
use crate::ui_profile::UiProfiler;
use playback_runtime::{
    DrumHit, HostAdapter, HostMessage, MusicalEvent, NativeRunner, NativeRunnerConfig,
    PlaybackRuntime, RunnerMessage, RuntimeAdapterError, RuntimeAudioCommand, RuntimeConfig,
    RuntimePlatformRequest,
};
use std::sync::Arc;
use std::time::{Duration, Instant};

fn worker() -> RenderWorker {
    let worker = RenderWorker::terminated_for_test();
    worker.allow_native_scenes_for_test();
    worker
}

#[test]
fn opted_in_profile_counts_successful_and_failed_scene_capture_calls() {
    let root = crate::test_temp_dir::unique_temp_path("octessera-orange-profile-capture");
    let mut adapter = adapter(&root);
    let (playback, mut runner, _) = playing_runner();
    let worker = worker();
    let mut profiler = UiProfiler::from_controls(Some("1"), false);
    let first_now = Instant::now();
    let mut successful = OrangeNativeScenePump::new(first_now - SNAPSHOT_TICK);
    successful.set_capture_profile_enabled(profiler.enabled());

    assert!(successful
        .submit(
            first_now,
            DisplaySnapshotDue::default(),
            &playback,
            &mut runner,
            &mut adapter,
            &worker,
        )
        .is_some());
    profiler.record_scene_capture(successful.take_capture_duration().unwrap());
    assert_eq!(profiler.scene_capture_count_for_test(), 1);

    runner.test_fail_next_snapshot();
    let failure_now = first_now + SNAPSHOT_TICK;
    let mut failed = OrangeNativeScenePump::new(failure_now - SNAPSHOT_TICK);
    failed.set_capture_profile_enabled(profiler.enabled());
    assert!(failed
        .submit(
            failure_now,
            DisplaySnapshotDue::default(),
            &playback,
            &mut runner,
            &mut adapter,
            &worker,
        )
        .is_none());
    profiler.record_scene_capture(failed.take_capture_duration().unwrap());
    assert_eq!(profiler.scene_capture_count_for_test(), 2);
    let _ = std::fs::remove_dir_all(root);
}

fn adapter(root: &std::path::Path) -> PiHostAdapter {
    let (audio, _, _, _) = crate::audio::test_service_with_recording_dir(root.join("recording"));
    PiHostAdapter::with_directories(
        audio,
        root.join("store"),
        root.join("samples"),
        Arc::new(|_| {}),
        false,
    )
    .unwrap()
}

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

fn playing_runner() -> (PlaybackRuntime, NativeRunner, TestHost) {
    let payload: serde_json::Value =
        serde_json::from_str(include_str!("../../../config/generated/pi/default.json")).unwrap();
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.apply_config_payload(payload).unwrap();
    runner.skip_startup_splash();
    let mut playback = PlaybackRuntime::new(RuntimeConfig::default());
    let mut host = TestHost;
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
                input: serde_json::json!({"type":"button_s","pressed":true}),
                request_snapshot: Some(true),
            },
            &mut runner,
            &mut host,
        )
        .unwrap();
    playback
        .advance_duration_music_first_with_output(
            Duration::from_millis(500),
            &mut runner,
            &mut host,
        )
        .unwrap();
    (playback, runner, host)
}

#[test]
fn queued_scene_does_not_recapture_generation_and_newer_expiry_stays_eligible() {
    let root = crate::test_temp_dir::unique_temp_path("octessera-orange-scene-pump");
    let mut adapter = adapter(&root);
    let (mut playback, mut runner, mut playback_host) = playing_runner();
    let worker = worker();
    let start = Instant::now();
    let mut scheduler = HardwareRuntimeScheduler::new(start, playback.last_snapshot_revision());
    let mut pump = OrangeNativeScenePump::new(start - SNAPSHOT_TICK);
    let initial_generation = runner.pending_display_scene_generation().unwrap();
    let first_capture = pump
        .submit(
            start,
            DisplaySnapshotDue::default(),
            &playback,
            &mut runner,
            &mut adapter,
            &worker,
        )
        .unwrap();
    scheduler.record_native_scene_capture(first_capture);
    assert_eq!(pump.capture_count, 1);

    let toast = playback
        .dispatch_host_message_music_first(
            HostMessage::RuntimeResult {
                result: playback_runtime::RuntimeStoreResult::RecordingStatus {
                    ok: true,
                    message: "a scrolling recording status toast needs typed refreshes".into(),
                    active: true,
                },
            },
            &mut runner,
            &mut playback_host,
        )
        .unwrap();
    assert!(toast
        .messages
        .iter()
        .any(|message| matches!(message, RunnerMessage::Snapshot { .. })));

    let refresh_due =
        scheduler.display_snapshot_due(first_capture + SNAPSHOT_TICK, &runner, &playback);
    assert!(refresh_due.continuous);
    assert!(pump
        .submit(
            first_capture + SNAPSHOT_TICK,
            refresh_due,
            &playback,
            &mut runner,
            &mut adapter,
            &worker,
        )
        .is_none());
    assert_eq!(pump.capture_count, 1);

    assert!(scheduler
        .next_runtime_advance(start, &playback, None)
        .is_none());
    let advance = scheduler
        .next_runtime_advance(start + PLAYBACK_TICK, &playback, None)
        .unwrap();
    assert!(!advance.request_snapshot);
    let output = playback
        .advance_duration_music_first_with_output(advance.elapsed, &mut runner, &mut playback_host)
        .unwrap();
    assert!(!output
        .messages
        .iter()
        .any(|message| matches!(message, RunnerMessage::Snapshot { .. })));
    assert!(
        scheduler
            .display_snapshot_due(start + SNAPSHOT_TICK, &runner, &playback)
            .continuous
    );
    assert!(!scheduler.snapshot_publication_due(start + SNAPSHOT_TICK, &playback));
    let scheduler_now = Instant::now();
    scheduler.record_runtime_advance_complete(scheduler_now, &playback, None);
    assert!(!scheduler
        .sleep_duration(scheduler_now, &playback, &runner)
        .is_zero());

    std::thread::sleep(Duration::from_millis(55));
    let newer_generation = runner.pending_display_scene_generation().unwrap();
    assert!(newer_generation > initial_generation);
    let expiry_due = scheduler.display_snapshot_due(Instant::now(), &runner, &playback);
    assert!(expiry_due.any());
    let newer_capture = pump
        .submit(
            Instant::now(),
            expiry_due,
            &playback,
            &mut runner,
            &mut adapter,
            &worker,
        )
        .unwrap();
    scheduler.record_native_scene_capture(newer_capture);
    assert_eq!(pump.capture_count, 2);
    pump.poll(&mut runner);
    assert_eq!(pump.generation, Some(newer_generation));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn legacy_runtime_error_due_stays_publishable_and_blocks_typed_overwrite() {
    let root = crate::test_temp_dir::unique_temp_path("octessera-orange-runtime-error-scene");
    let mut host = TestHost;
    let (mut playback, mut runner, _) = playing_runner();
    let now = Instant::now();
    let mut scheduler = HardwareRuntimeScheduler::new(now, playback.last_snapshot_revision());
    scheduler.observe_snapshot(now, &playback);
    playback
        .dispatch_host_message_music_first(
            HostMessage::RuntimeResult {
                result: playback_runtime::RuntimeStoreResult::RuntimeFailure {
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
    assert!(playback
        .last_snapshot()
        .and_then(|snapshot| snapshot.get("runtimeError"))
        .is_some_and(|error| !error.is_null()));
    assert!(scheduler.snapshot_publication_due(now + SNAPSHOT_TICK, &playback));

    let mut pump = OrangeNativeScenePump::new(now - SNAPSHOT_TICK);
    let mut adapter = adapter(&root);
    let worker = worker();
    assert!(pump
        .submit(
            now + SNAPSHOT_TICK,
            DisplaySnapshotDue {
                one_shot: false,
                continuous: true,
            },
            &playback,
            &mut runner,
            &mut adapter,
            &worker,
        )
        .is_none());
    assert_eq!(pump.capture_count, 0);

    playback
        .dispatch_host_message_music_first(
            HostMessage::DeviceInput {
                input: serde_json::json!({"type":"button_a","pressed":true}),
                request_snapshot: None,
            },
            &mut runner,
            &mut host,
        )
        .unwrap();
    assert!(playback
        .last_snapshot()
        .and_then(|snapshot| snapshot.get("runtimeError"))
        .is_none_or(serde_json::Value::is_null));
    assert!(scheduler.snapshot_publication_due(now + SNAPSHOT_TICK * 2, &playback));
    assert!(pump
        .submit(
            now + SNAPSHOT_TICK * 2,
            DisplaySnapshotDue {
                one_shot: false,
                continuous: true,
            },
            &playback,
            &mut runner,
            &mut adapter,
            &worker,
        )
        .is_some());
    assert_eq!(pump.capture_count, 1);
}
