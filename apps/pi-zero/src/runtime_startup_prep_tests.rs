use super::*;
use crate::audio::{test_service_with_prep_worker, test_service_with_recording_dir};
use crate::candidate_readiness::CandidateReadiness;
use crate::hardware_runtime_scheduler::HardwareRuntimeScheduler;
use crate::render::{HardwareRenderTargets, OLED_FRAME_BYTES};
use crate::render_loop::RenderWorker;
use playback_runtime::{
    CoreRunner, HostAdapter, RunnerMessage, RuntimeAudioCommand, RuntimeStoreResult,
};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;

#[test]
fn pi_startup_accepts_the_native_runner_initial_audio_result_shape() {
    let audio = test_service_with_prep_worker();
    let root = std::env::temp_dir().join(format!(
        "octessera-pi-native-runner-prep-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut adapter = PiPlaybackHostAdapter::new(
        Some(audio),
        root.join("store"),
        root.join("samples"),
        Arc::new(|_| {}),
        false,
        playback_runtime::AudioOutputSet::jack(),
    );
    let mut playback = PlaybackRuntime::new(RuntimeConfig::default());
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let messages = runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::LoadDefaultResult { payload: None },
        })
        .unwrap();
    let command = messages
        .iter()
        .find_map(|message| match message {
            RunnerMessage::AudioCommands { commands } => commands.iter().find_map(|command| {
                matches!(
                    command,
                    RuntimeAudioCommand::SetAudioConfig {
                        request_id: None,
                        ..
                    }
                )
                .then(|| command.clone())
            }),
            _ => None,
        })
        .expect("NativeRunner should emit its initial unidentified audio config");
    adapter.handle_audio_command(&command).unwrap();

    wait_for_initial_audio_prep(&mut playback, &mut runner, &mut adapter).unwrap();

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn pi_v1_persisted_startup_sleep_remains_due_after_scheduler_creation() {
    let root = std::env::temp_dir().join(format!(
        "octessera-pi-startup-sleep-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let store = root.join("store");
    std::fs::create_dir_all(&store).unwrap();
    let mut payload: serde_json::Value =
        serde_json::from_str(include_str!("../../../config/generated/pi/default.json")).unwrap();
    payload["runtimeConfig"]["screenSleepSeconds"] = serde_json::json!(10);
    payload["runtimeConfig"]["dimTimerSeconds"] = serde_json::json!(0);
    let documents = playback_runtime::split_system_patch_documents(&payload).unwrap();
    std::fs::write(
        store.join("system.json"),
        serde_json::to_vec(&documents.system).unwrap(),
    )
    .unwrap();
    std::fs::write(
        store.join("default.patch.json"),
        serde_json::to_vec(&documents.patch).unwrap(),
    )
    .unwrap();

    let mut adapter = PiPlaybackHostAdapter::new(
        None,
        store,
        root.join("samples"),
        Arc::new(|_| {}),
        false,
        playback_runtime::AudioOutputSet::jack(),
    );
    let (mut playback, mut runner) = init_runtime(AudioOptimization::Latency, false);
    runner.skip_startup_splash();
    initialize_host_state(&mut playback, &mut runner, &mut adapter).unwrap();
    let loaded_config = runner.test_config_payload();
    assert_eq!(loaded_config["runtimeConfig"]["screenSleepSeconds"], 10);
    assert_eq!(loaded_config["runtimeConfig"]["dimTimerSeconds"], 0);

    let sleep_deadline = runner
        .next_timed_display_snapshot_deadline()
        .expect("persisted sleep should have a deadline");
    let scheduler_now = sleep_deadline + Duration::from_secs(11);
    let scheduler = HardwareRuntimeScheduler::new(scheduler_now, playback.last_snapshot_revision());

    assert!(
        scheduler
            .display_snapshot_due(scheduler_now, &runner, &playback)
            .one_shot
    );
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
#[test]
fn pi_prepared_startup_seeds_static_recording_from_the_acknowledged_menu() {
    let root = std::env::temp_dir().join(format!(
        "octessera-pi-prepared-oled-recording-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let (audio, control_rx, _event_rx, prep_result_tx) =
        test_service_with_recording_dir(root.join("actual"));
    crate::host_audio_prep::spawn_audio_control_worker(control_rx, audio.clone(), prep_result_tx);
    let mut prepared = prepared_runtime(audio.clone(), &root, None);
    let snapshot = prepared.playback.last_snapshot().unwrap().clone();
    assert!(
        crate::normal_menu::is_normal_menu_snapshot(&snapshot),
        "{snapshot}"
    );
    let source_oled = prepared
        .adapter
        .oled_publication_for_snapshot(&snapshot, true)
        .unwrap();
    let source_revision = source_oled.revision().unwrap();
    let expected_pixels = source_oled.shared_pixels().unwrap();
    assert_eq!(expected_pixels.len(), OLED_FRAME_BYTES);
    assert!(expected_pixels.iter().any(|pixel| *pixel != 0));

    let worker = render_worker();
    prepared.publish_acknowledged_snapshot(&worker).unwrap();
    let (physical_revision, physical_pixels) = audio.latest_physical_oled_frame().unwrap();
    assert!(physical_revision > source_revision);
    assert_eq!(physical_pixels.as_ref(), expected_pixels.as_ref());

    audio.start_recording_audio_oled_from_latest(1).unwrap();
    audio.test_push_recording_samples(&vec![0; 8_820]).unwrap();
    let actual = audio.stop_recording_with_outcome().unwrap().unwrap();
    let (reference, _, _, _) = test_service_with_recording_dir(root.join("reference"));
    reference
        .start_recording_audio_oled_with_seed(
            1,
            Some((physical_revision, physical_pixels.to_vec())),
        )
        .unwrap();
    reference
        .test_push_recording_samples(&vec![0; 8_820])
        .unwrap();
    let expected = reference.stop_recording_with_outcome().unwrap().unwrap();
    let actual_frames = avi_video_payloads(&actual.path);
    let expected_frames = avi_video_payloads(&expected.path);
    let menu_frame_position = actual_frames
        .iter()
        .position(|frame| frame == &expected_frames[0]);
    assert_eq!(menu_frame_position, Some(0), "menu frame position");
    worker.publish_shutdown().unwrap();
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
#[test]
fn pi_prepared_startup_write_failure_keeps_recording_seed_and_ready_marker_absent() {
    let root = std::env::temp_dir().join(format!(
        "octessera-pi-prepared-oled-failure-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let marker = root.join("candidate-ready.json");
    let (audio, control_rx, _event_rx, prep_result_tx) =
        test_service_with_recording_dir(root.join("recording"));
    crate::host_audio_prep::spawn_audio_control_worker(control_rx, audio.clone(), prep_result_tx);
    let prepared = prepared_runtime(audio.clone(), &root, Some(marker.clone()));
    let worker = render_worker();
    worker.fail_next_startup_oled_write_for_test();

    prepared.run(worker);

    assert!(audio.latest_physical_oled_frame().is_none());
    assert!(!marker.exists());
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
fn prepared_runtime(
    audio: crate::audio::AudioService,
    root: &std::path::Path,
    marker: Option<std::path::PathBuf>,
) -> PreparedRuntime {
    let defaults: serde_json::Value =
        serde_json::from_str(include_str!("../../../config/generated/pi/default.json")).unwrap();
    crate::pi_store_test_support::write_pair(&root.join("store"), &defaults);
    let samples = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../samples")
        .canonicalize()
        .unwrap();
    let mut adapter = PiPlaybackHostAdapter::new(
        Some(audio),
        root.join("store"),
        samples,
        Arc::new(|_| {}),
        false,
        playback_runtime::AudioOutputSet::jack(),
    );
    adapter.set_test_midi_backend([], [], []);
    let (mut playback, mut runner) = init_runtime(AudioOptimization::Latency, false);
    runner.skip_startup_splash();
    initialize_host_state(&mut playback, &mut runner, &mut adapter).unwrap();
    crate::runtime_loop::dispatch_runtime_message(
        &mut playback,
        &mut runner,
        &mut adapter,
        HostMessage::TransportPulseStep {
            pulses: 0,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(true),
        },
    )
    .unwrap();
    wait_for_initial_audio_prep(&mut playback, &mut runner, &mut adapter).unwrap();
    let (_, midi_rx) = mpsc::channel();
    let (input_tx, input_rx) = mpsc::channel();
    let (_, encoder_rx) = mpsc::channel();
    PreparedRuntime {
        midi_rx,
        input_rx,
        encoder_rx,
        playback,
        runner,
        adapter,
        candidate_readiness: CandidateReadiness::new(marker, "pi-recording-test".into()),
        keyboard: crate::usb_keyboard::KeyboardCapture::spawn(input_tx, false),
        #[cfg(feature = "hardware-raspberry-pi-zero-2w")]
        audio_load_rx: None,
    }
}

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
fn render_worker() -> RenderWorker {
    let (seesaw_tx, _seesaw_rx) = mpsc::channel();
    RenderWorker::spawn(HardwareRenderTargets {
        oled: crate::render::test_oled_output::fake_oled_output(),
        seesaw_tx,
        oled_handoff: None,
        hdmi: crate::render::hdmi::HdmiFramebuffer::new(),
    })
}

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
fn avi_video_payloads(path: &std::path::Path) -> Vec<Vec<u8>> {
    let bytes = std::fs::read(path).unwrap();
    let list = bytes
        .windows(12)
        .position(|window| &window[..4] == b"LIST" && &window[8..12] == b"movi")
        .unwrap();
    let mut offset = list + 12;
    let end = list + 8 + u32::from_le_bytes(bytes[list + 4..list + 8].try_into().unwrap()) as usize;
    let mut videos = Vec::new();
    while offset + 8 <= end {
        let size = u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().unwrap()) as usize;
        let payload_end = offset + 8 + size;
        if &bytes[offset..offset + 4] == b"00dc" {
            videos.push(bytes[offset + 8..payload_end].to_vec());
        }
        offset = payload_end + size % 2;
    }
    videos
}
