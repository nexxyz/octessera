use super::support::{FakeHost, FakeRunner};
use crate::{
    HostMessage, PlaybackRuntime, RunnerMessage, RuntimeConfig, RuntimeSetupPortalErrorCode,
    RuntimeSetupPortalPhase, RuntimeSetupPortalStatus, RuntimeStatus, RuntimeStatusState,
    RuntimeStoreResult, RuntimeTransportState, SyncSource,
};
use serde_json::json;

#[test]
fn typed_setup_portal_failure_keeps_the_native_lifecycle_presentation() {
    let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
    let mut runner = FakeRunner::default();
    let mut host = FakeHost::default();
    runtime
        .ingest_runner_messages(
            vec![
                RunnerMessage::Snapshot {
                    snapshot: json!({"display": {"title": "Wi-Fi Setup"}}),
                },
                RunnerMessage::RuntimeStatus {
                    status: RuntimeStatus {
                        state: RuntimeStatusState::Running,
                        transport: RuntimeTransportState::Stopped,
                        current_ppqn_pulse: 0,
                        pending_resync: false,
                        sync_source: SyncSource::Internal,
                        message: None,
                        error: None,
                    },
                },
            ],
            &mut host,
        )
        .unwrap();

    let result = RuntimeStoreResult::SetupPortalStatus {
        status: RuntimeSetupPortalStatus {
            phase: RuntimeSetupPortalPhase::Failed,
            disposition: None,
            portal_suffix: None,
            reboot_required: false,
            error_code: Some(RuntimeSetupPortalErrorCode::OperationFailed),
        },
    }
    .with_identity("setup-1".into(), Some(1));
    let output = runtime
        .dispatch_host_message(
            HostMessage::RuntimeResult { result },
            &mut runner,
            &mut host,
        )
        .unwrap();
    let snapshot = output.messages.iter().find_map(|message| match message {
        RunnerMessage::Snapshot { snapshot } => Some(snapshot),
        _ => None,
    });
    assert!(snapshot.is_some_and(|snapshot| snapshot.get("runtimeError").is_none()));
    assert!(output.messages.iter().any(|message| matches!(
        message,
        RunnerMessage::RuntimeStatus { status } if status.error.is_none()
    )));
}
