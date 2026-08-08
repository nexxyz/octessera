use super::*;
use playback_runtime::{
    HostAdapter, RuntimePlatformEffect, RuntimePlatformRequest, RuntimeStoreResult,
};
use serde_json::json;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

static NEXT_ROOT: AtomicU64 = AtomicU64::new(0);

fn adapter() -> (PiPlaybackHostAdapter, PathBuf) {
    let root = std::env::temp_dir().join(format!(
        "octessera-pi-deferred-save-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        NEXT_ROOT.fetch_add(1, Ordering::Relaxed)
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

fn request(effect: RuntimePlatformEffect, id: &str, revision: u64) -> RuntimePlatformRequest {
    RuntimePlatformRequest::new(effect, id.into(), Some(revision))
}

fn deferred(id: &str, revision: u64, payload: serde_json::Value) -> RuntimePlatformRequest {
    request(
        RuntimePlatformEffect::StoreSaveDefault {
            payload,
            mode: Some("deferred".into()),
        },
        id,
        revision,
    )
}

fn cleanup(root: PathBuf) {
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn newer_deferred_save_replaces_payload_and_request() {
    let (mut adapter, root) = adapter();
    let first = json!({"version": 1});
    let second = json!({"version": 2});
    assert!(adapter
        .handle_platform_effect(&deferred("old", 3, first))
        .unwrap()
        .is_empty());
    assert!(adapter
        .handle_platform_effect(&deferred("new", 7, second.clone()))
        .unwrap()
        .is_empty());
    let entry = adapter.pending_default_save.take_now().unwrap();
    assert_eq!(entry.payload, second);
    assert_eq!(entry.request.request_id, "new");
    assert_eq!(entry.request.revision, Some(7));
    assert!(adapter.pending_default_save.take_now().is_none());
    cleanup(root);
}

#[test]
fn load_immediate_save_and_usb_apply_cancel_deferred_work() {
    let (mut adapter, root) = adapter();
    let deferred_payload = json!({"deferred": true});
    let immediate_payload = json!({"immediate": true});

    assert!(adapter
        .handle_platform_effect(&deferred("load", 1, deferred_payload.clone()))
        .unwrap()
        .is_empty());
    assert!(matches!(
        adapter
            .handle_platform_effect(&request(
                RuntimePlatformEffect::StoreLoadDefault,
                "load-now",
                2
            ))
            .unwrap()
            .as_slice(),
        [HostMessage::RuntimeResult {
            result: RuntimeStoreResult::LoadDefaultResult { .. }
        }]
    ));
    assert!(!adapter.pending_default_save.is_pending());

    assert!(adapter
        .handle_platform_effect(&deferred("immediate", 3, deferred_payload.clone()))
        .unwrap()
        .is_empty());
    assert!(adapter
        .handle_platform_effect(&request(
            RuntimePlatformEffect::StoreSaveDefault {
                payload: immediate_payload,
                mode: None
            },
            "immediate-now",
            4,
        ))
        .unwrap()
        .is_empty());
    assert!(!adapter.pending_default_save.is_pending());
    let barrier = adapter.platform_service.enqueue_test_barrier().unwrap();
    barrier.recv_timeout(Duration::from_secs(1)).unwrap();

    assert!(adapter
        .handle_platform_effect(&deferred("usb", 5, deferred_payload))
        .unwrap()
        .is_empty());
    assert!(adapter
        .handle_platform_effect(&request(
            RuntimePlatformEffect::UsbApplyReboot {
                payload: json!({"usb": true})
            },
            "usb-now",
            6,
        ))
        .unwrap()
        .is_empty());
    assert!(!adapter.pending_default_save.is_pending());
    cleanup(root);
}

#[test]
fn full_platform_queue_retains_deferred_entry_and_delays_retry() {
    let (mut adapter, root) = adapter();
    let (entered, release) = adapter.platform_service.enqueue_test_gate().unwrap();
    entered.recv_timeout(Duration::from_secs(1)).unwrap();
    let payload = json!({"version": 11});
    let deferred_request = deferred("full", 9, payload.clone());
    assert!(adapter
        .handle_platform_effect(&deferred_request)
        .unwrap()
        .is_empty());
    for index in 0..64 {
        if adapter
            .platform_service
            .enqueue(crate::platform_service::PlatformJob::new(
                request(
                    RuntimePlatformEffect::SystemInfoRequest,
                    &format!("fill-{index}"),
                    1,
                ),
                crate::platform_service::PlatformJobKind::SystemInfo,
            ))
            .is_err()
        {
            break;
        }
    }
    let before = Instant::now();
    let _ = adapter.pending_default_save.take_now();
    adapter
        .pending_default_save
        .schedule(payload, before, deferred_request);
    assert!(adapter
        .flush_due_default_save()
        .unwrap()
        .iter()
        .any(|message| matches!(
            message,
            HostMessage::RuntimeResult {
                result: RuntimeStoreResult::RuntimeFailure { .. }
            }
        )));
    let retry = adapter.pending_default_save.take_now().unwrap();
    assert!(retry.due_at > before);
    assert_eq!(retry.payload, json!({"version": 11}));
    assert_eq!(retry.request.request_id, "full");
    drop(release);
    cleanup(root);
}

#[test]
fn disconnected_platform_queue_retains_deferred_entry_and_delays_retry() {
    let (mut adapter, root) = adapter();
    adapter.platform_service.disconnect_results_for_test();
    let payload = json!({"version": 12});
    let deferred_request = deferred("disconnected", 10, payload.clone());
    assert!(adapter
        .handle_platform_effect(&deferred_request)
        .unwrap()
        .is_empty());
    adapter
        .platform_service
        .enqueue(crate::platform_service::PlatformJob::new(
            request(RuntimePlatformEffect::SystemInfoRequest, "wake", 1),
            crate::platform_service::PlatformJobKind::SystemInfo,
        ))
        .unwrap();
    if let Ok(barrier) = adapter.platform_service.enqueue_test_barrier() {
        assert!(barrier.recv_timeout(Duration::from_secs(1)).is_err());
    }
    let before = Instant::now();
    let entry = adapter.pending_default_save.take_now().unwrap();
    adapter
        .pending_default_save
        .schedule(entry.payload, before, entry.request);
    assert!(adapter
        .flush_due_default_save()
        .unwrap()
        .iter()
        .any(|message| matches!(
            message,
            HostMessage::RuntimeResult {
                result: RuntimeStoreResult::RuntimeFailure { .. }
            }
        )));
    let retry = adapter.pending_default_save.take_now().unwrap();
    assert!(retry.due_at > before);
    assert_eq!(retry.payload, json!({"version": 12}));
    assert_eq!(retry.request.request_id, "disconnected");
    cleanup(root);
}

#[test]
fn successful_deferred_save_has_original_identity_and_auto_flag() {
    let (mut adapter, root) = adapter();
    let payload = json!({"version": 13});
    let deferred_request = deferred("success", 11, payload.clone());
    assert!(adapter
        .handle_platform_effect(&deferred_request)
        .unwrap()
        .is_empty());
    let entry = adapter.pending_default_save.take_now().unwrap();
    adapter
        .pending_default_save
        .schedule(entry.payload, Instant::now(), entry.request);
    assert!(adapter.flush_due_default_save().unwrap().is_empty());
    let barrier = adapter.platform_service.enqueue_test_barrier().unwrap();
    barrier.recv_timeout(Duration::from_secs(1)).unwrap();
    let results = adapter.drain_platform_results(4);
    assert!(
        matches!(results.as_slice(), [HostMessage::RuntimeResult { result: RuntimeStoreResult::Identified { request_id, revision: Some(11), result } }] if request_id == "success" && matches!(result.as_ref(), RuntimeStoreResult::SaveDefaultResult { ok: true, is_auto: Some(true) }))
    );
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(
            &std::fs::read_to_string(root.join("store/default.json")).unwrap()
        )
        .unwrap(),
        payload
    );
    cleanup(root);
}
