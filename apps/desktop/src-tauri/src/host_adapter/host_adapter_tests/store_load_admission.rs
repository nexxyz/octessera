use super::{platform_request, temp_store_dir, test_adapter};
use playback_runtime::{HostAdapter, HostMessage, RuntimePlatformEffect, RuntimeStoreResult};
use std::time::Instant;

#[test]
fn pending_default_save_rejects_default_and_preset_loads_without_losing_bytes() {
    let (mut adapter, _) = test_adapter();
    let temp_dir = temp_store_dir("pending-save-load-rejection");
    adapter.store_dir = temp_dir.clone();
    let original_payload = serde_json::json!({ "patch": "pending-original" });
    std::fs::write(temp_dir.join("default.patch.json"), "not json").unwrap();
    let preset_path = temp_dir.join("presets").join("patches").join("Named.json");
    std::fs::create_dir_all(preset_path.parent().unwrap()).unwrap();
    std::fs::write(&preset_path, r#"{"patch":"named"}"#).unwrap();
    let request = platform_request(RuntimePlatformEffect::StoreSaveDefault {
        payload: original_payload.clone(),
        mode: Some("deferred".into()),
    });
    adapter
        .handle_platform_effect(&request)
        .expect("schedule deferred save");

    for (effect, operation) in [
        (
            RuntimePlatformEffect::StoreLoadDefault,
            playback_runtime::RuntimeOperation::StoreLoadDefault,
        ),
        (
            RuntimePlatformEffect::StoreLoadPreset {
                name: "Named".into(),
            },
            playback_runtime::RuntimeOperation::StoreLoadPreset,
        ),
    ] {
        let result = adapter
            .handle_platform_effect(&platform_request(effect))
            .expect("load rejection is a runtime result");
        assert!(matches!(
            result.as_slice(),
            [HostMessage::RuntimeResult {
                result: identified @ RuntimeStoreResult::Identified { request_id, result, .. }
            }] if request_id == "test-request"
                && matches!(
                    result.as_ref(),
                    RuntimeStoreResult::RuntimeFailure { error }
                        if error.domain == playback_runtime::RuntimeErrorDomain::Storage
                            && error.operation == operation
                            && error.message.as_deref() == Some("Save pending, try again")
                )
                && identified.operation() == operation
        ));
        assert!(adapter.pending_default_save.is_pending());
    }

    assert_eq!(
        std::fs::read_to_string(temp_dir.join("default.patch.json")).unwrap(),
        "not json"
    );
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
        original_payload
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&std::fs::read(preset_path).unwrap()).unwrap(),
        serde_json::json!({ "patch": "named" })
    );
    let _ = std::fs::remove_dir_all(temp_dir);
}

#[test]
fn default_and_preset_loads_succeed_without_pending_default_save() {
    let (mut adapter, _) = test_adapter();
    let temp_dir = temp_store_dir("patch-load-without-pending-save");
    adapter.store_dir = temp_dir.clone();
    let default_payload = serde_json::json!({ "default": true });
    let preset_payload = serde_json::json!({ "preset": true });
    std::fs::write(
        temp_dir.join("default.patch.json"),
        serde_json::to_vec(&default_payload).unwrap(),
    )
    .unwrap();
    let preset_path = temp_dir.join("presets").join("patches").join("Named.json");
    std::fs::create_dir_all(preset_path.parent().unwrap()).unwrap();
    std::fs::write(&preset_path, serde_json::to_vec(&preset_payload).unwrap()).unwrap();

    for (effect, operation, expected) in [
        (
            RuntimePlatformEffect::StoreLoadDefault,
            playback_runtime::RuntimeOperation::StoreLoadDefault,
            default_payload,
        ),
        (
            RuntimePlatformEffect::StoreLoadPreset {
                name: "Named".into(),
            },
            playback_runtime::RuntimeOperation::StoreLoadPreset,
            preset_payload,
        ),
    ] {
        let result = adapter
            .handle_platform_effect(&platform_request(effect))
            .unwrap();
        assert!(matches!(
            result.as_slice(),
            [HostMessage::RuntimeResult {
                result: identified @ RuntimeStoreResult::Identified { request_id, result, .. }
            }] if request_id == "test-request"
                && identified.operation() == operation
                && match result.as_ref() {
                    RuntimeStoreResult::LoadDefaultResult { payload } => payload.as_ref() == Some(&expected),
                    RuntimeStoreResult::LoadPresetResult { payload, .. } => payload.as_ref() == Some(&expected),
                    _ => false,
                }
        ));
    }
    let _ = std::fs::remove_dir_all(temp_dir);
}
