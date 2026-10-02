use super::*;
use crate::usb_config::UsbAudioOut;
use playback_runtime::{
    HostAdapter, RuntimePlatformEffect, RuntimePlatformRequest, RuntimeStoreResult,
};
use serde_json::json;
use std::path::PathBuf;
use std::time::{Duration, Instant};

fn adapter(label: &str) -> (PiPlaybackHostAdapter, PathBuf) {
    let root = std::env::temp_dir().join(format!(
        "octessera-pi-system-store-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let adapter = PiPlaybackHostAdapter::new(
        None,
        root.join("store"),
        root.join("samples"),
        Arc::new(|_| {}),
        false,
        UsbAudioOut::Jack,
    );
    (adapter, root)
}

fn request(effect: RuntimePlatformEffect) -> RuntimePlatformRequest {
    RuntimePlatformRequest::new(effect, "system-test".into(), Some(1))
}

#[test]
fn system_store_is_separate_and_does_not_cancel_queued_default_write() {
    let (mut adapter, root) = adapter("separate");
    let store = root.join("store");
    std::fs::create_dir_all(&store).unwrap();
    let mut prior_full = crate::user_data_archive::canonical_defaults();
    prior_full["runtimeConfig"]["bpm"] = json!(100);
    let prior_documents = playback_runtime::split_system_patch_documents(&prior_full).unwrap();
    std::fs::write(
        store.join("system.json"),
        serde_json::to_vec(&prior_documents.system).unwrap(),
    )
    .unwrap();
    std::fs::write(
        store.join("default.patch.json"),
        serde_json::to_vec(&prior_documents.patch).unwrap(),
    )
    .unwrap();
    let mut system = prior_documents.system.clone();
    system["runtimeConfig"]["screenSleepSeconds"] = json!(45);
    let mut next_full = prior_full;
    next_full["runtimeConfig"]["bpm"] = json!(140);
    let default_payload = playback_runtime::split_system_patch_documents(&next_full)
        .unwrap()
        .patch;

    assert!(adapter
        .handle_platform_effect(&request(RuntimePlatformEffect::StoreSaveSystem {
            payload: system.clone(),
        }))
        .unwrap()
        .is_empty());
    adapter
        .core
        .platform_service
        .enqueue_test_barrier()
        .unwrap()
        .recv_timeout(Duration::from_secs(1))
        .unwrap();
    let completions = adapter.core.platform_service.drain_results(4);
    assert!(matches!(
        completions.as_slice(),
        [HostMessage::RuntimeResult {
            result: RuntimeStoreResult::Identified { request_id, result, revision: Some(1) }
        }] if request_id == "system-test"
            && matches!(result.as_ref(), RuntimeStoreResult::SaveSystemResult { ok: true })
    ));
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(
            &std::fs::read(store.join("system.json")).unwrap()
        )
        .unwrap(),
        system
    );
    assert_eq!(
        crate::platform_service::load_json(&store.join("default.patch.json")).unwrap(),
        Some(prior_documents.patch)
    );

    assert!(adapter
        .handle_platform_effect(&request(RuntimePlatformEffect::StoreSaveDefault {
            payload: default_payload.clone(),
            mode: None,
        }))
        .unwrap()
        .is_empty());
    let loaded = adapter
        .handle_platform_effect(&request(RuntimePlatformEffect::StoreLoadSystem))
        .unwrap();
    assert!(matches!(
        loaded.as_slice(),
        [HostMessage::RuntimeResult { result: RuntimeStoreResult::LoadSystemResult { payload: Some(value) } }] if value == &system
    ));
    assert!(adapter.power_request.is_none());

    adapter
        .core
        .platform_service
        .enqueue_test_barrier()
        .unwrap()
        .recv_timeout(Duration::from_secs(1))
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut saved_default = None;
    while Instant::now() < deadline {
        if let Ok(bytes) = std::fs::read(store.join("default.patch.json")) {
            if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                if value == default_payload {
                    saved_default = Some(value);
                    break;
                }
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(saved_default, Some(default_payload));
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(
            &std::fs::read(store.join("system.json")).unwrap()
        )
        .unwrap(),
        system
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn system_save_is_rejected_while_restore_blocks_store_writes() {
    let (mut adapter, root) = adapter("blocked");
    let path = root.join("store/system.json");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let prior = br#"{"before":true}"#;
    std::fs::write(&path, prior).unwrap();
    adapter
        .core
        .platform_service
        .invalidate_store_writes_for_test();

    let result = adapter
        .handle_platform_effect(&request(RuntimePlatformEffect::StoreSaveSystem {
            payload: json!({"after": true}),
        }))
        .unwrap();

    assert!(matches!(
        result.as_slice(),
        [HostMessage::RuntimeResult {
            result: RuntimeStoreResult::RuntimeFailure { .. }
        }]
    ));
    assert_eq!(std::fs::read(path).unwrap(), prior);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn system_save_reports_identified_queue_full_failure() {
    let (mut adapter, root) = adapter("queue-full");
    let (entered_tx, entered_rx) = std::sync::mpsc::sync_channel(1);
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    adapter
        .core
        .platform_service
        .enqueue(crate::platform_service::PlatformJob::new(
            request(RuntimePlatformEffect::SystemInfoRequest),
            crate::platform_service::PlatformJobKind::TestGate {
                entered: entered_tx,
                release: release_rx,
            },
        ))
        .unwrap();
    entered_rx.recv_timeout(Duration::from_secs(1)).unwrap();
    for index in 0..32 {
        adapter
            .core
            .platform_service
            .enqueue(crate::platform_service::PlatformJob::new(
                RuntimePlatformRequest::new(
                    RuntimePlatformEffect::SystemInfoRequest,
                    format!("fill-{index}"),
                    None,
                ),
                crate::platform_service::PlatformJobKind::SystemInfo,
            ))
            .unwrap();
    }
    let failed = adapter
        .handle_platform_effect(&RuntimePlatformRequest::new(
            RuntimePlatformEffect::StoreSaveSystem {
                payload: serde_json::json!({"after": true}),
            },
            "queue-full-system".into(),
            Some(4),
        ))
        .unwrap();
    assert!(matches!(
        failed.as_slice(),
        [HostMessage::RuntimeResult {
            result: RuntimeStoreResult::RuntimeFailure { error }
        }] if error.operation == playback_runtime::RuntimeOperation::StoreSaveSystem
            && error.request_id.as_deref() == Some("queue-full-system")
    ));
    release_tx.send(()).unwrap();
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn missing_and_corrupt_system_store_are_not_replaced_with_default() {
    let (mut adapter, root) = adapter("errors");
    let store = root.join("store");
    std::fs::create_dir_all(&store).unwrap();
    std::fs::write(store.join("default.json"), br#"{"legacy":true}"#).unwrap();

    let missing = adapter
        .handle_platform_effect(&request(RuntimePlatformEffect::StoreLoadSystem))
        .unwrap();
    assert!(matches!(
        missing.as_slice(),
        [HostMessage::RuntimeResult {
            result: RuntimeStoreResult::LoadSystemResult { payload: None }
        }]
    ));
    std::fs::write(store.join("system.json"), b"{").unwrap();
    let corrupt = adapter
        .handle_platform_effect(&request(RuntimePlatformEffect::StoreLoadSystem))
        .unwrap();
    assert!(matches!(
        corrupt.as_slice(),
        [HostMessage::RuntimeResult {
            result: RuntimeStoreResult::RuntimeFailure { .. }
        }]
    ));
    assert!(adapter.power_request.is_none());
    let _ = std::fs::remove_dir_all(root);
}
