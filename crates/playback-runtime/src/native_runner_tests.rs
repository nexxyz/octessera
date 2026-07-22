use crate::{CoreRunner, HostMessage, NativeRunner, NativeRunnerConfig, RunnerMessage, SyncSource};
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
