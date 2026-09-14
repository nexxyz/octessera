use super::*;
use std::sync::{Arc, Mutex};

fn role_payload(role: UsbDataRole) -> serde_json::Value {
    serde_json::json!({
        "runtimeConfig": {
            "audioOutputs": { "dac": true, "usb": false, "hdmi": false },
            "usb": { "dataRole": role.as_str(), "midiOutEnabled": false }
        }
    })
}

fn transaction_adapter(
    root: &std::path::Path,
    apply_role: impl Fn(UsbDataRole) -> Result<(), String> + Send + Sync + 'static,
) -> PiPlaybackHostAdapter {
    let service = PiPlatformService::new_with_role_applier(
        root.join("store"),
        root.join("samples"),
        Arc::new(apply_role),
    );
    PiPlaybackHostAdapter::with_platform_service_and_role(
        None,
        root.join("samples"),
        Arc::new(|_| {}),
        false,
        AudioOutputSet::jack(),
        service,
        UsbDataRole::Gadget,
    )
}

#[test]
fn raspberry_host_role_rejects_sd2_before_audio_or_midi_actions() {
    let root = std::env::temp_dir().join(format!(
        "octessera-pi-host-role-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut adapter = PiPlaybackHostAdapter::new_with_data_role(
        None,
        root.join("store"),
        root.join("samples"),
        Arc::new(|_| {}),
        true,
        AudioOutputSet::jack(),
        UsbDataRole::Host,
    );
    let request = RuntimePlatformRequest::new(
        RuntimePlatformEffect::UsbSdTransferStart,
        "host-sd2".into(),
        Some(4),
    );
    let response = adapter.handle_platform_effect(&request).unwrap();
    let [HostMessage::RuntimeResult {
        result: RuntimeStoreResult::RuntimeFailure { error },
    }] = response.as_slice()
    else {
        panic!("expected typed host-role SD2 failure");
    };
    assert_eq!(error.code, playback_runtime::RuntimeErrorCode::Unavailable);
    assert_eq!(error.request_id.as_deref(), Some("host-sd2"));
    assert_eq!(error.revision, Some(4));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn default_role_changes_are_serialized_and_leave_the_last_default() {
    let root = std::env::temp_dir().join(format!("octessera-pi-role-queue-{}", std::process::id()));
    let calls = Arc::new(Mutex::new(Vec::new()));
    let observed = calls.clone();
    let mut adapter = transaction_adapter(&root, move |role| {
        observed.lock().unwrap().push(role);
        Ok(())
    });
    for (id, role) in [("host", UsbDataRole::Host), ("gadget", UsbDataRole::Gadget)] {
        assert!(adapter
            .handle_platform_effect(&RuntimePlatformRequest::new(
                RuntimePlatformEffect::StoreSaveDefault {
                    payload: role_payload(role),
                    mode: None,
                },
                id.into(),
                None,
            ))
            .unwrap()
            .is_empty());
    }
    let barrier = adapter.platform_service.enqueue_test_barrier().unwrap();
    barrier
        .recv_timeout(std::time::Duration::from_secs(1))
        .unwrap();
    assert_eq!(
        &*calls.lock().unwrap(),
        &[UsbDataRole::Host, UsbDataRole::Gadget]
    );
    let saved: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("store/default.json")).unwrap()).unwrap();
    assert_eq!(saved["runtimeConfig"]["usb"]["dataRole"], "gadget");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn failed_role_change_restores_prior_default_bytes() {
    let root =
        std::env::temp_dir().join(format!("octessera-pi-role-rollback-{}", std::process::id()));
    std::fs::create_dir_all(root.join("store")).unwrap();
    let prior = serde_json::to_vec(&role_payload(UsbDataRole::Gadget)).unwrap();
    std::fs::write(root.join("store/default.json"), &prior).unwrap();
    let mut adapter = transaction_adapter(&root, |_| Err("helper failed".into()));
    assert!(adapter
        .handle_platform_effect(&RuntimePlatformRequest::new(
            RuntimePlatformEffect::StoreSaveDefault {
                payload: role_payload(UsbDataRole::Host),
                mode: None,
            },
            "rollback".into(),
            Some(9),
        ))
        .unwrap()
        .is_empty());
    let barrier = adapter.platform_service.enqueue_test_barrier().unwrap();
    barrier
        .recv_timeout(std::time::Duration::from_secs(1))
        .unwrap();
    let results = adapter.drain_platform_results(2);
    assert!(matches!(
        results.as_slice(),
        [HostMessage::RuntimeResult {
            result: RuntimeStoreResult::Identified { request_id, revision: Some(9), result }
        }] if request_id == "rollback" && matches!(result.as_ref(), RuntimeStoreResult::RuntimeFailure { .. })
    ));
    assert_eq!(
        std::fs::read(root.join("store/default.json")).unwrap(),
        prior
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn active_sd2_state_rejects_same_host_save_before_default_write() {
    let root =
        std::env::temp_dir().join(format!("octessera-pi-role-active-{}", std::process::id()));
    std::fs::create_dir_all(root.join("store")).unwrap();
    let prior = serde_json::to_vec(&role_payload(UsbDataRole::Host)).unwrap();
    std::fs::write(root.join("store/default.json"), &prior).unwrap();
    std::fs::write(root.join("storage.state"), b"active").unwrap();
    let mut adapter = transaction_adapter(&root, |_| panic!("role applier was called"));
    assert!(adapter
        .handle_platform_effect(&RuntimePlatformRequest::new(
            RuntimePlatformEffect::StoreSaveDefault {
                payload: role_payload(UsbDataRole::Host),
                mode: None,
            },
            "active".into(),
            None,
        ))
        .unwrap()
        .is_empty());
    let barrier = adapter.platform_service.enqueue_test_barrier().unwrap();
    barrier
        .recv_timeout(std::time::Duration::from_secs(1))
        .unwrap();
    let results = adapter.drain_platform_results(2);
    assert!(matches!(
        results.as_slice(),
        [HostMessage::RuntimeResult {
            result: RuntimeStoreResult::Identified { result, .. }
        }] if matches!(result.as_ref(), RuntimeStoreResult::RuntimeFailure { .. })
    ));
    assert_eq!(
        std::fs::read(root.join("store/default.json")).unwrap(),
        prior
    );
    let _ = std::fs::remove_dir_all(root);
}
