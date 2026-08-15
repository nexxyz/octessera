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

fn adapter(label: &str) -> (OrangeHostAdapter, PathBuf) {
    let (root, store, samples) = directories(label);
    let (audio, _, _) = test_service();
    let adapter =
        OrangeHostAdapter::with_directories(audio, store, samples, Arc::new(|_| {}), false)
            .unwrap();
    (adapter, root)
}

#[test]
fn apply_waits_behind_queued_default_and_cancels_deferred_save() {
    let (mut adapter, root) = adapter("fifo");
    let earlier = json!({"earlier": true});
    let deferred = json!({"deferred": true});
    let applied = json!({"applied": true});
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
            RuntimePlatformEffect::StoreSaveDefault {
                payload: deferred,
                mode: Some("deferred".into()),
            },
            "deferred",
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
    assert_eq!(prior, serde_json::to_vec_pretty(&earlier).unwrap());
    assert_eq!(
        std::fs::read(store.join("default.json")).unwrap(),
        serde_json::to_vec_pretty(&applied).unwrap()
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn later_saves_and_applies_are_rejected_while_shutdown_is_pending() {
    let (mut adapter, root) = adapter("reject");
    let applied = json!({"applied": true});
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
        std::fs::read(root.join("store/default.json")).unwrap(),
        serde_json::to_vec_pretty(&applied).unwrap()
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn pending_apply_suppresses_later_musical_output() {
    let (root, store, samples) = directories("silent");
    let (audio, _, mut event_rx) = test_service();
    let mut adapter =
        OrangeHostAdapter::with_directories(audio, store, samples, Arc::new(|_| {}), false)
            .unwrap();
    assert!(adapter
        .handle_platform_effect(&request(
            RuntimePlatformEffect::ApplyDeviceConfigReboot {
                payload: json!({"applied": true}),
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
    let (audio, _, mut event_rx) = test_service();
    let mut adapter =
        OrangeHostAdapter::with_directories(audio, store, samples, Arc::new(|_| {}), false)
            .unwrap();
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
