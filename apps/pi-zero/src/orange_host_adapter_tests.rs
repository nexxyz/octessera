use super::*;
use crate::audio::test_service;
use playback_runtime::{
    CoreRunner, HostAdapter, NativeRunner, NativeRunnerConfig, PlaybackRuntime, RuntimeConfig,
    RuntimePlatformEffect, RuntimePlatformRequest, RuntimeStoreResult, RuntimeUserDataRestorePhase,
    RuntimeUserDataRestoreStatus,
};
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;

#[path = "orange_host_adapter_drain_tests.rs"]
mod drain_tests;
#[path = "orange_host_adapter_media_tests.rs"]
mod media_tests;
#[path = "orange_host_adapter_setup_portal_tests.rs"]
mod setup_portal_tests;

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
    let entry = adapter.pending_default_save.take_now().unwrap();
    adapter
        .pending_default_save
        .schedule(entry.payload, Instant::now(), entry.request);
    assert!(adapter.flush_due_default_save().unwrap().is_empty());
    assert!(!adapter.pending_default_save.is_pending());
    let results = wait_for_result(&adapter);
    let [HostMessage::RuntimeResult {
        result:
            RuntimeStoreResult::Identified {
                result,
                request_id,
                revision,
            },
    }] = results.as_slice()
    else {
        panic!("expected identified deferred save result");
    };
    assert_eq!(request_id, "deferred-save");
    assert_eq!(*revision, Some(1));
    assert!(matches!(
        result.as_ref(),
        RuntimeStoreResult::SaveDefaultResult {
            ok: true,
            is_auto: Some(true),
        }
    ));
    assert!(store.join("default.json").is_file());
    let _ = std::fs::remove_dir_all(store.parent().unwrap());
    let _ = std::fs::remove_dir_all(samples);
}

#[test]
fn restore_barrier_cancels_pending_deferred_save_before_flush() {
    let (mut adapter, store, samples) = adapter();
    let request = request(
        RuntimePlatformEffect::StoreSaveDefault {
            payload: json!({"stale": true}),
            mode: Some("deferred".into()),
        },
        "restore-race",
    );
    assert!(adapter.handle_platform_effect(&request).unwrap().is_empty());
    let entry = adapter.pending_default_save.take_now().unwrap();
    adapter
        .pending_default_save
        .schedule(entry.payload, Instant::now(), entry.request);

    adapter.platform_service.invalidate_store_writes_for_test();
    assert!(adapter.flush_due_default_save().unwrap().is_empty());
    assert!(!adapter.pending_default_save.is_pending());
    assert!(!store.join("default.json").exists());

    adapter.platform_service.acknowledge_restored_state();
    let _ = std::fs::remove_dir_all(store.parent().unwrap());
    let _ = std::fs::remove_dir_all(samples);
}

#[test]
fn runtime_restore_loads_default_before_orange_barrier_acknowledgement() {
    let (mut adapter, store, samples) = adapter();
    let mut payload = crate::user_data_archive::canonical_defaults();
    payload["runtimeConfig"]["masterVolume"] = json!(81);
    std::fs::write(
        store.join("default.json"),
        serde_json::to_vec(&payload).unwrap(),
    )
    .unwrap();
    adapter.platform_service.invalidate_store_writes_for_test();

    let mut playback = PlaybackRuntime::new(RuntimeConfig::default());
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let status_messages = runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::UserDataRestoreStatus {
                status: RuntimeUserDataRestoreStatus {
                    phase: RuntimeUserDataRestorePhase::Succeeded,
                },
            }
            .with_identity("restore-e2e".into(), Some(3)),
        })
        .unwrap();
    assert!(adapter.platform_service.store_writes_blocked());
    playback
        .dispatch_runner_messages(status_messages, &mut runner, &mut adapter)
        .unwrap();

    assert!(!adapter.platform_service.store_writes_blocked());
    assert_eq!(
        playback.last_snapshot().unwrap()["settings"]["masterVolume"],
        81
    );
    let _ = std::fs::remove_dir_all(store.parent().unwrap());
    let _ = std::fs::remove_dir_all(samples);
}

#[test]
fn failed_runtime_restore_apply_keeps_orange_barrier_blocked() {
    let (mut adapter, store, samples) = adapter();
    let recovery = br#"{"recovery":true}"#;
    std::fs::write(store.join("default.json"), br#"{"runtimeConfig":"bad"}"#).unwrap();
    std::fs::write(store.join("recovery-save.json"), recovery).unwrap();
    adapter.platform_service.invalidate_store_writes_for_test();

    let mut playback = PlaybackRuntime::new(RuntimeConfig::default());
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let status_messages = runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::UserDataRestoreStatus {
                status: RuntimeUserDataRestoreStatus {
                    phase: RuntimeUserDataRestorePhase::Succeeded,
                },
            }
            .with_identity("restore-failed".into(), Some(4)),
        })
        .unwrap();
    playback
        .dispatch_runner_messages(status_messages, &mut runner, &mut adapter)
        .unwrap();

    assert!(adapter.platform_service.store_writes_blocked());
    assert_eq!(
        std::fs::read(store.join("recovery-save.json")).unwrap(),
        recovery
    );
    let _ = std::fs::remove_dir_all(store.parent().unwrap());
    let _ = std::fs::remove_dir_all(samples);
}

#[path = "orange_host_adapter_parity_tests.rs"]
mod parity_tests;
