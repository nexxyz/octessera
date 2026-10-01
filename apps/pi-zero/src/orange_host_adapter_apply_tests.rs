use super::*;
use crate::audio::test_service;
use playback_runtime::{HostAdapter, MusicalEvent, RuntimePlatformEffect, RuntimePlatformRequest};
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;

fn directories(label: &str) -> (PathBuf, PathBuf, PathBuf) {
    let root = std::env::temp_dir().join(format!(
        "octessera-orange-apply-host-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    (root.clone(), root.join("store"), root.join("samples"))
}

fn request(effect: RuntimePlatformEffect, id: &str) -> RuntimePlatformRequest {
    RuntimePlatformRequest::new(effect, id.into(), Some(1))
}

fn documents() -> playback_runtime::SystemPatchDocuments {
    let full = crate::user_data_archive::canonical_defaults();
    playback_runtime::split_system_patch_documents(&full).unwrap()
}

fn system_for_apply() -> serde_json::Value {
    let mut system = documents().system;
    system["runtimeConfig"]["screenSleepSeconds"] = json!(45);
    system
}

fn seed_store(store: &std::path::Path) {
    crate::pi_store_test_support::write_pair(
        store,
        &crate::user_data_archive::canonical_defaults(),
    );
}

fn adapter(label: &str) -> (OrangeHostAdapter, PathBuf) {
    let (root, store, samples) = directories(label);
    let (audio, _, _) = test_service();
    let documents = documents();
    std::fs::create_dir_all(&store).unwrap();
    std::fs::write(
        store.join("system.json"),
        serde_json::to_vec(&documents.system).unwrap(),
    )
    .unwrap();
    std::fs::write(
        crate::platform_service::default_patch_path(&store),
        serde_json::to_vec(&documents.patch).unwrap(),
    )
    .unwrap();
    let adapter =
        OrangeHostAdapter::with_directories(audio, store, samples, Arc::new(|_| {}), false)
            .unwrap();
    (adapter, root)
}

fn arm_recovery_save(adapter: &mut OrangeHostAdapter, id: &str) {
    assert_eq!(
        adapter
            .handle_platform_effect(&request(
                RuntimePlatformEffect::StoreSaveRecovery {
                    payload: documents().patch,
                },
                id,
            ))
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn apply_waits_behind_queued_default_save() {
    let (mut adapter, root) = adapter("fifo");
    let prior_system_bytes = std::fs::read(root.join("store/system.json")).unwrap();
    let mut earlier = documents().patch;
    earlier["runtimeConfig"]["bpm"] = json!(110);
    let applied = system_for_apply();
    assert!(adapter
        .handle_platform_effect(&request(
            RuntimePlatformEffect::StoreSaveDefault {
                payload: earlier.clone(),
                mode: None,
            },
            "earlier",
        ))
        .unwrap()
        .is_empty());
    assert!(adapter
        .handle_platform_effect(&request(
            RuntimePlatformEffect::ApplyDeviceConfigReboot {
                payload: applied.clone(),
            },
            "apply",
        ))
        .unwrap()
        .is_empty());
    assert!(!adapter.pending_default_save.is_pending());

    let store = root.join("store");
    let record: serde_json::Value = serde_json::from_slice(
        &std::fs::read(store.join(crate::orange_device_apply::TRANSACTION_FILE_NAME)).unwrap(),
    )
    .unwrap();
    let prior: Vec<u8> = record["prior_default_bytes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|byte| byte.as_u64().unwrap() as u8)
        .collect();
    assert_eq!(prior, prior_system_bytes);
    assert_eq!(
        crate::platform_service::load_json(&store.join("default.patch.json")).unwrap(),
        Some(earlier)
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(
            &std::fs::read(store.join("system.json")).unwrap()
        )
        .unwrap(),
        applied
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn later_saves_and_applies_are_rejected_while_shutdown_is_pending() {
    let (mut adapter, root) = adapter("reject");
    let applied = system_for_apply();
    assert!(adapter
        .handle_platform_effect(&request(
            RuntimePlatformEffect::ApplyDeviceConfigReboot {
                payload: applied.clone(),
            },
            "apply",
        ))
        .unwrap()
        .is_empty());
    assert!(!adapter
        .handle_platform_effect(&request(
            RuntimePlatformEffect::StoreSaveDefault {
                payload: json!({"later": true}),
                mode: None,
            },
            "later-save",
        ))
        .unwrap()
        .is_empty());
    assert!(!adapter
        .handle_platform_effect(&request(
            RuntimePlatformEffect::ApplyDeviceConfigReboot {
                payload: json!({"later": true}),
            },
            "later-apply",
        ))
        .unwrap()
        .is_empty());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(
            &std::fs::read(root.join("store/system.json")).unwrap()
        )
        .unwrap(),
        applied
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn pending_apply_suppresses_later_musical_output() {
    let (root, store, samples) = directories("silent");
    seed_store(&store);
    let (audio, _, mut event_rx) = test_service();
    let mut adapter =
        OrangeHostAdapter::with_directories(audio, store, samples, Arc::new(|_| {}), false)
            .unwrap();
    assert!(adapter
        .handle_platform_effect(&request(
            RuntimePlatformEffect::ApplyDeviceConfigReboot {
                payload: system_for_apply(),
            },
            "apply",
        ))
        .unwrap()
        .is_empty());
    HostAdapter::handle_musical_event(
        &mut adapter,
        &MusicalEvent::NoteOn {
            channel: 0,
            note: 60,
            velocity: 100,
            duration_ms: None,
        },
    )
    .unwrap();
    HostAdapter::handle_midi_message(&mut adapter, &[0x90, 60, 100]).unwrap();
    assert!(event_rx.try_recv().is_err());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn pending_reboot_suppresses_later_musical_output() {
    let (root, store, samples) = directories("reboot-silent");
    seed_store(&store);
    let (audio, _, mut event_rx) = test_service();
    let mut adapter =
        OrangeHostAdapter::with_directories(audio, store, samples, Arc::new(|_| {}), false)
            .unwrap();
    arm_recovery_save(&mut adapter, "recovery");
    assert!(adapter
        .handle_platform_effect(&request(RuntimePlatformEffect::Reboot, "reboot"))
        .unwrap()
        .is_empty());
    HostAdapter::handle_musical_event(
        &mut adapter,
        &MusicalEvent::NoteOn {
            channel: 0,
            note: 60,
            velocity: 100,
            duration_ms: None,
        },
    )
    .unwrap();
    HostAdapter::handle_midi_message(&mut adapter, &[0x90, 60, 100]).unwrap();
    assert!(event_rx.try_recv().is_err());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn shutdown_effect_maps_to_typed_orange_shutdown_request() {
    let (mut adapter, root) = adapter("shutdown-request");
    arm_recovery_save(&mut adapter, "recovery");
    assert!(adapter
        .handle_platform_effect(&request(RuntimePlatformEffect::Shutdown, "shutdown"))
        .unwrap()
        .is_empty());
    assert!(matches!(
        adapter.take_shutdown_request(),
        Some(OrangeShutdownRequest::Shutdown)
    ));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn pending_shutdown_suppresses_a_second_shutdown_request() {
    let (mut adapter, root) = adapter("shutdown-pending");
    arm_recovery_save(&mut adapter, "recovery");
    assert!(adapter
        .handle_platform_effect(&request(RuntimePlatformEffect::Shutdown, "first"))
        .unwrap()
        .is_empty());
    assert!(adapter
        .handle_platform_effect(&request(RuntimePlatformEffect::Shutdown, "second"))
        .unwrap()
        .is_empty());
    assert!(matches!(
        adapter.take_shutdown_request(),
        Some(OrangeShutdownRequest::Shutdown)
    ));
    let _ = std::fs::remove_dir_all(root);
}
