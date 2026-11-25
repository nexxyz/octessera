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

fn load_named_preset_in_factory_store(label: &str, preset_bytes: &[u8]) -> Vec<HostMessage> {
    let (mut adapter, _) = test_adapter();
    let temp_dir = temp_store_dir(label);
    adapter.store_dir = temp_dir.clone();
    std::fs::write(
        temp_dir.join("system.json"),
        serde_json::to_vec(&factory_documents().system).unwrap(),
    )
    .unwrap();
    let preset_path = temp_dir.join("presets").join("patches").join("Named.json");
    std::fs::create_dir_all(preset_path.parent().unwrap()).unwrap();
    std::fs::write(&preset_path, preset_bytes).unwrap();
    let result = adapter
        .handle_platform_effect(&platform_request(RuntimePlatformEffect::StoreLoadPreset {
            name: "Named".into(),
        }))
        .unwrap();
    let _ = std::fs::remove_dir_all(temp_dir);
    result
}

fn preset_refusal(result: &[HostMessage]) -> String {
    let [HostMessage::RuntimeResult {
        result: RuntimeStoreResult::RuntimeFailure { error },
    }] = result
    else {
        panic!("expected a preset load failure, got {result:?}");
    };
    assert_eq!(
        error.operation,
        playback_runtime::RuntimeOperation::StoreLoadPreset
    );
    assert_eq!(
        error.code,
        playback_runtime::RuntimeErrorCode::OperationFailed
    );
    assert_eq!(error.preset_name.as_deref(), Some("Named"));
    assert_eq!(error.request_id.as_deref(), Some("test-request"));
    error.message.clone().unwrap()
}

#[test]
fn a_damaged_preset_is_refused_with_the_boards_error() {
    let documents = factory_documents();
    let mut damaged = documents.patch;
    damaged["runtimeConfig"]["fromTheFuture"] = serde_json::json!(true);
    let expected =
        playback_runtime::compose_system_patch_documents(&documents.system, &damaged).unwrap_err();

    let result = load_named_preset_in_factory_store(
        "damaged-preset-load",
        &serde_json::to_vec(&damaged).unwrap(),
    );

    let message = preset_refusal(&result);
    assert_eq!(message, expected);
    assert!(message.contains("saved by a newer Octessera"), "{message}");
}

#[test]
fn a_preset_that_is_not_json_is_refused_as_on_the_boards() {
    let result = load_named_preset_in_factory_store("non-json-preset-load", b"not json");

    assert!(!preset_refusal(&result).is_empty());
}
