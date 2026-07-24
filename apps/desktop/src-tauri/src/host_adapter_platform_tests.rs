use super::{platform_request, test_adapter};
use crate::types::QueuedAudioEvent;
use playback_runtime::{HostAdapter, HostMessage, RuntimePlatformEffect, RuntimeStoreResult};
use std::time::Duration;

#[test]
fn sample_list_request_reports_service_unavailable_when_enqueue_fails() {
    let (mut adapter, _) = test_adapter();
    let follow_ups = adapter
        .handle_platform_effect(&platform_request(
            RuntimePlatformEffect::SampleListRequest {
                instrument_slot: 1,
                sample_slot: 2,
                dir: "kits".into(),
            },
        ))
        .unwrap();

    assert!(
        matches!(&follow_ups[..], [HostMessage::RuntimeResult { result: RuntimeStoreResult::Identified { result, .. } }] if matches!(result.as_ref(), RuntimeStoreResult::SampleListError { instrument_slot: 1, sample_slot: 2, dir, message } if dir == "kits" && message == "Desktop platform service unavailable"))
    );
}

#[test]
fn system_info_request_reports_typed_service_unavailable() {
    let (mut adapter, _) = test_adapter();
    let follow_ups = adapter
        .handle_platform_effect(&platform_request(RuntimePlatformEffect::SystemInfoRequest))
        .unwrap();
    assert!(matches!(
        &follow_ups[..],
        [HostMessage::RuntimeResult {
            result: RuntimeStoreResult::Identified { result, .. }
        }] if matches!(result.as_ref(), RuntimeStoreResult::SystemInfoError { error } if error.code == playback_runtime::RuntimeErrorCode::Unavailable)
    ));
}

#[test]
fn midi_panic_returns_native_status_result() {
    let (mut adapter, rx) = test_adapter();
    let follow_ups = adapter
        .handle_platform_effect(&platform_request(RuntimePlatformEffect::MidiPanic))
        .unwrap();
    assert_eq!(
        follow_ups,
        vec![HostMessage::RuntimeResult {
            result: RuntimeStoreResult::MidiStatus {
                ok: true,
                message: Some("Panic sent".into()),
                selected_out_id: None,
                selected_in_id: None
            }
        }]
    );
    assert!(matches!(
        rx.recv_timeout(Duration::from_secs(1)).unwrap(),
        QueuedAudioEvent::AllNotesOff
    ));
}

#[test]
fn desktop_update_apply_returns_typed_unsupported() {
    let (mut adapter, _) = test_adapter();
    let follow_ups = adapter
        .handle_platform_effect(&platform_request(RuntimePlatformEffect::UpdateApply))
        .unwrap();

    assert!(matches!(
        &follow_ups[..],
        [HostMessage::RuntimeResult {
            result: RuntimeStoreResult::RuntimeFailure { error }
        }] if error.code == playback_runtime::RuntimeErrorCode::Unsupported
    ));
}

#[test]
fn shutdown_effect_sets_pending_shutdown_request() {
    let (mut adapter, _) = test_adapter();
    let follow_ups = adapter
        .handle_platform_effect(&platform_request(RuntimePlatformEffect::Shutdown))
        .unwrap();
    assert!(follow_ups.is_empty());
    assert!(adapter.take_shutdown_request());
    assert!(!adapter.take_shutdown_request());
}
