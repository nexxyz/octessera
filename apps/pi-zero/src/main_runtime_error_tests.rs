use super::*;
use crate::hardware_runtime_scheduler::{
    DisplaySnapshotDue, HardwareRuntimeScheduler, SNAPSHOT_TICK,
};
use crate::host_adapter::PiHostAdapter;
use crate::raspberry_native_scene::NativeScenePump;
use crate::render::HardwareRenderTargets;
use crate::render_loop::RenderWorker;
use playback_runtime::{
    HostMessage, NativeRunner, NativeRunnerConfig, RuntimeConfig, RuntimeErrorCode,
    RuntimeErrorDomain, RuntimeErrorFacts, RuntimeOperation, RuntimeStoreResult, UsbDataRole,
};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[test]
fn playing_save_error_and_dismissal_snapshots_reach_the_physical_worker() {
    let root = crate::test_temp_dir::unique_temp_path("octessera-playing-runtime-error");
    let audio = crate::audio::test_service_with_prep_worker();
    let mut adapter = PiHostAdapter::new_with_data_role(
        Some(audio.clone()),
        root.join("store"),
        root.join("samples"),
        Arc::new(|_| {}),
        false,
        playback_runtime::AudioOutputSet::jack(),
        UsbDataRole::Gadget,
    );
    let payload: serde_json::Value =
        serde_json::from_str(include_str!("../../../config/generated/pi/default.json")).unwrap();
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.apply_config_payload(payload).unwrap();
    runner.skip_startup_splash();
    let mut playback = PlaybackRuntime::new(RuntimeConfig::default());
    playback
        .dispatch_runner_messages(
            runner.messages_with_snapshot().unwrap(),
            &mut runner,
            &mut adapter,
        )
        .unwrap();
    let mut initial_snapshot = playback.last_snapshot().unwrap().clone();
    initial_snapshot["oledFrameRevision"] = serde_json::json!(1);
    adapter
        .core
        .ingest_oled_frame(&playback_runtime::RunnerMessage::OledFrame {
            revision: 1,
            width: 128,
            height: 128,
            format: "rgb565be".into(),
            pixels: vec![0; crate::render::OLED_FRAME_BYTES],
        });
    adapter.core.accept_oled_frame_reference(&initial_snapshot);

    let (seesaw_tx, _seesaw_rx) = mpsc::channel();
    let worker = RenderWorker::spawn(HardwareRenderTargets {
        oled: crate::render::test_oled_output::fake_oled_output(),
        seesaw_tx,
        oled_handoff: None,
        hdmi: crate::render::hdmi::HdmiFramebuffer::new(),
    });
    let initial_oled = adapter
        .core
        .oled_publication_for_snapshot(&initial_snapshot, true)
        .unwrap();
    let source_oled_revision = initial_oled.revision().unwrap();
    worker
        .publish_acknowledged_snapshot(initial_snapshot, initial_oled)
        .unwrap();
    let (physical_oled_revision, physical_oled_pixels) =
        worker.take_acknowledged_startup_oled_frame().unwrap();
    assert!(physical_oled_revision > source_oled_revision);
    audio
        .submit_accepted_oled_frame_shared(physical_oled_revision, physical_oled_pixels)
        .unwrap();
    worker.set_recording_audio(audio.clone());
    worker.mark_first_menu_rendered().unwrap();
    let initial_revision = playback.last_snapshot_revision();
    let before_error_frame = audio.latest_physical_oled_frame().unwrap();
    let mut scheduler = HardwareRuntimeScheduler::new(
        Instant::now()
            .checked_sub(Duration::from_millis(500))
            .expect("test clock should accommodate the initial Play interval"),
        initial_revision,
    );
    let mut native_scenes = NativeScenePump::new(Instant::now() - SNAPSHOT_TICK);
    let (input_tx, input_rx) = mpsc::channel();
    for message in [
        HostMessage::DeviceInput {
            input: serde_json::json!({"type":"button_s","pressed":true}),
            request_snapshot: None,
        },
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
        HostMessage::DeviceInput {
            input: serde_json::json!({"type":"button_s","pressed":false}),
            request_snapshot: None,
        },
    ] {
        input_tx.send(message).unwrap();
    }
    crate::main_runtime_loop::drain_host_messages(
        &input_rx,
        &mut playback,
        &mut runner,
        &mut adapter,
    );
    let mut ui_profiler = crate::ui_profile::UiProfiler::from_process();
    assert!(!crate::main_runtime_loop::maybe_advance_runtime(
        &mut scheduler,
        &mut playback,
        &mut runner,
        &mut adapter,
        &worker,
        &mut ui_profiler,
        &mut native_scenes,
    ));
    assert!(playback.last_status().is_some_and(|status| {
        status.transport == playback_runtime::RuntimeTransportState::Playing
    }));
    let error_snapshot = playback.last_snapshot().unwrap().clone();
    assert!(!error_snapshot["runtimeError"].is_null());
    let error_revision = playback.last_snapshot_revision();
    assert!(error_revision > initial_revision);
    let error_oled = adapter
        .core
        .oled_publication_for_snapshot(&error_snapshot, false)
        .unwrap();
    let error_pixels = error_oled.pixels().unwrap().to_vec();
    assert_ne!(error_pixels, before_error_frame.1.as_ref());
    wait_for_latest_frame(&audio, |revision, pixels| {
        revision > before_error_frame.0 && pixels == error_pixels
    });
    assert_eq!(scheduler.published_snapshot_revision(), error_revision);
    native_scenes.poll(&mut runner);
    assert!(native_scenes
        .submit(
            Instant::now() + SNAPSHOT_TICK,
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

    crate::runtime_loop::dispatch_runtime_message(
        &mut playback,
        &mut runner,
        &mut adapter,
        HostMessage::DeviceInput {
            input: serde_json::json!({"type":"button_a","pressed":true}),
            request_snapshot: None,
        },
    )
    .unwrap();
    let cleared_snapshot = playback.last_snapshot().unwrap().clone();
    assert!(cleared_snapshot["runtimeError"].is_null());
    let cleared_revision = playback.last_snapshot_revision();
    assert!(cleared_revision > error_revision);
    let cleared_oled = adapter
        .core
        .oled_publication_for_snapshot(&cleared_snapshot, false)
        .unwrap();
    let cleared_pixels = cleared_oled.pixels().unwrap().to_vec();
    assert_ne!(cleared_pixels, error_pixels);
    let before_clear_frame = audio.latest_physical_oled_frame().unwrap();
    native_scenes.poll(&mut runner);

    let due_at = Instant::now() + SNAPSHOT_TICK;
    assert!(scheduler.snapshot_publication_due(due_at, &playback));
    let ordinary = native_scenes
        .submit(
            due_at,
            DisplaySnapshotDue {
                one_shot: false,
                continuous: true,
            },
            &playback,
            &mut runner,
            &mut adapter,
            &worker,
        )
        .unwrap();
    scheduler.record_native_scene_capture(ordinary);
    service_render_if_due(due_at, &mut scheduler, &mut playback, &mut adapter, &worker);
    wait_for_latest_frame(&audio, |revision, pixels| {
        revision > before_clear_frame.0 && pixels == cleared_pixels
    });
    worker.publish_shutdown().unwrap();
    let _ = std::fs::remove_dir_all(root);
}

fn wait_for_latest_frame(audio: &crate::audio::AudioService, matches: impl Fn(u64, &[u8]) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        if audio
            .latest_physical_oled_frame()
            .is_some_and(|(revision, pixels)| matches(revision, &pixels))
        {
            return;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    panic!("render worker did not accept the expected physical OLED frame");
}
