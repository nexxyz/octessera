use super::*;
use crate::autoaux_sequence::{AutoAuxSequence, Phase};
use crate::hardware_runtime_scheduler::DisplaySnapshotDue;
use crate::host_adapter::PiPlaybackHostAdapter;
use crate::render::HardwareRenderTargets;
use crate::render_loop::RenderWorker;
use crate::runtime_loop::store_autoaux_result_observation;
use octessera_hal::OledSsd1351;
use playback_runtime::{
    NativeRunnerConfig, RuntimeConfig, RuntimeErrorCode, RuntimeErrorDomain, RuntimeErrorFacts,
    RuntimeOperation, SyncSource, UsbDataRole,
};
use std::process::Command;
use std::sync::mpsc;
use std::sync::Arc;
use std::time::{Duration, Instant};

const FAIL_CHILD_ENV: &str = "OCTESSERA_TEST_AUTOAUX_FAILURE_CHILD";

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
    let payload: serde_json::Value =
        serde_json::from_str(include_str!("../../../config/generated/pi/default.json")).unwrap();
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
        oled: OledSsd1351::new().unwrap(),
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
fn native_autoaux_worker_save_result_produces_the_receipt() {
    let root = root();
    let (mut playback, mut runner, mut adapter, _audio_keep_alive) = runtime(&root);
    let worker = scene_worker(&playback, &mut adapter);
    let original = {
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
        autoaux_menu::cutoff_display_value(&playback, "Raspberry").unwrap()
    };
    for delta in [1, -1] {
        dispatch(
            &mut playback,
            &mut runner,
            &mut adapter,
            crate::input::encoder_turn_message("encoder_aux_1", delta),
        )
        .unwrap();
    }
    for pressed in [true, false] {
        dispatch(
            &mut playback,
            &mut runner,
            &mut adapter,
            crate::input::neokey_message(1, pressed).unwrap(),
        )
        .unwrap();
    }
    adapter.begin_autoaux_evidence();
    let mut scenes = NativeScenePump::new(Instant::now());
    scenes.begin_autoaux_cutoff_evidence([original + 1, original + 2]);
    let mut autoaux = RaspberryAutoAux {
        sequence: AutoAuxSequence::new(Instant::now()),
        starting_cutoff: original,
    };
    let deadline = Instant::now() + Duration::from_secs(17);
    let completed = loop {
        let now = Instant::now();
        scenes.poll(&mut runner);
        let completed = autoaux
            .tick(now, &mut playback, &mut runner, &mut adapter, &scenes)
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
            break completed;
        }
        assert!(
            Instant::now() < deadline,
            "native AutoAux worker save timed out"
        );
        std::thread::sleep(Duration::from_millis(2));
    };
    assert!(completed);
    let (request_id, elapsed) = autoaux.sequence.save_completion.as_ref().unwrap();
    assert!(!request_id.is_empty());
    assert!(*elapsed >= Duration::from_secs(2));
    assert!(*elapsed < crate::autoaux_sequence::SAVE_COMPLETION_TIMEOUT);
    assert!(autoaux.sequence.final_revision.is_some());
    assert!(root.join("store/default.json").is_file());
    assert!(scenes.autoaux_cutoff_acceptances().is_some());
    assert!(!adapter.autoaux_active());
    adapter.begin_autoaux_evidence();
    let started = Instant::now() - Duration::from_secs(3);
    let mut no_commands = RaspberryAutoAux {
        sequence: AutoAuxSequence::new(started),
        starting_cutoff: original,
    };
    no_commands.sequence.phase = Phase::AwaitSave { started };
    no_commands.sequence.final_revision = Some(999);
    no_commands.sequence.save_completion = Some(("not-a-real-save".into(), Duration::from_secs(1)));
    assert!(no_commands
        .tick(
            Instant::now(),
            &mut playback,
            &mut runner,
            &mut adapter,
            &scenes,
        )
        .unwrap_err()
        .contains("two distinct synth Cutoff values"));
    worker.publish_shutdown().unwrap();
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn completed_save_cannot_pass_without_physical_scene_acknowledgements() {
    let root = root();
    let (mut playback, mut runner, mut adapter, _audio_keep_alive) = runtime(&root);
    for pressed in [true, false] {
        dispatch(
            &mut playback,
            &mut runner,
            &mut adapter,
            crate::input::neokey_message(1, pressed).unwrap(),
        )
        .unwrap();
    }
    adapter.begin_autoaux_evidence();
    let scenes = NativeScenePump::new(Instant::now());
    let started = Instant::now() - Duration::from_secs(3);
    let mut autoaux = RaspberryAutoAux {
        sequence: AutoAuxSequence::new(started),
        starting_cutoff: 90,
    };
    autoaux.sequence.phase = Phase::AwaitSave { started };
    autoaux.sequence.final_revision = Some(8);
    autoaux.sequence.save_completion = Some(("save-8".into(), Duration::from_secs(1)));
    assert!(autoaux
        .tick(
            Instant::now(),
            &mut playback,
            &mut runner,
            &mut adapter,
            &scenes,
        )
        .unwrap_err()
        .contains("two physical Cutoff frames"));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn buffered_save_completion_is_checked_before_the_sequence_timeout() {
    for (accepted_before_deadline, expected_error) in
        [(true, "physical Cutoff frames"), (false, "timed out")]
    {
        let root = root();
        let (mut playback, mut runner, mut adapter, _audio_keep_alive) = runtime(&root);
        for pressed in [true, false] {
            dispatch(
                &mut playback,
                &mut runner,
                &mut adapter,
                crate::input::neokey_message(1, pressed).unwrap(),
            )
            .unwrap();
        }
        adapter.begin_autoaux_evidence();
        let tick_at = Instant::now();
        let started =
            tick_at - crate::autoaux_sequence::SAVE_COMPLETION_TIMEOUT - Duration::from_millis(10);
        let deadline = started + crate::autoaux_sequence::SAVE_COMPLETION_TIMEOUT;
        let accepted_at = if accepted_before_deadline {
            deadline - Duration::from_millis(1)
        } else {
            deadline + Duration::from_millis(1)
        };
        let mut autoaux = RaspberryAutoAux {
            sequence: AutoAuxSequence::new(started),
            starting_cutoff: 90,
        };
        autoaux.sequence.phase = Phase::AwaitSave { started };
        autoaux.sequence.final_revision = Some(41);
        adapter.set_autoaux_expected_revision(Some(41));
        let result = RuntimeStoreResult::Identified {
            result: Box::new(RuntimeStoreResult::SaveDefaultResult {
                ok: true,
                is_auto: Some(true),
            }),
            request_id: "buffered-save-41".into(),
            revision: Some(41),
        };
        adapter.observe_autoaux_store_result(
            store_autoaux_result_observation(&result).unwrap(),
            accepted_at,
        );
        let error = autoaux
            .tick(
                tick_at,
                &mut playback,
                &mut runner,
                &mut adapter,
                &NativeScenePump::new(tick_at),
            )
            .unwrap_err();
        assert!(error.contains(expected_error), "{error}");
        assert_eq!(
            autoaux.sequence.save_completion.is_some(),
            accepted_before_deadline
        );
        let _ = std::fs::remove_dir_all(root);
    }
}

#[test]
fn buffered_completion_deadline_uses_worker_acceptance_time() {
    for accepted_before_deadline in [true, false] {
        let root = root();
        let (mut playback, mut runner, mut adapter, _audio_keep_alive) = runtime(&root);
        for pressed in [true, false] {
            dispatch(
                &mut playback,
                &mut runner,
                &mut adapter,
                crate::input::neokey_message(1, pressed).unwrap(),
            )
            .unwrap();
        }
        adapter.begin_autoaux_evidence();
        let tick_at = Instant::now();
        let started =
            tick_at - crate::autoaux_sequence::SAVE_COMPLETION_TIMEOUT - Duration::from_millis(10);
        let deadline = started + crate::autoaux_sequence::SAVE_COMPLETION_TIMEOUT;
        let accepted_at = if accepted_before_deadline {
            deadline - Duration::from_millis(1)
        } else {
            deadline + Duration::from_millis(1)
        };
        let mut autoaux = RaspberryAutoAux {
            sequence: AutoAuxSequence::new(started),
            starting_cutoff: 90,
        };
        autoaux.sequence.phase = Phase::AwaitSave { started };
        autoaux.sequence.final_revision = Some(41);
        adapter.set_autoaux_expected_revision(Some(41));
        let result = RuntimeStoreResult::Identified {
            result: Box::new(RuntimeStoreResult::SaveDefaultResult {
                ok: true,
                is_auto: Some(true),
            }),
            request_id: "buffered-save-41".into(),
            revision: Some(41),
        };
        adapter.observe_autoaux_store_result(
            store_autoaux_result_observation(&result).unwrap(),
            accepted_at,
        );
        let error = autoaux
            .tick(
                tick_at,
                &mut playback,
                &mut runner,
                &mut adapter,
                &NativeScenePump::new(tick_at),
            )
            .unwrap_err();
        if accepted_before_deadline {
            assert!(error.contains("two physical Cutoff frames"), "{error}");
            assert!(autoaux.sequence.save_completion.is_some());
        } else {
            assert!(error.contains("timed out"), "{error}");
            assert!(autoaux.sequence.save_completion.is_none());
        }
        let _ = std::fs::remove_dir_all(root);
    }
}

#[test]
fn result_observation_keeps_only_identified_automatic_default_saves() {
    let success = RuntimeStoreResult::Identified {
        result: Box::new(RuntimeStoreResult::SaveDefaultResult {
            ok: true,
            is_auto: Some(true),
        }),
        request_id: "save-1".into(),
        revision: Some(8),
    };
    assert!(store_autoaux_result_observation(&success).is_some());
    let manual = RuntimeStoreResult::Identified {
        result: Box::new(RuntimeStoreResult::SaveDefaultResult {
            ok: true,
            is_auto: None,
        }),
        request_id: "manual".into(),
        revision: Some(8),
    };
    assert!(store_autoaux_result_observation(&manual).is_none());
    assert!(
        store_autoaux_result_observation(&RuntimeStoreResult::SaveDefaultResult {
            ok: true,
            is_auto: Some(true),
        },)
        .is_none()
    );

    let save_failure = RuntimeStoreResult::Identified {
        result: Box::new(RuntimeStoreResult::RuntimeFailure {
            error: RuntimeErrorFacts::new(
                RuntimeErrorDomain::Storage,
                RuntimeErrorCode::OperationFailed,
                RuntimeOperation::StoreSaveDefault,
                Some("save failed".into()),
            ),
        }),
        request_id: "save-failed".into(),
        revision: Some(6),
    };
    assert!(store_autoaux_result_observation(&save_failure).is_some());

    let root = root();
    let (_playback, _runner, mut adapter, _audio_keep_alive) = runtime(&root);
    adapter.begin_autoaux_evidence();
    adapter.set_autoaux_expected_revision(Some(9));
    adapter.observe_autoaux_store_result(
        store_autoaux_result_observation(&success).unwrap(),
        Instant::now(),
    );
    assert!(adapter.take_autoaux_store_result().is_none());
    adapter.set_autoaux_expected_revision(Some(8));
    adapter.observe_autoaux_store_result(
        store_autoaux_result_observation(&success).unwrap(),
        Instant::now(),
    );
    assert!(adapter.take_autoaux_store_result().is_some());
    let _ = std::fs::remove_dir_all(root);

    if std::env::var_os(FAIL_CHILD_ENV).is_some() {
        fail_autoaux("child failure");
    }
    let output = Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg("raspberry_autoaux::tests::result_observation_keeps_only_identified_automatic_default_saves")
        .arg("--nocapture")
        .env(FAIL_CHILD_ENV, "1")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(stderr.matches("raspberry-autoaux-failed:").count(), 1);
    assert!(stderr.contains("raspberry-autoaux-failed: child failure"));
}

#[test]
fn result_observation_is_not_queued_when_opt_in_is_off() {
    let root = root();
    let (mut _playback, mut _runner, mut adapter, _audio_keep_alive) = runtime(&root);
    dispatch(
        &mut _playback,
        &mut _runner,
        &mut adapter,
        HostMessage::RuntimeResult {
            result: RuntimeStoreResult::Identified {
                result: Box::new(RuntimeStoreResult::SaveDefaultResult {
                    ok: true,
                    is_auto: Some(true),
                }),
                request_id: "save-off".into(),
                revision: Some(1),
            },
        },
    )
    .unwrap();
    assert!(adapter.take_autoaux_store_result().is_none());
    let _ = std::fs::remove_dir_all(root);
}
