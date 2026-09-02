use super::handoff::{startup_fatal_code, OrangeStartupOperation};
use super::{
    drain_host_results, encoder_id, prepare_runtime, qualified_encoder_ids,
    wait_for_initial_audio_prep, OrangeStartupReadinessGate,
};
use crate::audio::test_service_with_prep_sender;
use crate::audio_stream_health::{AudioStreamHealth, AudioStreamStatus};
use crate::candidate_readiness::CandidateReadiness;
use crate::hardware_runtime_scheduler::{MAINTENANCE_TICK, PLAYBACK_TICK, SNAPSHOT_TICK};
use crate::orange_host_adapter::OrangeHostAdapter;
use playback_runtime::{
    CoreRunner, HostAdapter, HostMessage, MusicalEvent, NativeRunner, NativeRunnerConfig,
    PlaybackRuntime, RunnerMessage, RuntimeAudioCommand, RuntimeConfig, RuntimeDispatchInput,
    RuntimeOperation, RuntimePlatformEffect, RuntimePlatformRequest, RuntimeStoreResult,
};
use std::path::PathBuf;
use std::sync::Arc;

fn test_host(audio: crate::audio::AudioService) -> (OrangeHostAdapter, PathBuf) {
    let root = std::env::temp_dir().join(format!(
        "octessera-orange-candidate-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let host = OrangeHostAdapter::with_directories(
        audio,
        root.join("store"),
        root.join("samples"),
        Arc::new(|_| {}),
        false,
    )
    .unwrap();
    (host, root)
}

#[test]
fn orange_candidate_uses_shared_runtime_cadence() {
    assert_eq!(PLAYBACK_TICK.as_millis(), 8);
    assert_eq!(SNAPSHOT_TICK.as_millis(), 33);
    assert_eq!(MAINTENANCE_TICK.as_millis(), 50);
}

#[test]
fn orange_startup_operations_map_to_typed_fatal_codes() {
    assert_eq!(
        startup_fatal_code(OrangeStartupOperation::Trellis),
        crate::boot_oled_handoff::StartupFatalCode::TrellisUnavailable
    );
    assert_eq!(
        startup_fatal_code(OrangeStartupOperation::Neokey),
        crate::boot_oled_handoff::StartupFatalCode::NeokeyUnavailable
    );
    assert_eq!(
        startup_fatal_code(OrangeStartupOperation::Controls),
        crate::boot_oled_handoff::StartupFatalCode::ControlsUnavailable
    );
    assert_eq!(
        startup_fatal_code(OrangeStartupOperation::Audio),
        crate::boot_oled_handoff::StartupFatalCode::AudioUnavailable
    );
    assert_eq!(
        startup_fatal_code(OrangeStartupOperation::Oled),
        crate::boot_oled_handoff::StartupFatalCode::OledUnavailable
    );
}

#[test]
fn orange_sample_root_creation_failure_does_not_publish_candidate_ready() {
    let root = std::env::temp_dir().join(format!(
        "octessera-orange-sample-root-failure-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let samples = root.join("samples");
    std::fs::write(&samples, b"not a directory").unwrap();
    let marker = root.join("candidate-ready.json");
    let _readiness = CandidateReadiness::new(Some(marker.clone()), "orange-root-failure".into());
    let (audio, _, _) = crate::audio::test_service();

    let error = match OrangeHostAdapter::with_directories(
        audio,
        root.join("store"),
        samples,
        Arc::new(|_| {}),
        false,
    ) {
        Ok(_) => panic!("sample root failure should abort Orange startup"),
        Err(error) => error,
    };

    assert!(error.contains("Orange samples directory is not usable"));
    assert!(!marker.exists());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn orange_startup_waits_for_the_identified_audio_prep_result() {
    let (audio, _, _, result_tx) = test_service_with_prep_sender();
    let (mut host, root) = test_host(audio);
    let mut playback = PlaybackRuntime::new(RuntimeConfig::default());
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(20));
        result_tx
            .send(HostMessage::RuntimeResult {
                result: RuntimeStoreResult::Identified {
                    result: Box::new(RuntimeStoreResult::OperationSucceeded {
                        operation: RuntimeOperation::AudioCommand,
                        request_id: None,
                        revision: Some(0),
                    }),
                    request_id: "audio-initial".into(),
                    revision: Some(0),
                },
            })
            .unwrap();
    });

    wait_for_initial_audio_prep(&mut playback, &mut runner, &mut host).unwrap();

    assert!(host.drain_results(1).is_empty());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn orange_prepare_runtime_preserves_audio_prep_for_startup_wait() {
    let (audio, control_rx, _event_rx, result_tx) = test_service_with_prep_sender();
    result_tx
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::OperationSucceeded {
                operation: RuntimeOperation::AudioCommand,
                request_id: None,
                revision: Some(0),
            },
        })
        .unwrap();
    let mut prepared = prepare_runtime(audio, Arc::new(|_| {}), false, true).unwrap();

    wait_for_initial_audio_prep(
        &mut prepared.playback,
        &mut prepared.runner,
        &mut prepared.host,
    )
    .unwrap();

    drop(control_rx);
}

#[test]
fn orange_startup_audio_is_not_starved_by_a_platform_backlog() {
    let (audio, control_rx, _event_rx, result_tx) = test_service_with_prep_sender();
    let (mut host, root) = test_host(audio);
    for index in 0..8 {
        assert!(host
            .handle_platform_effect(&RuntimePlatformRequest::new(
                RuntimePlatformEffect::StoreListPresets,
                format!("startup-backlog-{index}"),
                Some(1),
            ))
            .unwrap()
            .is_empty());
    }
    std::thread::sleep(std::time::Duration::from_millis(50));
    result_tx
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::OperationSucceeded {
                operation: RuntimeOperation::AudioCommand,
                request_id: None,
                revision: Some(0),
            },
        })
        .unwrap();
    let mut playback = PlaybackRuntime::new(RuntimeConfig::default());
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();

    wait_for_initial_audio_prep(&mut playback, &mut runner, &mut host).unwrap();

    drop(control_rx);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn orange_startup_accepts_the_native_runner_initial_audio_result_shape() {
    let (audio, control_rx, _event_rx, result_tx) = test_service_with_prep_sender();
    crate::host_audio_prep::spawn_audio_control_worker(control_rx, audio.clone(), result_tx);
    let (mut host, root) = test_host(audio);
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
    host.handle_audio_command(&command).unwrap();

    wait_for_initial_audio_prep(&mut playback, &mut runner, &mut host).unwrap();

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn legacy_orange_preparation_starts_with_a_canonical_normal_snapshot() {
    let (audio, _, _) = crate::audio::test_service();
    let prepared = prepare_runtime(audio, Arc::new(|_| {}), false, true).unwrap();
    let snapshot = prepared.playback.last_snapshot().unwrap();

    assert!(prepared.runner.is_canonical_menu_presentation());
    assert_eq!(snapshot["display"]["off"], false);
    assert_eq!(snapshot["display"]["splash"], "");
    assert!(snapshot["display"]["title"]
        .as_str()
        .is_some_and(|title| !title.is_empty()));
    assert!(snapshot["display"]["lines"]
        .as_array()
        .is_some_and(|lines| !lines.is_empty()));
    assert!(prepared.playback.last_snapshot_revision() > 0);
}

#[test]
fn legacy_orange_readiness_gate_requires_ack_and_healthy_dac() {
    let path = std::env::temp_dir().join(format!(
        "octessera-orange-readiness-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut readiness = CandidateReadiness::new(Some(path.clone()), "orange-test".into());
    let mut gate = OrangeStartupReadinessGate::new(false);
    assert!(gate
        .acknowledge_initial_write(Err("OLED write failed".into()))
        .is_err());
    assert!(!path.exists());

    gate.acknowledge_initial_write(Ok(())).unwrap();
    gate.try_mark_ready(
        crate::audio_stream_health::AudioStreamStatus::Recovering,
        &mut readiness,
    )
    .unwrap();
    assert!(!path.exists());
    gate.try_mark_ready(
        crate::audio_stream_health::AudioStreamStatus::Healthy,
        &mut readiness,
    )
    .unwrap();
    assert!(!path.exists());
    gate.acknowledge_initial_audio_prep(Ok(())).unwrap();
    gate.try_mark_ready(
        crate::audio_stream_health::AudioStreamStatus::Healthy,
        &mut readiness,
    )
    .unwrap();
    assert!(path.is_file());
    drop(readiness);
    let _ = std::fs::remove_dir_all(path);
}

#[test]
fn qualified_encoder_ids_preserve_main_and_aux_semantics() {
    assert_eq!(encoder_id(0), Ok("encoder_main"));
    assert_eq!(encoder_id(1), Ok("encoder_aux_1"));
    assert_eq!(encoder_id(2), Ok("encoder_aux_2"));
    assert_eq!(encoder_id(3), Ok("encoder_aux_3"));
    assert!(encoder_id(4).is_err());
}

#[test]
fn orange_candidate_composes_all_encoders_after_uart0_is_disabled() {
    assert_eq!(
        qualified_encoder_ids(),
        [
            "encoder_main",
            "encoder_aux_1",
            "encoder_aux_2",
            "encoder_aux_3"
        ]
    );
}

#[test]
fn orange_audio_prep_results_are_redispatched_to_runtime() {
    let (audio, _, _, result_tx) = test_service_with_prep_sender();
    result_tx
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::OperationSucceeded {
                operation: RuntimeOperation::AudioCommand,
                request_id: None,
                revision: Some(1),
            },
        })
        .unwrap();
    let mut playback = PlaybackRuntime::new(RuntimeConfig::default());
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let (mut host, root) = test_host(audio.clone());

    drain_host_results(&mut playback, &mut runner, &mut host).unwrap();

    assert!(audio.drain_prep_results(1).is_empty());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn orange_runner_midi_events_do_not_latch_midi_error_when_disabled() {
    let (audio, _, _) = crate::audio::test_service();
    let (mut host, root) = test_host(audio);
    let mut playback = PlaybackRuntime::new(RuntimeConfig {
        midi_out_enabled: true,
        ..RuntimeConfig::default()
    });
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();

    playback
        .dispatch(
            RuntimeDispatchInput::RunnerMessages(vec![RunnerMessage::MidiEvents {
                events: vec![MusicalEvent::NoteOn {
                    channel: 0,
                    note: 60,
                    velocity: 100,
                    duration_ms: Some(25),
                }],
            }]),
            &mut runner,
            &mut host,
        )
        .unwrap();

    assert!(playback.latched_errors().is_empty());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn required_jack_fault_terminates_orange_runtime() {
    let error = super::ensure_required_audio_health(
        crate::audio_stream_health::AudioStreamStatus::Terminal,
    )
    .unwrap_err();

    assert_eq!(error, "Orange Jack audio stream faulted");
}

#[test]
fn worker_terminal_becomes_runtime_terminal_and_fails_readiness() {
    let health = AudioStreamHealth::new("Jack".into());
    health.mark_worker_terminal();

    assert_eq!(health.external_status(), AudioStreamStatus::Healthy);
    assert_eq!(health.runtime_status(), AudioStreamStatus::Terminal);
    assert!(super::ensure_required_audio_health(health.runtime_status()).is_err());
}

#[test]
fn recoverable_jack_fault_keeps_runtime_alive_for_recovery() {
    assert!(super::ensure_required_audio_health(
        crate::audio_stream_health::AudioStreamStatus::Recovering,
    )
    .is_ok());
}
