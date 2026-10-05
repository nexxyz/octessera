use super::*;
use std::sync::{Arc, Mutex};

fn system_patch(role: UsbDataRole) -> (serde_json::Value, serde_json::Value) {
    let mut full = crate::user_data_archive::canonical_defaults();
    full["runtimeConfig"]["usb"]["dataRole"] = serde_json::json!(role.as_str());
    if role == UsbDataRole::Host {
        full["runtimeConfig"]["audioOutputs"]["usb"] = serde_json::json!(false);
        full["runtimeConfig"]["usb"]["midiOutEnabled"] = serde_json::json!(false);
    }
    let documents = playback_runtime::split_system_patch_documents(&full).unwrap();
    (documents.system, documents.patch)
}

fn write_pair(root: &std::path::Path, role: UsbDataRole) -> (serde_json::Value, serde_json::Value) {
    let store = root.join("store");
    std::fs::create_dir_all(&store).unwrap();
    let (system, patch) = system_patch(role);
    std::fs::write(
        store.join("system.json"),
        serde_json::to_vec(&system).unwrap(),
    )
    .unwrap();
    std::fs::write(
        crate::platform_service::default_patch_path(&store),
        serde_json::to_vec(&patch).unwrap(),
    )
    .unwrap();
    (system, patch)
}

fn transaction_adapter(
    root: &std::path::Path,
    apply_role: impl Fn(UsbDataRole) -> Result<(), String> + Send + Sync + 'static,
) -> PiHostAdapter {
    let service = PiPlatformService::new_with_role_applier(
        root.join("store"),
        root.join("samples"),
        Arc::new(apply_role),
    );
    PiHostAdapter::with_platform_service_and_role(
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
    let root = crate::test_temp_dir::unique_temp_path("octessera-pi-host-role");
    let mut adapter = PiHostAdapter::new_with_data_role(
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
fn saving_system_role_does_not_apply_it_or_modify_the_patch() {
    let root = std::env::temp_dir().join(format!("octessera-pi-role-save-{}", std::process::id()));
    let calls = Arc::new(Mutex::new(Vec::new()));
    let observed = calls.clone();
    let (host_system, patch) = write_pair(&root, UsbDataRole::Gadget);
    let mut requested_system = host_system;
    requested_system["runtimeConfig"]["usb"]["dataRole"] =
        serde_json::json!(UsbDataRole::Host.as_str());
    let mut adapter = transaction_adapter(&root, move |role| {
        observed.lock().unwrap().push(role);
        Ok(())
    });
    let result = adapter
        .handle_platform_effect(&RuntimePlatformRequest::new(
            RuntimePlatformEffect::StoreSaveSystem {
                payload: requested_system.clone(),
            },
            "system-save".into(),
            None,
        ))
        .unwrap();
    assert!(result.is_empty());
    adapter
        .core
        .platform_service
        .enqueue_test_barrier()
        .unwrap()
        .recv_timeout(std::time::Duration::from_secs(1))
        .unwrap();
    assert!(matches!(
        adapter.core.platform_service.drain_results(4).as_slice(),
        [HostMessage::RuntimeResult {
            result: RuntimeStoreResult::Identified { request_id, result, .. }
        }] if request_id == "system-save"
            && matches!(result.as_ref(), RuntimeStoreResult::SaveSystemResult { ok: true })
    ));
    assert!(calls.lock().unwrap().is_empty());
    assert!(adapter.power_request.is_none());
    assert_eq!(
        crate::platform_service::load_json(&root.join("store/system.json")).unwrap(),
        Some(requested_system)
    );
    assert_eq!(
        crate::platform_service::load_json(&root.join("store/default.patch.json")).unwrap(),
        Some(patch)
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn failed_device_apply_restores_prior_system_bytes_without_touching_patch() {
    let root =
        std::env::temp_dir().join(format!("octessera-pi-role-rollback-{}", std::process::id()));
    let (prior_system, patch) = write_pair(&root, UsbDataRole::Gadget);
    let prior_system_bytes = std::fs::read(root.join("store/system.json")).unwrap();
    let mut adapter = transaction_adapter(&root, |_| Err("helper failed".into()));
    let mut next_system = prior_system;
    next_system["runtimeConfig"]["usb"]["dataRole"] = serde_json::json!(UsbDataRole::Host.as_str());
    let result = adapter
        .handle_platform_effect(&RuntimePlatformRequest::new(
            RuntimePlatformEffect::ApplyDeviceConfigReboot {
                payload: next_system,
            },
            "apply".into(),
            Some(9),
        ))
        .unwrap();
    assert!(matches!(
        result.as_slice(),
        [HostMessage::RuntimeResult {
            result: RuntimeStoreResult::RuntimeFailure { .. }
        }]
    ));
    assert_eq!(
        std::fs::read(root.join("store/system.json")).unwrap(),
        prior_system_bytes
    );
    assert_eq!(
        crate::platform_service::load_json(&root.join("store/default.patch.json")).unwrap(),
        Some(patch)
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn active_sd2_state_rejects_device_apply_before_system_write() {
    let root =
        std::env::temp_dir().join(format!("octessera-pi-role-active-{}", std::process::id()));
    let (prior_system, patch) = write_pair(&root, UsbDataRole::Host);
    let prior_system_bytes = std::fs::read(root.join("store/system.json")).unwrap();
    std::fs::write(root.join("storage.state"), b"active").unwrap();
    let mut adapter = transaction_adapter(&root, |_| panic!("role applier was called"));
    let result = adapter
        .handle_platform_effect(&RuntimePlatformRequest::new(
            RuntimePlatformEffect::ApplyDeviceConfigReboot {
                payload: prior_system,
            },
            "active-apply".into(),
            None,
        ))
        .unwrap();
    assert!(matches!(
        result.as_slice(),
        [HostMessage::RuntimeResult {
            result: RuntimeStoreResult::RuntimeFailure { .. }
        }]
    ));
    assert_eq!(
        std::fs::read(root.join("store/system.json")).unwrap(),
        prior_system_bytes
    );
    assert_eq!(
        crate::platform_service::load_json(&root.join("store/default.patch.json")).unwrap(),
        Some(patch)
    );
    let _ = std::fs::remove_dir_all(root);
}
