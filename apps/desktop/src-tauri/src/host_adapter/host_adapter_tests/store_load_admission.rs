use super::{platform_request, temp_store_dir, test_adapter};
use playback_runtime::{HostAdapter, HostMessage, RuntimePlatformEffect, RuntimeStoreResult};
use std::time::Instant;

fn factory_documents() -> playback_runtime::SystemPatchDocuments {
    let bundled = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../config/generated/desktop/default.json"
    )))
    .unwrap();
    playback_runtime::split_system_patch_documents(&bundled).unwrap()
}

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
        mode: Some(playback_runtime::DefaultSaveMode::Deferred),
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
    let documents = factory_documents();
    std::fs::write(
        temp_dir.join("system.json"),
        serde_json::to_vec(&documents.system).unwrap(),
    )
    .unwrap();
    let preset_payload = documents.patch;
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

#[test]
fn a_damaged_preset_is_refused_with_the_boards_error() {
    let (mut adapter, _) = test_adapter();
    let temp_dir = temp_store_dir("damaged-preset-load");
    adapter.store_dir = temp_dir.clone();
    let documents = factory_documents();
    std::fs::write(
        temp_dir.join("system.json"),
        serde_json::to_vec(&documents.system).unwrap(),
    )
    .unwrap();
    let mut damaged = documents.patch;
    damaged["runtimeConfig"]["fromTheFuture"] = serde_json::json!(true);
    let expected =
        playback_runtime::compose_system_patch_documents(&documents.system, &damaged).unwrap_err();
    let preset_path = temp_dir.join("presets").join("patches").join("Named.json");
    std::fs::create_dir_all(preset_path.parent().unwrap()).unwrap();
    std::fs::write(&preset_path, serde_json::to_vec(&damaged).unwrap()).unwrap();

    let result = adapter
        .handle_platform_effect(&platform_request(RuntimePlatformEffect::StoreLoadPreset {
            name: "Named".into(),
        }))
        .unwrap();

    assert!(matches!(
        result.as_slice(),
        [HostMessage::RuntimeResult {
            result: RuntimeStoreResult::RuntimeFailure { error }
        }] if error.operation == playback_runtime::RuntimeOperation::StoreLoadPreset
            && error.code == playback_runtime::RuntimeErrorCode::OperationFailed
            && error.preset_name.as_deref() == Some("Named")
            && error.request_id.as_deref() == Some("test-request")
            && error.message.as_deref() == Some(expected.as_str())
    ));
    let _ = std::fs::remove_dir_all(temp_dir);
}
