use super::*;
use crate::render_loop_queue::SnapshotCommand;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
use octessera_hal::OledSsd1351;
use playback_runtime::{oled_frame::OledPresentationMetrics, NativeRunner, NativeRunnerConfig};
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
use playback_runtime::{
    CoreRunner, DrumHit, HostAdapter, HostMessage, MusicalEvent, PlaybackRuntime,
    RuntimeAdapterError, RuntimeAudioCommand, RuntimeConfig, RuntimePlatformRequest, SyncSource,
};
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
use std::sync::mpsc;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
use std::time::Duration;

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
#[derive(Default)]
struct RecordingHost {
    musical_events: Vec<MusicalEvent>,
    audio_commands: Vec<RuntimeAudioCommand>,
}

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
impl HostAdapter for RecordingHost {
    fn handle_musical_event(&mut self, event: &MusicalEvent) -> Result<(), RuntimeAdapterError> {
        self.musical_events.push(event.clone());
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
        command: &RuntimeAudioCommand,
    ) -> Result<(), RuntimeAdapterError> {
        self.audio_commands.push(command.clone());
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
fn native_latest_slot_supersedes_without_false_success() {
    let worker = RenderWorker::terminated_for_test();
    worker
        .state
        .0
        .lock()
        .unwrap()
        .acknowledged_snapshot_rendered = true;
    let payload: serde_json::Value =
        serde_json::from_str(include_str!("../../../../config/generated/pi/default.json")).unwrap();
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.apply_config_payload(payload).unwrap();
    runner.skip_startup_splash();
    let first = runner.capture_display_scene().unwrap();
    let first_generation = first.generation();
    let first_ack = worker
        .publish_native_scene(first, OledPresentationMetrics::default(), None)
        .unwrap();
    let latest = runner.capture_display_scene().unwrap();
    let latest_generation = latest.generation();
    let latest_ack = worker
        .publish_native_scene(latest, OledPresentationMetrics::default(), None)
        .unwrap();
    let result = first_ack.try_recv().unwrap();
    assert_eq!(result.generation, first_generation);
    assert!(result.result.is_err());
    assert!(result.frame_revision.is_none());
    assert!(latest_ack.try_recv().is_err());
    assert!(matches!(&worker.state.0.lock().unwrap().snapshot,
        Some(SnapshotCommand::Native(scene)) if scene.scene.generation() == latest_generation));
}

#[test]
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
fn native_help_scene_supersedes_pending_ordinary_scene_with_failure_ack() {
    let worker = RenderWorker::terminated_for_test();
    worker
        .state
        .0
        .lock()
        .unwrap()
        .acknowledged_snapshot_rendered = true;
    let payload: serde_json::Value =
        serde_json::from_str(include_str!("../../../../config/generated/pi/default.json")).unwrap();
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.apply_config_payload(payload).unwrap();
    runner.skip_startup_splash();
    for pressed in [true, false] {
        runner
            .send(HostMessage::DeviceInput {
                input: serde_json::json!({"type":"button_s","pressed":pressed}),
                request_snapshot: None,
            })
            .unwrap();
    }
    let ordinary = runner.capture_display_scene().unwrap();
    let ordinary_generation = ordinary.generation();
    let ordinary_ack = worker
        .publish_native_scene(ordinary, OledPresentationMetrics::default(), None)
        .unwrap();
    for input in [
        serde_json::json!({"type":"button_shift","pressed":true}),
        serde_json::json!({"type":"button_fn","pressed":true}),
        serde_json::json!({"type":"encoder_press","id":"main"}),
    ] {
        runner
            .send_music_first(HostMessage::DeviceInput {
                input,
                request_snapshot: Some(false),
            })
            .unwrap();
    }
    let help = runner.capture_display_scene().unwrap();
    let help_generation = help.generation();
    assert_eq!(help.into_snapshot()["display"]["title"], "Help: Build");
    let _help_ack = worker
        .publish_native_scene(
            runner.capture_display_scene().unwrap(),
            OledPresentationMetrics::default(),
            None,
        )
        .unwrap();
    let superseded = ordinary_ack.try_recv().unwrap();
    assert_eq!(superseded.generation, ordinary_generation);
    assert!(superseded.result.is_err());
    assert!(matches!(&worker.state.0.lock().unwrap().snapshot,
        Some(SnapshotCommand::Native(scene)) if scene.scene.generation() >= help_generation));
}

#[test]
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
fn native_worker_returns_physically_accepted_typed_frame() {
    let (seesaw_tx, _seesaw_rx) = mpsc::channel();
    let worker = RenderWorker::spawn(HardwareRenderTargets {
        oled: OledSsd1351::new().unwrap(),
        seesaw_tx,
        oled_handoff: None,
        hdmi: crate::render::hdmi::HdmiFramebuffer::new(),
    });
    worker
        .state
        .0
        .lock()
        .unwrap()
        .acknowledged_snapshot_rendered = true;
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let scene = runner.capture_display_scene().unwrap();
    let generation = scene.generation();
    let ack = worker
        .publish_native_scene(scene, OledPresentationMetrics::default(), None)
        .unwrap();
    let accepted = ack.recv_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!(accepted.generation, generation);
    assert!(accepted.result.is_ok());
    let revision = accepted.frame_revision.unwrap();
    assert!(revision > 0);
    worker.publish_shutdown().unwrap();
}

#[test]
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
fn unchanged_scene_generation_is_acknowledged_without_a_new_frame_revision() {
    let (seesaw_tx, _seesaw_rx) = mpsc::channel();
    let worker = RenderWorker::spawn(HardwareRenderTargets {
        oled: OledSsd1351::new().unwrap(),
        seesaw_tx,
        oled_handoff: None,
        hdmi: crate::render::hdmi::HdmiFramebuffer::new(),
    });
    worker
        .state
        .0
        .lock()
        .unwrap()
        .acknowledged_snapshot_rendered = true;
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let payload: serde_json::Value =
        serde_json::from_str(include_str!("../../../../config/generated/pi/default.json")).unwrap();
    runner.apply_config_payload(payload).unwrap();
    runner.skip_startup_splash();
    let mut playback = PlaybackRuntime::new(RuntimeConfig::default());
    let mut host = RecordingHost::default();
    playback
        .dispatch_runner_messages(
            runner.messages_with_snapshot().unwrap(),
            &mut runner,
            &mut host,
        )
        .unwrap();
    for pressed in [true, false] {
        playback
            .dispatch_host_message_music_first(
                HostMessage::DeviceInput {
                    input: serde_json::json!({"type":"button_s","pressed":pressed}),
                    request_snapshot: None,
                },
                &mut runner,
                &mut host,
            )
            .unwrap();
    }
    playback
        .advance_duration_music_first_with_output(
            Duration::from_millis(500),
            &mut runner,
            &mut host,
        )
        .unwrap();
    std::thread::sleep(Duration::from_millis(220));
    let first = runner.capture_display_scene().unwrap();
    let first_generation = first.generation();
    let first_ack = worker
        .publish_native_scene(first, OledPresentationMetrics::default(), None)
        .unwrap();
    let accepted_first = first_ack.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(accepted_first.result.is_ok());

    playback
        .advance_duration_music_first_with_output(
            Duration::from_millis(500),
            &mut runner,
            &mut host,
        )
        .unwrap();
    std::thread::sleep(Duration::from_millis(220));
    let second = runner.capture_display_scene().unwrap();
    let second_generation = second.generation();
    let second_ack = worker
        .publish_native_scene(second, OledPresentationMetrics::default(), None)
        .unwrap();
    let accepted_second = second_ack.recv_timeout(Duration::from_secs(2)).unwrap();

    assert_ne!(first_generation, second_generation);
    assert_eq!(accepted_second.generation, second_generation);
    assert!(accepted_second.result.is_ok());
    assert_eq!(
        accepted_second.frame_revision,
        accepted_first.frame_revision
    );
    worker.publish_shutdown().unwrap();
}

#[test]
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
fn native_to_terminal_source_revision_collision_acknowledges_preserving_teardown() {
    let root = std::env::temp_dir().join(format!(
        "octessera-native-terminal-recording-{}",
        std::process::id()
    ));
    let (audio, _, _, _) = crate::audio::test_service_with_recording_dir(root.clone());
    audio.start_recording_audio_oled_with_seed(1, None).unwrap();
    let (seesaw_tx, _seesaw_rx) = mpsc::channel();
    let worker = RenderWorker::spawn(HardwareRenderTargets {
        oled: OledSsd1351::new().unwrap(),
        seesaw_tx,
        oled_handoff: None,
        hdmi: crate::render::hdmi::HdmiFramebuffer::new(),
    });
    worker
        .state
        .0
        .lock()
        .unwrap()
        .acknowledged_snapshot_rendered = true;
    worker.set_recording_audio(audio.clone());
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let scene = runner.capture_display_scene().unwrap();
    let native_ack = worker
        .publish_native_scene(scene, OledPresentationMetrics::default(), None)
        .unwrap();
    let accepted = native_ack.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(accepted.result.is_ok());

    let mut snapshot = serde_json::json!({"oledFrameRevision": 2});
    snapshot["display"] = serde_json::json!({"off": false});
    worker
        .publish_terminal_preserving(
            snapshot,
            OledFramePublication::test_native(
                2,
                vec![0x67; playback_runtime::oled_frame::OLED_FRAME_BYTES],
            ),
        )
        .unwrap();
    assert!(worker.is_terminated());
    audio.test_push_recording_samples(&vec![0; 8_820]).unwrap();
    let recording = audio.stop_recording_with_outcome().unwrap().unwrap();
    assert!(recording.frames_written > 0);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
fn worker_submits_accepted_native_frame_to_existing_audio_recording_ingress() {
    let root = std::env::temp_dir().join(format!(
        "octessera-native-render-recording-{}",
        std::process::id()
    ));
    let (audio, _, _, _) = crate::audio::test_service_with_recording_dir(root.clone());
    audio.start_recording_audio_oled_with_seed(1, None).unwrap();
    let (seesaw_tx, _seesaw_rx) = mpsc::channel();
    let worker = RenderWorker::spawn(HardwareRenderTargets {
        oled: OledSsd1351::new().unwrap(),
        seesaw_tx,
        oled_handoff: None,
        hdmi: crate::render::hdmi::HdmiFramebuffer::new(),
    });
    worker
        .state
        .0
        .lock()
        .unwrap()
        .acknowledged_snapshot_rendered = true;
    worker.set_recording_audio(audio.clone());
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.skip_startup_splash();
    let scene = runner.capture_display_scene().unwrap();
    let ack = worker
        .publish_native_scene(scene, OledPresentationMetrics::default(), None)
        .unwrap();
    let result = ack.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(result.result.is_ok());
    assert!(result.frame_revision.is_some());
    audio.test_push_recording_samples(&vec![0; 8_820]).unwrap();
    let recording = audio.stop_recording_with_outcome().unwrap().unwrap();
    assert!(recording.frames_written > 0);
    worker.publish_shutdown().unwrap();
    let _ = std::fs::remove_dir_all(root);
}

#[test]
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
fn blocked_worker_publishes_beat_onset_then_expiry_without_blocking_scene_submit() {
    let (seesaw_tx, _seesaw_rx) = mpsc::channel();
    let worker = RenderWorker::spawn(HardwareRenderTargets {
        oled: OledSsd1351::new().unwrap(),
        seesaw_tx,
        oled_handoff: None,
        hdmi: crate::render::hdmi::HdmiFramebuffer::new(),
    });
    worker
        .state
        .0
        .lock()
        .unwrap()
        .acknowledged_snapshot_rendered = true;
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    worker.state.0.lock().unwrap().native_render_gate = Some((entered_tx, release_rx));
    let payload: serde_json::Value =
        serde_json::from_str(include_str!("../../../../config/generated/pi/default.json")).unwrap();
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.apply_config_payload(payload).unwrap();
    runner.skip_startup_splash();
    let mut playback = PlaybackRuntime::new(RuntimeConfig::default());
    let mut host = RecordingHost::default();
    playback
        .dispatch_runner_messages(
            runner.messages_with_snapshot().unwrap(),
            &mut runner,
            &mut host,
        )
        .unwrap();
    for pressed in [true, false] {
        playback
            .dispatch_host_message_music_first(
                HostMessage::DeviceInput {
                    input: serde_json::json!({"type":"button_s","pressed":pressed}),
                    request_snapshot: None,
                },
                &mut runner,
                &mut host,
            )
            .unwrap();
    }
    assert_eq!(
        playback.last_status().unwrap().transport,
        playback_runtime::RuntimeTransportState::Playing
    );
    let onset_output = playback
        .advance_duration_music_first_with_output(
            Duration::from_millis(500),
            &mut runner,
            &mut host,
        )
        .unwrap();
    assert!(!onset_output
        .messages
        .iter()
        .any(|message| matches!(message, playback_runtime::RunnerMessage::Snapshot { .. })));
    assert!(!host.musical_events.is_empty());
    host.musical_events.clear();
    let onset = runner.capture_display_scene().unwrap();
    let onset_generation = onset.generation();
    let onset_ack = worker
        .publish_native_scene(onset, OledPresentationMetrics::default(), None)
        .unwrap();
    entered_rx.recv_timeout(Duration::from_secs(1)).unwrap();

    std::thread::sleep(Duration::from_millis(20));
    let pulse_before = playback.last_status().unwrap().current_ppqn_pulse;
    for _ in 0..4 {
        let continued = playback
            .advance_duration_music_first_with_output(
                Duration::from_millis(8),
                &mut runner,
                &mut host,
            )
            .unwrap();
        assert!(!continued
            .messages
            .iter()
            .any(|message| matches!(message, playback_runtime::RunnerMessage::Snapshot { .. })));
    }
    assert!(playback.last_status().unwrap().current_ppqn_pulse > pulse_before);
    assert!(onset_ack.try_recv().is_err());
    release_tx.send(()).unwrap();
    let onset_frame = onset_ack.recv_timeout(Duration::from_secs(2)).unwrap();

    std::thread::sleep(Duration::from_millis(220));
    let expiry_messages = runner
        .send_music_first(HostMessage::TransportPulseStep {
            pulses: 0,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();
    assert!(!expiry_messages
        .iter()
        .any(|message| matches!(message, playback_runtime::RunnerMessage::Snapshot { .. })));
    let expiry = runner.capture_display_scene().unwrap();
    let expiry_generation = expiry.generation();
    let expiry_ack = worker
        .publish_native_scene(expiry, OledPresentationMetrics::default(), None)
        .unwrap();
    let expiry_frame = expiry_ack.recv_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!(onset_frame.generation, onset_generation);
    assert_eq!(expiry_frame.generation, expiry_generation);
    assert!(onset_frame.result.is_ok());
    assert!(expiry_frame.result.is_ok());
    let onset_revision = onset_frame.frame_revision.unwrap();
    let expiry_revision = expiry_frame.frame_revision.unwrap();
    assert!(expiry_revision > onset_revision);
    runner.acknowledge_display_scene(onset_generation);
    assert!(runner.display_scene_pending());
    runner.acknowledge_display_scene(expiry_generation);
    assert!(!runner.display_scene_pending());
    worker.publish_shutdown().unwrap();
}

pub(super) fn wait_for_test_native_render_gate(state: &Arc<(Mutex<RenderState>, Condvar)>) {
    let gate = state
        .0
        .lock()
        .ok()
        .and_then(|mut state| state.native_render_gate.take());
    if let Some((entered, release)) = gate {
        let _ = entered.send(());
        let _ = release.recv();
    }
}
