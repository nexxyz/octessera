use super::{platform_request, temp_store_dir, test_adapter};
use playback_runtime::{HostAdapter, HostMessage, RuntimePlatformEffect, RuntimeStoreResult};
use std::time::Instant;

#[test]
fn system_load_and_save_leave_pending_patch_save_intact() {
    let (mut adapter, _) = test_adapter();
    let temp_dir = temp_store_dir("system-store");
    adapter.store_dir = temp_dir.clone();
    let system_payload = serde_json::json!({ "deviceName": "Octessera", "nested": [1, true] });
    adapter
        .handle_platform_effect(&platform_request(RuntimePlatformEffect::StoreSaveDefault {
            payload: serde_json::json!({ "kind": "octessera.patch", "schemaVersion": 2 }),
            mode: Some("deferred".into()),
        }))
        .unwrap();

    let saved = adapter
        .handle_platform_effect(&platform_request(RuntimePlatformEffect::StoreSaveSystem {
            payload: system_payload.clone(),
        }))
        .unwrap();
    assert!(matches!(
        saved.as_slice(),
        [HostMessage::RuntimeResult {
            result: RuntimeStoreResult::Identified { result, .. }
        }] if matches!(result.as_ref(), RuntimeStoreResult::SaveSystemResult { ok: true })
    ));
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(
            &std::fs::read(temp_dir.join("system.json")).unwrap()
        )
        .unwrap(),
        system_payload
    );
    assert!(adapter.pending_default_save.is_pending());

    let loaded = adapter
        .handle_platform_effect(&platform_request(RuntimePlatformEffect::StoreLoadSystem))
        .unwrap();
    assert!(matches!(
        loaded.as_slice(),
        [HostMessage::RuntimeResult {
            result: RuntimeStoreResult::Identified { result, .. }
        }] if matches!(result.as_ref(), RuntimeStoreResult::LoadSystemResult { payload: Some(payload) } if payload == &system_payload)
    ));
    assert!(adapter.pending_default_save.is_pending());
    let entry = adapter.pending_default_save.take_now().unwrap();
    adapter
        .pending_default_save
        .schedule(entry.payload, Instant::now(), entry.request);
    adapter.flush_due_default_save().unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(
            &std::fs::read(temp_dir.join("default.patch.json")).unwrap()
        )
        .unwrap(),
        serde_json::json!({ "kind": "octessera.patch", "schemaVersion": 2 })
    );
    assert!(!temp_dir.join("default.json").exists());
    let _ = std::fs::remove_dir_all(temp_dir);
}

#[test]
fn malformed_system_load_returns_identified_storage_failure() {
    let (mut adapter, _) = test_adapter();
    let temp_dir = temp_store_dir("system-load");
    adapter.store_dir = temp_dir.clone();
    let missing = adapter
        .handle_platform_effect(&platform_request(RuntimePlatformEffect::StoreLoadSystem))
        .unwrap();
    assert!(matches!(
        missing.as_slice(),
        [HostMessage::RuntimeResult {
            result: RuntimeStoreResult::Identified { result, .. }
        }] if matches!(result.as_ref(), RuntimeStoreResult::LoadSystemResult { payload: None })
    ));
    std::fs::write(temp_dir.join("system.json"), "not json").unwrap();

    let malformed = adapter
        .handle_platform_effect(&platform_request(RuntimePlatformEffect::StoreLoadSystem))
        .unwrap();

    assert!(matches!(
        malformed.as_slice(),
        [HostMessage::RuntimeResult {
            result: RuntimeStoreResult::Identified { result, .. }
        }] if matches!(result.as_ref(), RuntimeStoreResult::RuntimeFailure { error }
            if error.domain == playback_runtime::RuntimeErrorDomain::Storage
                && error.operation == playback_runtime::RuntimeOperation::StoreLoadSystem)
    ));
    let _ = std::fs::remove_dir_all(temp_dir);
}

#[test]
fn apply_and_recovery_write_system_and_patch_files_independently() {
    let (mut adapter, _) = test_adapter();
    let temp_dir = temp_store_dir("system-apply-recovery");
    adapter.store_dir = temp_dir.clone();
    let system = serde_json::json!({ "kind": "octessera.system", "schemaVersion": 1 });
    let patch = serde_json::json!({ "kind": "octessera.patch", "schemaVersion": 2 });

    adapter
        .handle_platform_effect(&platform_request(
            RuntimePlatformEffect::ApplyDeviceConfigReboot {
                payload: system.clone(),
            },
        ))
        .unwrap();
    assert!(adapter.take_shutdown_request());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(
            &std::fs::read(temp_dir.join("system.json")).unwrap()
        )
        .unwrap(),
        system
    );
    assert!(!temp_dir.join("default.patch.json").exists());
    assert!(!temp_dir.join("default.json").exists());

    adapter
        .handle_platform_effect(&platform_request(RuntimePlatformEffect::StoreSaveBackup {
            payload: patch.clone(),
        }))
        .unwrap();
    adapter
        .handle_platform_effect(&platform_request(
            RuntimePlatformEffect::StoreSaveRecovery {
                payload: patch.clone(),
            },
        ))
        .unwrap();
    let backup = std::fs::read_dir(temp_dir.join("backups"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&std::fs::read(backup).unwrap()).unwrap(),
        patch
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(
            &std::fs::read(temp_dir.join("recovery-save.patch.json")).unwrap()
        )
        .unwrap(),
        patch
    );
    assert!(!temp_dir.join("recovery-save.json").exists());
    let _ = std::fs::remove_dir_all(temp_dir);
}

#[test]
fn failed_apply_system_write_does_not_request_restart() {
    let (mut adapter, _) = test_adapter();
    let root = temp_store_dir("apply-system-failed");
    let system_path = root.join("system.json");
    std::fs::create_dir(&system_path).unwrap();
    let old_path = system_path.join("old-bytes");
    std::fs::write(&old_path, b"keep me").unwrap();
    adapter.store_dir = root.clone();

    let result = adapter.handle_platform_effect(&platform_request(
        RuntimePlatformEffect::ApplyDeviceConfigReboot {
            payload: serde_json::json!({ "new": true }),
        },
    ));

    assert!(result.is_err());
    assert!(!adapter.take_shutdown_request());
    assert_eq!(std::fs::read(old_path).unwrap(), b"keep me");
    let _ = std::fs::remove_dir_all(root);
}
