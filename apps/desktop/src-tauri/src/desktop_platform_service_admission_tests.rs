use super::*;
use playback_runtime::{
    HostMessage, RuntimePlatformEffect, RuntimePlatformRequest, RuntimeStoreResult,
};
use std::sync::mpsc;

fn system_info_request(index: usize) -> DesktopPlatformServiceRequest {
    DesktopPlatformServiceRequest::new(
        RuntimePlatformRequest::new(
            RuntimePlatformEffect::SystemInfoRequest,
            format!("request-{index}"),
            Some(index as u64),
        ),
        DesktopPlatformServiceKind::SystemInfo,
    )
}

fn identified_result(messages: Vec<HostMessage>) -> (String, Option<u64>, RuntimeStoreResult) {
    let [HostMessage::RuntimeResult {
        result:
            RuntimeStoreResult::Identified {
                request_id,
                revision,
                result,
            },
    }] = messages.as_slice()
    else {
        panic!("expected one identified runtime result");
    };
    (request_id.clone(), *revision, (**result).clone())
}

#[test]
fn platform_request_admission_accepts_exactly_32_requests() {
    let (sender, _receiver) = mpsc::sync_channel(DESKTOP_PLATFORM_REQUEST_QUEUE_CAPACITY);

    for index in 0..DESKTOP_PLATFORM_REQUEST_QUEUE_CAPACITY {
        assert!(admit_platform_service_request(&sender, system_info_request(index)).is_ok());
    }
}

#[test]
fn full_platform_request_preserves_identity_and_returns_typed_failure() {
    let (sender, _receiver) = mpsc::sync_channel(DESKTOP_PLATFORM_REQUEST_QUEUE_CAPACITY);
    for index in 0..DESKTOP_PLATFORM_REQUEST_QUEUE_CAPACITY {
        assert!(admit_platform_service_request(&sender, system_info_request(index)).is_ok());
    }

    let (request_id, revision, result) = identified_result(
        admit_platform_service_request(&sender, system_info_request(32)).unwrap_err(),
    );
    assert_eq!(request_id, "request-32");
    assert_eq!(revision, Some(32));
    assert!(matches!(
        result,
        RuntimeStoreResult::SystemInfoError { error }
            if error.code == playback_runtime::RuntimeErrorCode::Unavailable
                && error.message == "Desktop platform service queue is full."
    ));
}

#[test]
fn platform_request_admission_preserves_fifo_after_drain() {
    let (sender, receiver) = mpsc::sync_channel(DESKTOP_PLATFORM_REQUEST_QUEUE_CAPACITY);
    for index in 0..DESKTOP_PLATFORM_REQUEST_QUEUE_CAPACITY {
        assert!(admit_platform_service_request(&sender, system_info_request(index)).is_ok());
    }

    let requests = receiver.try_iter().collect::<Vec<_>>();
    let request_ids = requests
        .into_iter()
        .map(|request| request.request.request_id)
        .collect::<Vec<_>>();
    assert_eq!(
        request_ids,
        (0..DESKTOP_PLATFORM_REQUEST_QUEUE_CAPACITY)
            .map(|index| format!("request-{index}"))
            .collect::<Vec<_>>()
    );
}

#[test]
fn platform_request_admission_recovers_after_one_drain() {
    let (sender, receiver) = mpsc::sync_channel(DESKTOP_PLATFORM_REQUEST_QUEUE_CAPACITY);
    for index in 0..DESKTOP_PLATFORM_REQUEST_QUEUE_CAPACITY {
        assert!(admit_platform_service_request(&sender, system_info_request(index)).is_ok());
    }
    assert!(admit_platform_service_request(&sender, system_info_request(32)).is_err());

    assert_eq!(receiver.try_recv().unwrap().request.request_id, "request-0");
    assert!(admit_platform_service_request(&sender, system_info_request(32)).is_ok());
}

#[test]
fn disconnected_platform_request_preserves_identity_and_unavailable_semantics() {
    let (sender, receiver) = mpsc::sync_channel(DESKTOP_PLATFORM_REQUEST_QUEUE_CAPACITY);
    drop(receiver);

    let (request_id, revision, result) = identified_result(
        admit_platform_service_request(&sender, system_info_request(7)).unwrap_err(),
    );
    assert_eq!(request_id, "request-7");
    assert_eq!(revision, Some(7));
    assert!(matches!(
        result,
        RuntimeStoreResult::SystemInfoError { error }
            if error.code == playback_runtime::RuntimeErrorCode::Unavailable
                && error.message == "Desktop platform service unavailable"
    ));
}

#[test]
fn full_platform_request_shapes_each_service_kind() {
    let cases = [
        (
            RuntimePlatformEffect::SampleListRequest {
                instrument_slot: 1,
                sample_slot: 2,
                dir: "kits".into(),
            },
            DesktopPlatformServiceKind::SampleList {
                instrument_slot: 1,
                sample_slot: 2,
                dir: "kits".into(),
            },
        ),
        (
            RuntimePlatformEffect::MidiListInputsRequest,
            DesktopPlatformServiceKind::MidiListInputs,
        ),
        (
            RuntimePlatformEffect::MidiListOutputsRequest,
            DesktopPlatformServiceKind::MidiListOutputs,
        ),
        (
            RuntimePlatformEffect::SystemInfoRequest,
            DesktopPlatformServiceKind::SystemInfo,
        ),
    ];

    for (index, (effect, kind)) in cases.into_iter().enumerate() {
        let (sender, _receiver) = mpsc::sync_channel(DESKTOP_PLATFORM_REQUEST_QUEUE_CAPACITY);
        let request = DesktopPlatformServiceRequest::new(
            RuntimePlatformRequest::new(effect, format!("shape-{index}"), Some(40 + index as u64)),
            kind,
        );
        for filler in 0..DESKTOP_PLATFORM_REQUEST_QUEUE_CAPACITY {
            assert!(admit_platform_service_request(&sender, system_info_request(filler)).is_ok());
        }

        let (request_id, revision, result) =
            identified_result(admit_platform_service_request(&sender, request).unwrap_err());
        assert_eq!(request_id, format!("shape-{index}"));
        assert_eq!(revision, Some(40 + index as u64));
        match result {
            RuntimeStoreResult::SampleListError { message, .. } => {
                assert_eq!(message, "Desktop platform service queue is full.")
            }
            RuntimeStoreResult::RuntimeFailure { error } => assert_eq!(
                error.message.as_deref(),
                Some("Desktop platform service queue is full.")
            ),
            RuntimeStoreResult::SystemInfoError { error } => {
                assert_eq!(error.message, "Desktop platform service queue is full.")
            }
            result => panic!("unexpected full-queue result: {result:?}"),
        }
    }
}
