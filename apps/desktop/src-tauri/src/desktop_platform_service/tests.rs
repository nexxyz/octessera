use super::*;
use std::sync::mpsc;

fn only_result(messages: Vec<HostMessage>) -> RuntimeStoreResult {
    assert_eq!(messages.len(), 1);
    match messages.into_iter().next().unwrap() {
        HostMessage::RuntimeResult { result } => match result {
            RuntimeStoreResult::Identified { result, .. } => *result,
            result => result,
        },
        _ => panic!("expected one runtime result"),
    }
}

#[test]
fn sample_list_error_shapes_runtime_error() {
    let result = only_result(shape_sample_list_result(1, 2, "bad".into(), |_| {
        Err("nope".into())
    }));
    assert!(
        matches!(result, RuntimeStoreResult::SampleListError { instrument_slot: 1, sample_slot: 2, dir, message } if dir == "bad" && message == "nope")
    );
}

#[test]
fn midi_output_error_returns_only_store_error() {
    let result = only_result(shape_midi_outputs_result(|| Err("midi unavailable".into())));
    assert!(
        matches!(result, RuntimeStoreResult::StoreError { message } if message == "midi unavailable")
    );
}

#[test]
fn midi_input_error_returns_only_store_error() {
    let result = only_result(shape_midi_inputs_result(|| Err("midi unavailable".into())));
    assert!(
        matches!(result, RuntimeStoreResult::StoreError { message } if message == "midi unavailable")
    );
}

#[test]
fn midi_empty_lists_remain_successful_results() {
    let outputs = only_result(shape_midi_outputs_result(|| Ok(Vec::new())));
    assert!(
        matches!(outputs, RuntimeStoreResult::MidiListOutputsResult { outputs } if outputs.is_empty())
    );
    let inputs = only_result(shape_midi_inputs_result(|| Ok(Vec::new())));
    assert!(
        matches!(inputs, RuntimeStoreResult::MidiListInputsResult { inputs } if inputs.is_empty())
    );
}

#[test]
fn service_unavailable_midi_requests_return_only_store_error() {
    let outputs = only_result(shape_service_unavailable_result(
        DesktopPlatformServiceRequest::new(
            RuntimePlatformRequest::new(
                playback_runtime::RuntimePlatformEffect::MidiListOutputsRequest,
                "test-output".into(),
                None,
            ),
            DesktopPlatformServiceKind::MidiListOutputs,
        ),
        "service down".into(),
    ));
    assert!(
        matches!(outputs, RuntimeStoreResult::RuntimeFailure { error } if error.message.as_deref() == Some("service down"))
    );
    let inputs = only_result(shape_service_unavailable_result(
        DesktopPlatformServiceRequest::new(
            RuntimePlatformRequest::new(
                playback_runtime::RuntimePlatformEffect::MidiListInputsRequest,
                "test-input".into(),
                None,
            ),
            DesktopPlatformServiceKind::MidiListInputs,
        ),
        "service down".into(),
    ));
    assert!(
        matches!(inputs, RuntimeStoreResult::RuntimeFailure { error } if error.message.as_deref() == Some("service down"))
    );
}

#[test]
fn service_unavailable_shapes_sample_list_error() {
    let result = only_result(shape_service_unavailable_result(
        DesktopPlatformServiceRequest::new(
            RuntimePlatformRequest::new(
                playback_runtime::RuntimePlatformEffect::SampleListRequest {
                    instrument_slot: 2,
                    sample_slot: 3,
                    dir: "kits".into(),
                },
                "test-sample".into(),
                None,
            ),
            DesktopPlatformServiceKind::SampleList {
                instrument_slot: 2,
                sample_slot: 3,
                dir: "kits".into(),
            },
        ),
        "service down".into(),
    ));
    assert!(
        matches!(result, RuntimeStoreResult::SampleListError { instrument_slot: 2, sample_slot: 3, dir, message } if dir == "kits" && message == "service down")
    );
}

#[test]
fn system_info_service_sanitizes_successful_adapter_data() {
    let messages = shape_system_info_result(|| {
        Ok(RuntimeSystemInfo {
            os: "Linux\nnoise".into(),
            os_version: "6.6".into(),
            octessera_version: "0.7.0".into(),
            primary_ip: None,
            primary_mac: None,
            hostname: "octessera".into(),
            board_profile: "desktop".into(),
        })
    });
    let result = only_result(messages);
    assert!(
        matches!(result, RuntimeStoreResult::SystemInfoResult { info } if info.os == "Linuxnoise" && info.board_profile == "desktop")
    );
}

#[test]
fn system_info_service_shapes_typed_unavailable_error() {
    let result = only_result(shape_system_info_result(|| Err("service down".into())));
    assert!(
        matches!(result, RuntimeStoreResult::SystemInfoError { error } if error.code == RuntimeErrorCode::Unavailable && error.message == "service down")
    );
}

#[test]
fn system_info_service_result_keeps_request_identity() {
    let messages = shape_service_result(DesktopPlatformServiceRequest::new(
        RuntimePlatformRequest::new(
            playback_runtime::RuntimePlatformEffect::SystemInfoRequest,
            "system-info-test".into(),
            Some(4),
        ),
        DesktopPlatformServiceKind::SystemInfo,
    ));
    assert!(matches!(messages.as_slice(), [HostMessage::RuntimeResult {
        result: RuntimeStoreResult::Identified { request_id, revision, result }
    }] if request_id == "system-info-test" && *revision == Some(4) && matches!(result.as_ref(), RuntimeStoreResult::SystemInfoResult { .. })));
}

#[test]
fn system_save_worker_writes_before_matching_completion() {
    let root = std::env::temp_dir().join(format!(
        "octessera-system-save-worker-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("system.json");
    std::fs::write(&path, b"old bytes").unwrap();
    let service = spawn_desktop_platform_service();
    let (started_tx, started_rx) = mpsc::sync_channel(1);
    let release = std::sync::Arc::new((std::sync::Mutex::new(false), std::sync::Condvar::new()));
    let gate_request = DesktopPlatformServiceRequest::new(
        RuntimePlatformRequest::new(
            playback_runtime::RuntimePlatformEffect::SystemInfoRequest,
            "gate".into(),
            None,
        ),
        DesktopPlatformServiceKind::WorkerGate {
            started: started_tx,
            release: release.clone(),
        },
    );
    admit_platform_service_request(&service.request_tx, gate_request).unwrap();
    started_rx.recv().unwrap();
    let payload = serde_json::json!({ "deviceName": "Octessera" });
    admit_platform_service_request(
        &service.request_tx,
        DesktopPlatformServiceRequest::new(
            RuntimePlatformRequest::new(
                playback_runtime::RuntimePlatformEffect::StoreSaveSystem {
                    payload: payload.clone(),
                },
                "system-save".into(),
                Some(7),
            ),
            DesktopPlatformServiceKind::SaveSystem {
                store_dir: root.clone(),
                payload: payload.clone(),
            },
        ),
    )
    .unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"old bytes");
    let (lock, condition) = &*release;
    *lock.lock().unwrap() = true;
    condition.notify_one();
    assert!(service.result_rx.recv().unwrap().is_empty());
    let messages = service.result_rx.recv().unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&std::fs::read(&path).unwrap()).unwrap(),
        payload
    );
    assert!(
        matches!(messages.as_slice(), [HostMessage::RuntimeResult {
            result: RuntimeStoreResult::Identified { request_id, revision: Some(7), result }
        }] if request_id == "system-save" && matches!(result.as_ref(), RuntimeStoreResult::SaveSystemResult { ok: true })),
        "unexpected save result: {messages:?}"
    );
    assert!(service.result_rx.try_recv().is_err());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn failed_system_save_is_identified_storage_runtime_failure() {
    let root = std::env::temp_dir().join(format!(
        "octessera-system-save-failure-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("system.json");
    std::fs::create_dir(&path).unwrap();
    let request = DesktopPlatformServiceRequest::new(
        RuntimePlatformRequest::new(
            playback_runtime::RuntimePlatformEffect::StoreSaveSystem {
                payload: serde_json::json!({ "new": true }),
            },
            "failed-save".into(),
            Some(3),
        ),
        DesktopPlatformServiceKind::SaveSystem {
            store_dir: root.clone(),
            payload: serde_json::json!({ "new": true }),
        },
    );
    let result = only_result(shape_service_result(request));
    assert!(
        matches!(result, RuntimeStoreResult::RuntimeFailure { ref error } if error.domain == RuntimeErrorDomain::Storage && error.operation == RuntimeOperation::StoreSaveSystem),
        "unexpected failed save result: {result:?}"
    );
    let _ = std::fs::remove_dir_all(root);
}
