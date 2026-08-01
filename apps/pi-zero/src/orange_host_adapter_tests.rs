use super::*;
use crate::audio::test_service;
use playback_runtime::{
    HostAdapter, RuntimeErrorCode, RuntimeOperation, RuntimePlatformEffect, RuntimePlatformRequest,
    RuntimeStoreResult,
};
use serde_json::json;
use std::sync::Arc;

fn directories() -> (PathBuf, PathBuf) {
    let root = std::env::temp_dir().join(format!(
        "octessera-orange-host-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    (root.join("store"), root.join("samples"))
}

fn adapter() -> (OrangeHostAdapter, PathBuf, PathBuf) {
    let (store, samples) = directories();
    let (audio, _, _) = test_service();
    let adapter = OrangeHostAdapter::with_directories(
        audio,
        store.clone(),
        samples.clone(),
        Arc::new(|_| {}),
        false,
    )
    .unwrap();
    (adapter, store, samples)
}

fn request(effect: RuntimePlatformEffect, id: &str) -> RuntimePlatformRequest {
    RuntimePlatformRequest::new(effect, id.into(), Some(1))
}

fn wait_for_result(adapter: &OrangeHostAdapter) -> Vec<HostMessage> {
    adapter
        .platform_service
        .enqueue_test_barrier()
        .unwrap()
        .recv_timeout(Duration::from_secs(1))
        .unwrap();
    adapter.drain_results(4)
}

fn unwrap_result(message: HostMessage) -> RuntimeStoreResult {
    let HostMessage::RuntimeResult { result } = message else {
        panic!("expected runtime result");
    };
    match result {
        RuntimeStoreResult::Identified { result, .. } => *result,
        result => result,
    }
}

#[test]
fn first_backup_effect_does_not_latch_an_error() {
    let (mut adapter, store, samples) = adapter();
    let response = adapter
        .handle_platform_effect(&request(
            RuntimePlatformEffect::StoreSaveBackup {
                payload: json!({"runtimeConfig": {"bpm": 120}}),
            },
            "backup-1",
        ))
        .unwrap();
    assert!(response.is_empty());
    let _ = wait_for_result(&adapter);
    let _ = std::fs::remove_dir_all(store.parent().unwrap());
    let _ = std::fs::remove_dir_all(samples);
}

#[test]
fn default_and_preset_round_trips_use_atomic_service() {
    let (mut adapter, store, samples) = adapter();
    let payload = json!({"runtimeConfig": {"bpm": 99}});
    assert!(adapter
        .handle_platform_effect(&request(
            RuntimePlatformEffect::StoreSaveDefault {
                payload: payload.clone(),
                mode: None,
            },
            "default-save",
        ))
        .unwrap()
        .is_empty());
    let _ = wait_for_result(&adapter);
    let loaded = adapter
        .handle_platform_effect(&request(
            RuntimePlatformEffect::StoreLoadDefault,
            "default-load",
        ))
        .unwrap();
    assert!(matches!(
        loaded.as_slice(),
        [HostMessage::RuntimeResult {
            result: RuntimeStoreResult::LoadDefaultResult {
                payload: Some(value)
            }
        }] if value == &payload
    ));

    assert!(adapter
        .handle_platform_effect(&request(
            RuntimePlatformEffect::StoreSavePreset {
                name: "round-trip".into(),
                payload: payload.clone(),
                mode: None,
            },
            "preset-save",
        ))
        .unwrap()
        .is_empty());
    let save_result = unwrap_result(wait_for_result(&adapter).remove(0));
    assert!(matches!(
        save_result,
        RuntimeStoreResult::SavePresetResult { .. }
    ));
    let patch_path = store.join("patches").join("round-trip.json");
    assert!(patch_path.is_file());
    assert!(std::fs::read_dir(patch_path.parent().unwrap())
        .unwrap()
        .all(|entry| !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .contains(".tmp-")));
    assert!(adapter
        .handle_platform_effect(&request(
            RuntimePlatformEffect::StoreLoadPreset {
                name: "round-trip".into(),
            },
            "preset-load",
        ))
        .unwrap()
        .is_empty());
    let load_result = unwrap_result(wait_for_result(&adapter).remove(0));
    assert!(matches!(
        load_result,
        RuntimeStoreResult::LoadPresetResult { payload: Some(value), .. }
            if value == payload
    ));
    let _ = std::fs::remove_dir_all(store.parent().unwrap());
    let _ = std::fs::remove_dir_all(samples);
}

#[test]
fn midi_panic_and_clear_selection_succeed_without_selected_ports() {
    let (store, samples) = directories();
    let (audio, _, _event_rx) = test_service();
    let mut adapter = OrangeHostAdapter::with_directories(
        audio,
        store.clone(),
        samples.clone(),
        Arc::new(|_| {}),
        false,
    )
    .unwrap();
    for effect in [
        RuntimePlatformEffect::MidiPanic,
        RuntimePlatformEffect::MidiSelectOutput { id: None },
        RuntimePlatformEffect::MidiSelectInput { id: None },
    ] {
        let responses = adapter
            .handle_platform_effect(&request(effect, "midi"))
            .unwrap();
        let [HostMessage::RuntimeResult { result }] = responses.as_slice() else {
            panic!("expected MIDI status");
        };
        assert!(matches!(
            result,
            RuntimeStoreResult::MidiStatus { ok: true, .. }
        ));
    }
    let _ = std::fs::remove_dir_all(store.parent().unwrap());
    let _ = std::fs::remove_dir_all(samples);
}

#[test]
fn deferred_default_save_can_be_flushed() {
    let (mut adapter, store, samples) = adapter();
    let request = request(
        RuntimePlatformEffect::StoreSaveDefault {
            payload: json!({"runtimeConfig": {"bpm": 88}}),
            mode: Some("deferred".into()),
        },
        "deferred-save",
    );
    assert!(adapter.handle_platform_effect(&request).unwrap().is_empty());
    adapter.pending_default_save.as_mut().unwrap().1 = Instant::now();
    assert!(adapter.flush_due_default_save().unwrap().is_empty());
    let _ = wait_for_result(&adapter);
    assert!(store.join("default.json").is_file());
    let _ = std::fs::remove_dir_all(store.parent().unwrap());
    let _ = std::fs::remove_dir_all(samples);
}

#[test]
fn orange_update_effects_fail_closed_as_unsupported() {
    let (mut adapter, store, samples) = adapter();
    for (effect, message) in [
        (
            RuntimePlatformEffect::UpdateCheck,
            "does not support update checks",
        ),
        (
            RuntimePlatformEffect::UpdateApply,
            "does not support update apply",
        ),
        (RuntimePlatformEffect::Rollback, "does not support rollback"),
    ] {
        let request_id = format!("orange-update-{message}");
        let responses = adapter
            .handle_platform_effect(&request(effect, &request_id))
            .unwrap();
        let [HostMessage::RuntimeResult {
            result: RuntimeStoreResult::RuntimeFailure { error },
        }] = responses.as_slice()
        else {
            panic!("expected unsupported update result");
        };
        assert_eq!(error.code, RuntimeErrorCode::Unsupported);
        assert_eq!(error.operation, RuntimeOperation::DeviceUpdate);
        assert_eq!(error.request_id.as_deref(), Some(request_id.as_str()));
        assert_eq!(error.revision, Some(1));
        assert!(error.message.as_deref().unwrap().contains(message));
    }
    assert!(adapter.drain_results(4).is_empty());
    let _ = std::fs::remove_dir_all(store.parent().unwrap());
    let _ = std::fs::remove_dir_all(samples);
}
