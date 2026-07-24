use crate::{
    CoreRunner, HostMessage, NativeRunner, NativeRunnerConfig, RunnerMessage, RuntimeConfig,
    SyncSource,
};
use serde_json::json;

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
