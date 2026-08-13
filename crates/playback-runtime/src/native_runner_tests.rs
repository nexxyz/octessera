use super::support::FakeHost;
use crate::{
    CoreRunner, HostMessage, NativeRunner, NativeRunnerConfig, PlaybackRuntime, RunnerMessage,
    RuntimeConfig, RuntimeErrorCode, RuntimeErrorDomain, RuntimeErrorFacts, RuntimeOperation,
    RuntimeStoreResult, SyncSource,
};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

fn snapshot_count(messages: &[RunnerMessage]) -> usize {
    messages
        .iter()
        .filter(|message| matches!(message, RunnerMessage::Snapshot { .. }))
        .count()
}

fn snapshot_value(messages: &[RunnerMessage]) -> &Value {
    messages
        .iter()
        .find_map(|message| match message {
            RunnerMessage::Snapshot { snapshot } => Some(snapshot),
            _ => None,
        })
        .expect("runner output should contain a snapshot")
}

#[test]
fn native_runner_rejects_unsupported_behavior() {
    let error = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "unsupported".into(),
        ..NativeRunnerConfig::default()
    })
    .err()
    .unwrap();
    assert!(error.contains("unsupported native behavior `unsupported`"));
}

#[test]
fn native_runner_transport_tick_returns_status_and_snapshot() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let _ = runner.send(HostMessage::DeviceInput {
        input: json!({ "type": "button_s", "pressed": true }),
        request_snapshot: None,
    });
    let messages = runner
        .send(HostMessage::TransportPulseStep {
            pulses: 24,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(true),
        })
        .unwrap();
    assert!(matches!(
        messages.last(),
        Some(RunnerMessage::RuntimeStatus { .. })
    ));
    assert!(messages
        .iter()
        .any(|message| matches!(message, RunnerMessage::Snapshot { .. })));
}

#[test]
fn request_snapshot_false_device_event_has_ordered_snapshot_and_expiry() {
    let start = Instant::now();
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "keys".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.test_set_display_time(start);
    runner.send(HostMessage::MidiRealtimeStart).unwrap();
    runner.test_set_display_time(start + Duration::from_millis(91));

    let event = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 2, "y": 3 }),
            request_snapshot: Some(false),
        })
        .unwrap();
    let event_index = event
        .iter()
        .position(|message| matches!(message, RunnerMessage::MusicalEvents { .. }))
        .expect("device input should produce a musical event");
    let snapshot_index = event
        .iter()
        .position(|message| matches!(message, RunnerMessage::Snapshot { .. }))
        .expect("device input event should produce a snapshot");
    let status_index = event
        .iter()
        .position(|message| matches!(message, RunnerMessage::RuntimeStatus { .. }))
        .expect("device input event should produce status");
    assert_eq!(snapshot_count(&event), 1);
    assert!(event_index < snapshot_index);
    assert!(snapshot_index < status_index);
    assert_eq!(snapshot_value(&event)["eventDotOn"], true);

    runner.test_set_display_time(start + Duration::from_millis(100));
    let combined = runner
        .send(HostMessage::TransportPulseStep {
            pulses: 24,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();
    assert_eq!(snapshot_count(&combined), 1);
    assert_eq!(snapshot_value(&combined)["eventDotOn"], true);
    assert_eq!(snapshot_value(&combined)["transportFlash"], "beat");

    runner.test_set_display_time(start + Duration::from_millis(136));
    let expiry = runner
        .send(HostMessage::TransportPulseStep {
            pulses: 0,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();
    assert_eq!(snapshot_count(&expiry), 1);
    assert_eq!(snapshot_value(&expiry)["eventDotOn"], false);
}

#[test]
fn native_runner_publishes_runtime_config_changes_once() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let initial = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "other" }),
            request_snapshot: None,
        })
        .unwrap();
    assert!(initial.iter().any(|message| matches!(
        message,
        RunnerMessage::RuntimeConfigChanged {
            config: RuntimeConfig {
                bpm: 120.0,
                sync_source: SyncSource::Internal,
                midi_clock_out_enabled: false,
                midi_out_enabled: false,
            }
        }
    )));

    let unchanged = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "other" }),
            request_snapshot: None,
        })
        .unwrap();
    assert!(!unchanged
        .iter()
        .any(|message| matches!(message, RunnerMessage::RuntimeConfigChanged { .. })));

    runner
        .apply_config_payload(json!({
            "runtimeConfig": {
                "transport": { "bpm": 93.5 },
                "midi": {
                    "enabled": true,
                    "outId": "out-1",
                    "syncMode": "external",
                    "clockOutEnabled": true
                }
            }
        }))
        .unwrap();
    let changed = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "other" }),
            request_snapshot: None,
        })
        .unwrap();
    assert!(changed.iter().any(|message| matches!(
        message,
        RunnerMessage::RuntimeConfigChanged {
            config: RuntimeConfig {
                bpm,
                sync_source: SyncSource::External,
                midi_clock_out_enabled: true,
                midi_out_enabled: true,
            }
        } if (*bpm - 93.5).abs() < f64::EPSILON
    )));
}

#[test]
fn midi_input_failure_uses_concise_native_oled_presentation() {
    let raw_detail = "ALSA lib seq_hw.c:466:(snd_seq_hw_open) open /dev/snd/seq failed: No such file or directory";
    let result = RuntimeStoreResult::RuntimeFailure {
        error: RuntimeErrorFacts::new(
            RuntimeErrorDomain::Midi,
            RuntimeErrorCode::OperationFailed,
            RuntimeOperation::MidiListInputs,
            Some(raw_detail.into()),
        ),
    }
    .with_identity("midi-inputs-1".into(), Some(7));
    let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut host = FakeHost::default();

    runtime
        .dispatch_host_message(
            HostMessage::RuntimeResult { result },
            &mut runner,
            &mut host,
        )
        .unwrap();

    let snapshot = runtime.last_snapshot().unwrap();
    let display = snapshot["display"].as_object().unwrap();
    assert_eq!(display["title"], "MIDI INPUTS");
    assert_eq!(display["lines"], json!(["MIDI unavailable"]));
    assert_eq!(display["toast"], "MIDI unavailable");
    for line in std::iter::once(display["title"].as_str().unwrap())
        .chain(
            display["lines"]
                .as_array()
                .unwrap()
                .iter()
                .map(|line| line.as_str().unwrap()),
        )
        .chain(std::iter::once(display["toast"].as_str().unwrap()))
    {
        assert!(line.chars().count() <= 20, "OLED line is too wide: {line}");
        assert!(!line.contains(raw_detail));
    }
    let visible_composition = format!(
        "{} {} {}",
        display["title"], display["lines"][0], display["toast"]
    );
    assert!(!visible_composition.contains("MIDI OPERATION FAILED MIDI LIST INPUTS"));
    let typed_error = runtime.last_status().unwrap().error.as_ref().unwrap();
    assert_eq!(typed_error.message.as_deref(), Some(raw_detail));
    assert_eq!(typed_error.request_id.as_deref(), Some("midi-inputs-1"));
    assert_eq!(typed_error.revision, Some(7));
    assert_eq!(snapshot["runtimeError"]["domain"], "midi");
    assert_eq!(snapshot["runtimeError"]["code"], "operation_failed");
    assert_eq!(snapshot["runtimeError"]["operation"], "midi_list_inputs");
    assert_eq!(snapshot["runtimeError"]["message"], raw_detail);
    assert_eq!(snapshot["runtimeError"]["requestId"], "midi-inputs-1");
    assert_eq!(snapshot["runtimeError"]["revision"], 7);
}
