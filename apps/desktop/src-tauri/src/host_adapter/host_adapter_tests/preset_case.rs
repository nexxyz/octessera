//! Preset names that differ only in case name one file on the desktop's NTFS store.
use super::{platform_request, temp_store_dir, test_adapter};
use crate::host_adapter::DesktopPlaybackHostAdapter;
use playback_runtime::{
    HostAdapter, HostMessage, PresetSaveMode, RuntimeOperation, RuntimePlatformEffect,
    RuntimeStoreResult,
};
use serde_json::json;

fn effect(
    adapter: &mut DesktopPlaybackHostAdapter,
    effect: RuntimePlatformEffect,
) -> Vec<RuntimeStoreResult> {
    adapter
        .handle_platform_effect(&platform_request(effect))
        .unwrap()
        .into_iter()
        .map(|message| match message {
            HostMessage::RuntimeResult { result } => result,
            other => panic!("unexpected host message {other:?}"),
        })
        .collect()
}

fn save(
    adapter: &mut DesktopPlaybackHostAdapter,
    name: &str,
    payload: serde_json::Value,
) -> Vec<RuntimeStoreResult> {
    effect(
        adapter,
        RuntimePlatformEffect::StoreSavePreset {
            name: name.into(),
            payload,
            mode: None,
        },
    )
}

fn names(adapter: &DesktopPlaybackHostAdapter) -> Vec<String> {
    adapter.list_preset_names().unwrap()
}

#[test]
fn renaming_a_preset_to_another_case_keeps_it() {
    let (mut adapter, _) = test_adapter();
    let root = temp_store_dir("preset-case-rename");
    adapter.store_dir = root.clone();
    save(&mut adapter, "Song", json!({ "take": 1 }));

    save(&mut adapter, "song", json!({ "take": 2 }));
    let deleted = effect(
        &mut adapter,
        RuntimePlatformEffect::StoreDeletePreset {
            name: "Song".into(),
        },
    );

    assert!(matches!(
        deleted.as_slice(),
        [RuntimeStoreResult::DeletePresetResult { ok: true, .. }]
    ));
    assert_eq!(names(&adapter), ["song"]);
    assert_eq!(
        adapter.load_preset_payload("song").unwrap(),
        Some(json!({ "take": 2 }))
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn saving_over_a_case_variant_replaces_it_and_refreshes_the_list() {
    let (mut adapter, _) = test_adapter();
    let root = temp_store_dir("preset-case-save");
    adapter.store_dir = root.clone();
    save(&mut adapter, "Song", json!({ "take": 1 }));

    let results = save(&mut adapter, "song", json!({ "take": 2 }));

    assert!(matches!(
        results.as_slice(),
        [
            RuntimeStoreResult::SavePresetResult { name, .. },
            RuntimeStoreResult::ListPresetsResult { names },
        ] if name == "song" && names == &["song".to_string()]
    ));
    assert_eq!(names(&adapter), ["song"]);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn saving_onto_an_existing_preset_is_refused_unless_told_to_overwrite() {
    let (mut adapter, _) = test_adapter();
    let root = temp_store_dir("preset-overwrite-mode");
    adapter.store_dir = root.clone();
    save(&mut adapter, "Song", json!({ "take": 1 }));

    let refused = save(&mut adapter, "Song", json!({ "take": 2 }));
    assert!(matches!(
        refused.as_slice(),
        [RuntimeStoreResult::RuntimeFailure { error }]
            if error.operation == RuntimeOperation::StoreSavePreset
                && error.message.as_deref() == Some("Save Song failed: preset already exists")
    ));
    assert_eq!(
        adapter.load_preset_payload("Song").unwrap(),
        Some(json!({ "take": 1 }))
    );

    let overwritten = effect(
        &mut adapter,
        RuntimePlatformEffect::StoreSavePreset {
            name: "Song".into(),
            payload: json!({ "take": 2 }),
            mode: Some(PresetSaveMode::Overwrite),
        },
    );
    assert!(matches!(
        overwritten.as_slice(),
        [RuntimeStoreResult::SavePresetResult { name, .. }] if name == "Song"
    ));
    assert_eq!(
        adapter.load_preset_payload("Song").unwrap(),
        Some(json!({ "take": 2 }))
    );
    let _ = std::fs::remove_dir_all(root);
}
