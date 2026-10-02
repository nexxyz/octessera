use crate::autoaux_menu;
use crate::autoaux_sequence::{AutoAuxSequence, Phase};
use crate::hardware_runtime_scheduler::DisplaySnapshotDue;
use crate::host_adapter::PiPlaybackHostAdapter;
use crate::raspberry_native_scene::NativeScenePump;
use crate::render::HardwareRenderTargets;
use crate::render_loop::RenderWorker;
use crate::timing_input::{complete_study, fail_study, TimingInput, TimingStudyEvidence};
use playback_runtime::{HostMessage, NativeRunner, PlaybackRuntime};
use playback_runtime::{NativeRunnerConfig, RuntimeConfig, SyncSource, UsbDataRole};
use std::path::Path;
use std::process::Command;
use std::sync::mpsc;
use std::sync::Arc;
use std::time::{Duration, Instant};

const FAIL_CHILD_ENV: &str = "OCTESSERA_TEST_TIMING_FAILURE_CHILD";

struct AudioKeepAlive {
    _control_rx: std::sync::mpsc::Receiver<crate::audio::AudioControlRequest>,
    _event_rx: rodio_engine_source::EngineEventReceiver,
    _prep_tx: std::sync::mpsc::Sender<HostMessage>,
}

fn root() -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "octessera-raspberry-autoaux-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

use std::time::{SystemTime, UNIX_EPOCH};

fn runtime(
    root: &Path,
) -> (
    PlaybackRuntime,
    NativeRunner,
    PiPlaybackHostAdapter,
    AudioKeepAlive,
) {
    let payload: serde_json::Value =
        serde_json::from_str(include_str!("../../../config/generated/pi/default.json")).unwrap();
    crate::pi_store_test_support::write_pair(&root.join("store"), &payload);
    let (audio, control_rx, event_rx, prep_tx) =
        crate::audio::test_service_with_recording_dir(root.join("recording"));
    let mut adapter = PiPlaybackHostAdapter::new_with_data_role(
        Some(audio),
        root.join("store"),
        root.join("samples"),
        Arc::new(|_| {}),
        false,
        playback_runtime::AudioOutputSet::jack(),
        UsbDataRole::Gadget,
    );
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.apply_config_payload(payload).unwrap();
    runner.skip_startup_splash();
    let mut playback = PlaybackRuntime::new(RuntimeConfig {
        sync_source: SyncSource::Internal,
        ..RuntimeConfig::default()
    });
    let output = playback
        .dispatch_runner_messages(
            runner.messages_with_snapshot().unwrap(),
            &mut runner,
            &mut adapter,
        )
        .unwrap();
    crate::runtime_loop::process_runtime_output(&mut playback, &mut runner, &mut adapter, output)
        .unwrap();
    (
        playback,
        runner,
        adapter,
        AudioKeepAlive {
            _control_rx: control_rx,
            _event_rx: event_rx,
            _prep_tx: prep_tx,
        },
    )
}

fn dispatch(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    adapter: &mut PiPlaybackHostAdapter,
    message: HostMessage,
) -> Result<(), String> {
    crate::runtime_loop::dispatch_runtime_message(playback, runner, adapter, message)
}

fn scene_worker(playback: &PlaybackRuntime, adapter: &mut PiPlaybackHostAdapter) -> RenderWorker {
    let (seesaw_tx, _seesaw_rx) = mpsc::channel();
    let worker = RenderWorker::spawn(HardwareRenderTargets {
        oled: crate::render::test_oled_output::fake_oled_output(),
        seesaw_tx,
        oled_handoff: None,
        hdmi: crate::render::hdmi::HdmiFramebuffer::new(),
    });
    let snapshot = playback.last_snapshot().unwrap().clone();
    let oled = adapter
        .oled_publication_for_snapshot(&snapshot, true)
        .unwrap();
    worker
        .publish_acknowledged_snapshot(snapshot, oled)
        .unwrap();
    worker
}

#[test]
fn native_worker_save_result_produces_the_receipt() {
    let root = root();
    let (mut playback, mut runner, mut adapter, _audio_keep_alive) = runtime(&root);
    let worker = scene_worker(&playback, &mut adapter);
    let (original, targets) = {
        let mut send = |playback: &mut PlaybackRuntime,
                        runner: &mut NativeRunner,
                        message: HostMessage| {
            crate::runtime_loop::dispatch_runtime_message(playback, runner, &mut adapter, message)
        };
        autoaux_menu::require_stopped_normal_menu(&playback, "Raspberry").unwrap();
        autoaux_menu::enable_study_auto_save(&mut playback, &mut runner, &mut send, "Raspberry")
            .unwrap();
        autoaux_menu::navigate_to_cutoff(&mut playback, &mut runner, &mut send, "Raspberry")
            .unwrap();
        autoaux_menu::preflight_aux_cutoff(&mut playback, &mut runner, &mut send, "Raspberry")
            .unwrap()
    };
    for pressed in [true, false] {
        dispatch(
            &mut playback,
            &mut runner,
            &mut adapter,
            crate::input::neokey_message(1, pressed).unwrap(),
        )
        .unwrap();
    }
    adapter.timing_evidence = Some(TimingStudyEvidence::default());
    let mut scenes = NativeScenePump::new(Instant::now());
    scenes.set_timing_cutoff_targets(targets);
    let mut timing = TimingInput::for_test(AutoAuxSequence::new(Instant::now()), original);
    let deadline = Instant::now() + Duration::from_secs(17);
    loop {
        scenes.poll(&mut runner);
        let completed = timing
            .tick(Instant::now(), &mut playback, &mut runner, &mut adapter)
            .unwrap();
        crate::runtime_loop::handle_deferred_host_work(&mut playback, &mut runner, &mut adapter)
            .unwrap();
        if let Some(captured_at) = scenes.submit(
            Instant::now() + crate::hardware_runtime_scheduler::SNAPSHOT_TICK,
            DisplaySnapshotDue::default(),
            &playback,
            &mut runner,
            &mut adapter,
            &worker,
        ) {
            assert!(captured_at <= Instant::now());
        }
        if completed {
            break;
        }
        assert!(Instant::now() < deadline, "native timing save timed out");
        std::thread::sleep(Duration::from_millis(2));
    }
    let (request_id, elapsed) = timing.sequence.save_completion.clone().unwrap();
    assert!(!request_id.is_empty());
    assert!(elapsed >= Duration::from_secs(2));
    assert!(elapsed < crate::autoaux_sequence::SAVE_COMPLETION_TIMEOUT);
    assert!(timing.sequence.final_revision.is_some());
    assert!(root.join("store/default.patch.json").is_file());
    complete_study(&timing, &mut adapter, scenes.timing_cutoff_acceptances()).unwrap();
    assert!(adapter.timing_evidence.is_none());

    adapter.timing_evidence = Some(TimingStudyEvidence::default());
    let started = Instant::now() - Duration::from_secs(3);
    let mut no_commands = TimingInput::for_test(AutoAuxSequence::new(started), original);
    no_commands.sequence.phase = Phase::AwaitSave { started };
    no_commands.sequence.final_revision = Some(999);
    no_commands.sequence.save_completion = Some(("not-a-real-save".into(), Duration::from_secs(1)));
    assert!(complete_study(
        &no_commands,
        &mut adapter,
        scenes.timing_cutoff_acceptances()
    )
    .unwrap_err()
    .contains("two distinct synth Cutoff values"));
    worker.publish_shutdown().unwrap();
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn completed_save_cannot_pass_without_physical_scene_acknowledgements() {
    let root = root();
    let (_playback, _runner, mut adapter, _audio_keep_alive) = runtime(&root);
    adapter.timing_evidence = Some(TimingStudyEvidence::default());
    let scenes = NativeScenePump::new(Instant::now());
    let started = Instant::now() - Duration::from_secs(3);
    let mut timing = TimingInput::for_test(AutoAuxSequence::new(started), 90);
    timing.sequence.phase = Phase::AwaitSave { started };
    timing.sequence.final_revision = Some(8);
    timing.sequence.save_completion = Some(("save-8".into(), Duration::from_secs(1)));
    assert!(
        complete_study(&timing, &mut adapter, scenes.timing_cutoff_acceptances())
            .unwrap_err()
            .contains("physically publish both Cutoff plateaus")
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn study_failure_prints_one_board_marker_and_exits_two() {
    if std::env::var_os(FAIL_CHILD_ENV).is_some() {
        fail_study::<PiPlaybackHostAdapter>("child failure");
    }
    let output = Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg("raspberry_runtime::timing_input_tests::study_failure_prints_one_board_marker_and_exits_two")
        .arg("--nocapture")
        .env(FAIL_CHILD_ENV, "1")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(stderr.matches("raspberry-autoaux-failed:").count(), 1);
    assert!(stderr.contains("raspberry-autoaux-failed: child failure"));
}
