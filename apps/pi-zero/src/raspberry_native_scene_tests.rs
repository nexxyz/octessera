use super::*;
use crate::hardware_runtime_scheduler::{HardwareRuntimeScheduler, PLAYBACK_TICK, SNAPSHOT_TICK};
use crate::host_adapter::PiPlaybackHostAdapter;
use crate::render::{HardwareRenderTargets, OLED_FRAME_BYTES};
use crate::render_loop::RenderWorker;
use playback_runtime::{
    HostMessage, NativeRunner, NativeRunnerConfig, PlaybackRuntime, RunnerMessage, RuntimeConfig,
    UsbDataRole,
};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::{Duration, Instant};

fn worker() -> RenderWorker {
    worker_with_recording(None)
}

pub(super) fn worker_with_recording(audio: Option<crate::audio::AudioService>) -> RenderWorker {
    let (seesaw_tx, _seesaw_rx) = mpsc::channel();
    let worker = RenderWorker::spawn(HardwareRenderTargets {
        oled: crate::render::test_oled_output::fake_oled_output(),
        seesaw_tx,
        oled_handoff: None,
        hdmi: crate::render::hdmi::HdmiFramebuffer::new(),
    });
    let initial_oled =
        crate::oled_frame_cache::OledFramePublication::test_native(1, vec![0; OLED_FRAME_BYTES]);
    worker
        .publish_acknowledged_snapshot(startup_snapshot(), initial_oled)
        .unwrap();
    if let Some(audio) = audio {
        let (revision, pixels) = worker.take_acknowledged_startup_oled_frame().unwrap();
        audio
            .submit_accepted_oled_frame_shared(revision, pixels)
            .unwrap();
        worker.set_recording_audio(audio);
    }
    worker
}

#[test]
fn failed_initial_oled_write_leaves_no_startup_recording_seed() {
    let root = crate::test_temp_dir::unique_temp_path("octessera-failed-startup-oled");
    let (audio, _, _, _) = crate::audio::test_service_with_recording_dir(root.clone());
    let (seesaw_tx, _seesaw_rx) = mpsc::channel();
    let worker = RenderWorker::spawn(HardwareRenderTargets {
        oled: crate::render::test_oled_output::fake_oled_output(),
        seesaw_tx,
        oled_handoff: None,
        hdmi: crate::render::hdmi::HdmiFramebuffer::new(),
    });
    worker.fail_next_startup_oled_write_for_test();
    assert!(worker.take_acknowledged_startup_oled_frame().is_err());
    assert!(worker
        .publish_acknowledged_snapshot(
            startup_snapshot(),
            crate::oled_frame_cache::OledFramePublication::test_native(
                1,
                vec![0; OLED_FRAME_BYTES],
            ),
        )
        .is_err());
    assert!(worker.take_acknowledged_startup_oled_frame().is_err());
    assert!(audio.latest_physical_oled_frame().is_none());
    worker.publish_shutdown().unwrap();
    let _ = std::fs::remove_dir_all(root);
}

fn startup_snapshot() -> serde_json::Value {
    serde_json::json!({
        "display": {"off": false},
        "settings": {"buttonBrightness": 100, "displayBrightness": 100},
        "leds": {"rgb": vec![0; 64 * 3]},
        "transport": {"playing": false},
        "transportIcon": "stop",
        "transportFlash": "none",
        "eventDotOn": false,
        "oledFrameRevision": 1,
        "neoKeyLeds": {
            "back": [0, 0, 0],
            "space": [0, 0, 0],
            "shift": [0, 0, 0],
            "fn": [0, 0, 0]
        }
    })
}

fn adapter(
    root: &std::path::Path,
    audio: Option<crate::audio::AudioService>,
) -> PiPlaybackHostAdapter {
    PiPlaybackHostAdapter::new_with_data_role(
        audio,
        root.join("store"),
        root.join("samples"),
        Arc::new(|_| {}),
        false,
        playback_runtime::AudioOutputSet::jack(),
        UsbDataRole::Gadget,
    )
}

pub(super) fn playing_runner(
    adapter: &mut PiPlaybackHostAdapter,
) -> (PlaybackRuntime, NativeRunner) {
    let payload: serde_json::Value =
        serde_json::from_str(include_str!("../../../config/generated/pi/default.json")).unwrap();
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.apply_config_payload(payload).unwrap();
    runner.skip_startup_splash();
    runner.test_set_display_time(Instant::now());
    let mut playback = PlaybackRuntime::new(RuntimeConfig::default());
    playback
        .dispatch_runner_messages(
            runner.messages_with_snapshot().unwrap(),
            &mut runner,
            adapter,
        )
        .unwrap();
    playback
        .dispatch_host_message_music_first(
            HostMessage::DeviceInput {
                input: serde_json::json!({"type":"button_s","pressed":true}),
                request_snapshot: Some(true),
            },
            &mut runner,
            adapter,
        )
        .unwrap();
    playback
        .advance_duration_music_first_with_output(Duration::from_millis(500), &mut runner, adapter)
        .unwrap();
    (playback, runner)
}

#[test]
fn held_worker_does_not_recapture_in_flight_generation_and_submits_new_expiry() {
    let root = crate::test_temp_dir::unique_temp_path("octessera-scene-pump");
    let mut adapter = adapter(&root, None);
    let (mut playback, mut runner) = playing_runner(&mut adapter);
    let worker = worker();
    let (entered, release) = worker.block_next_native_scene_for_test();
    let start = Instant::now();
    let mut scheduler = HardwareRuntimeScheduler::new(start, playback.last_snapshot_revision());
    let mut pump = NativeScenePump::new(start - SNAPSHOT_TICK);
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
    entered.recv_timeout(Duration::from_secs(1)).unwrap();
    assert_eq!(pump.capture_count, 1);
    assert_eq!(
        runner.pending_display_scene_generation(),
        Some(initial_generation)
    );

    let _toast_output = playback
        .dispatch_host_message_music_first(
            HostMessage::RuntimeResult {
                result: playback_runtime::RuntimeStoreResult::RecordingStatus {
                    ok: true,
                    message: "a scrolling recording status toast used to refresh from JSON".into(),
                    active: true,
                },
            },
            &mut runner,
            &mut adapter,
        )
        .unwrap();

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
        .advance_duration_music_first_with_output(advance.elapsed, &mut runner, &mut adapter)
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
    runner.test_advance_display_time(Duration::from_millis(100));
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
    release.send(()).unwrap();

    let deadline = Instant::now() + Duration::from_secs(2);
    while runner.display_scene_pending() && Instant::now() < deadline {
        pump.poll(&mut runner);
        std::thread::sleep(Duration::from_millis(1));
    }
    pump.poll(&mut runner);
    assert!(!runner.display_scene_pending());

    let periodic_at = newer_capture + SNAPSHOT_TICK;
    std::thread::sleep(periodic_at.saturating_duration_since(Instant::now()));
    let refresh_due = scheduler.display_snapshot_due(Instant::now(), &runner, &playback);
    assert!(refresh_due.continuous);
    let refresh_capture = pump
        .submit(
            Instant::now(),
            refresh_due,
            &playback,
            &mut runner,
            &mut adapter,
            &worker,
        )
        .unwrap();
    scheduler.record_native_scene_capture(refresh_capture);
    assert_eq!(pump.capture_count, 3);
    let refresh_deadline = Instant::now() + Duration::from_secs(2);
    while pump.generation.is_some() && Instant::now() < refresh_deadline {
        pump.poll(&mut runner);
        std::thread::sleep(Duration::from_millis(1));
    }
    pump.poll(&mut runner);
    assert!(pump.generation.is_none());
    worker.publish_shutdown().unwrap();
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn rejected_queue_submission_keeps_the_bounded_capture_retry() {
    let root = crate::test_temp_dir::unique_temp_path("octessera-scene-pump-retry");
    let mut adapter = adapter(&root, None);
    let (_playback, mut runner) = playing_runner(&mut adapter);
    let (seesaw_tx, _seesaw_rx) = mpsc::channel();
    let worker = RenderWorker::spawn(HardwareRenderTargets {
        oled: crate::render::test_oled_output::fake_oled_output(),
        seesaw_tx,
        oled_handoff: None,
        hdmi: crate::render::hdmi::HdmiFramebuffer::new(),
    });
    let start = Instant::now();
    let mut pump = NativeScenePump::new(start - SNAPSHOT_TICK);
    assert!(pump
        .submit(
            start,
            DisplaySnapshotDue::default(),
            &_playback,
            &mut runner,
            &mut adapter,
            &worker,
        )
        .is_some());
    assert!(pump.generation.is_none());
    assert_eq!(pump.capture_count, 1);
    assert!(pump
        .submit(
            start + SNAPSHOT_TICK - Duration::from_nanos(1),
            DisplaySnapshotDue::default(),
            &_playback,
            &mut runner,
            &mut adapter,
            &worker,
        )
        .is_none());
    assert_eq!(pump.capture_count, 1);
    let retry_at = pump.last_submission + SNAPSHOT_TICK;
    assert!(pump
        .submit(
            retry_at,
            DisplaySnapshotDue::default(),
            &_playback,
            &mut runner,
            &mut adapter,
            &worker,
        )
        .is_some());
    assert_eq!(pump.capture_count, 2);
    worker.abort().unwrap();
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(test)]
#[path = "raspberry_native_recording_tests.rs"]
mod recording_tests;
