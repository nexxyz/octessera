use super::{
    drain_host_results, encoder_id, encoder_message, qualified_encoder_ids, POLLING_INTERVAL,
    RENDER_INTERVAL, RUNTIME_TICK,
};
use crate::audio::test_service_with_prep_sender;
use crate::orange_host_adapter::OrangeHostAdapter;
use octessera_hal::encoder_gpio::HardwareEvent;
use playback_runtime::{
    HostMessage, MusicalEvent, NativeRunner, NativeRunnerConfig, PlaybackRuntime, RunnerMessage,
    RuntimeConfig, RuntimeDispatchInput, RuntimeOperation, RuntimeStoreResult,
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
fn orange_candidate_uses_ten_millisecond_polling() {
    assert_eq!(POLLING_INTERVAL.as_millis(), 10);
    assert!(RUNTIME_TICK <= POLLING_INTERVAL);
    assert!(RENDER_INTERVAL > POLLING_INTERVAL);
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
fn orange_encoder_events_use_native_input_messages() {
    let HostMessage::DeviceInput { input, .. } = encoder_message(HardwareEvent::EncoderTurn {
        id: "encoder_main",
        delta: 1,
    })
    .unwrap() else {
        panic!("expected main encoder turn input");
    };
    assert_eq!(input["type"], "encoder_turn");
    assert_eq!(input["id"], "main");

    let HostMessage::DeviceInput { input, .. } = encoder_message(HardwareEvent::EncoderPress {
        id: "encoder_aux_2",
    })
    .unwrap() else {
        panic!("expected aux encoder press input");
    };
    assert_eq!(input["type"], "encoder_press");
    assert_eq!(input["id"], "aux2");
    assert!(encoder_message(HardwareEvent::EncoderRelease { id: "encoder_main" }).is_none());
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
fn required_internal_dac_fault_terminates_orange_runtime() {
    let error = super::ensure_required_audio_health(
        crate::audio_stream_health::AudioStreamStatus::Terminal,
    )
    .unwrap_err();

    assert_eq!(error, "Orange internal DAC audio stream faulted");
}

#[test]
fn recoverable_internal_dac_fault_keeps_runtime_alive_for_recovery() {
    assert!(super::ensure_required_audio_health(
        crate::audio_stream_health::AudioStreamStatus::Recovering,
    )
    .is_ok());
}
