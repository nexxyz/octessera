use super::*;

#[test]
fn system_info_job_returns_identified_typed_result() {
    let job = PlatformJob::new(
        RuntimePlatformRequest::new(
            playback_runtime::RuntimePlatformEffect::SystemInfoRequest,
            "system-info-test".into(),
            Some(7),
        ),
        PlatformJobKind::SystemInfo,
    );
    let update_executor = device_update::production_executor();
    let result = handle_job(
        Path::new("."),
        Path::new("."),
        job,
        update_executor.as_ref(),
    );
    assert!(matches!(
        result,
        RuntimeStoreResult::Identified { result, request_id, revision }
            if request_id == "system-info-test"
                && revision == Some(7)
                && matches!(*result, RuntimeStoreResult::SystemInfoResult { .. })
    ));
}

#[test]
fn system_info_does_not_wait_for_store_lock() {
    let root = std::env::temp_dir().join(format!(
        "octessera-platform-system-info-lock-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let service = PiPlatformService::new(root.join("store"), root.join("samples"));
    let store_guard = service.store_lock.lock().unwrap();
    service
        .enqueue(PlatformJob::new(
            RuntimePlatformRequest::new(
                playback_runtime::RuntimePlatformEffect::SystemInfoRequest,
                "system-info-unblocked".into(),
                None,
            ),
            PlatformJobKind::SystemInfo,
        ))
        .unwrap();
    let mut found = false;
    for _ in 0..100 {
        found |= service.drain_results(4).into_iter().any(|message| {
            matches!(
                message,
                HostMessage::RuntimeResult {
                    result: RuntimeStoreResult::Identified { request_id, result, .. }
                } if request_id == "system-info-unblocked"
                    && matches!(
                        result.as_ref(),
                        RuntimeStoreResult::SystemInfoResult { .. }
                            | RuntimeStoreResult::SystemInfoError { .. }
                    )
            )
        });
        if found {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    drop(store_guard);
    assert!(found);
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
#[test]
fn orange_apply_drains_saturated_results_and_preserves_fifo_order() {
    use std::time::Duration;

    let root = std::env::temp_dir().join(format!(
        "octessera-orange-apply-queue-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let service = PiPlatformService::new(root.join("store"), root.join("samples"));
    for index in 0..31 {
        enqueue_system_info(&service, index);
    }
    let barrier = service.enqueue_test_barrier().unwrap();
    barrier.recv_timeout(Duration::from_secs(1)).unwrap();
    enqueue_system_info(&service, 31);
    enqueue_system_info(&service, 32);

    service
        .prepare_orange_device_apply(&serde_json::json!({"applied": true}))
        .unwrap();

    assert!(!service.preserved_results.lock().unwrap().is_empty());
    let results = service.drain_results(64);
    let ids: Vec<_> = results.iter().map(result_request_id).collect();
    assert_eq!(
        ids,
        (0..33)
            .map(|index| format!("system-info-{index}"))
            .collect::<Vec<_>>()
    );
    assert!(service.drain_results(64).is_empty());
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
fn enqueue_system_info(service: &PiPlatformService, index: usize) {
    service
        .enqueue(PlatformJob::new(
            RuntimePlatformRequest::new(
                playback_runtime::RuntimePlatformEffect::SystemInfoRequest,
                format!("system-info-{index}"),
                None,
            ),
            PlatformJobKind::SystemInfo,
        ))
        .unwrap();
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
fn result_request_id(message: &HostMessage) -> String {
    let HostMessage::RuntimeResult {
        result: RuntimeStoreResult::Identified { request_id, .. },
    } = message
    else {
        panic!("expected identified platform result");
    };
    request_id.clone()
}
