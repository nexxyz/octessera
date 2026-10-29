use super::{platform_request, temp_store_dir, test_adapter};
use crate::desktop_platform_service::shape_service_result;
use playback_runtime::{HostAdapter, HostMessage, RuntimePlatformEffect, RuntimeStoreResult};
use std::sync::mpsc;
use std::time::Instant;

#[test]
fn system_load_and_save_leave_pending_patch_save_intact() {
    let (mut adapter, _) = test_adapter();
    let temp_dir = temp_store_dir("system-store");
    adapter.store_dir = temp_dir.clone();
    let (platform_service_tx, platform_service_rx) = mpsc::sync_channel(32);
    adapter.platform_service_tx = platform_service_tx;
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
    assert!(saved.is_empty());
    let completion = shape_service_result(platform_service_rx.try_recv().unwrap());
    assert!(matches!(
        completion.as_slice(),
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
fn backup_and_recovery_write_only_patch_files() {
    let (mut adapter, _) = test_adapter();
    let temp_dir = temp_store_dir("system-apply-recovery");
    adapter.store_dir = temp_dir.clone();
    let patch = serde_json::json!({ "kind": "octessera.patch", "schemaVersion": 2 });

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
    assert!(!temp_dir.join("system.json").exists());
    let _ = std::fs::remove_dir_all(temp_dir);
}
