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
fn a_preset_save_job_refuses_to_overwrite_unless_told_to() {
    let root = crate::test_temp_dir::unique_temp_path("octessera-platform-save-overwrite");
    let store = root.join("store");
    let path = super::super::platform_service_store::preset_patch_path(&store, "taken").unwrap();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, b"{\"kept\": true}").unwrap();
    let update_executor = device_update::production_executor();
    let save = |overwrite| {
        handle_job(
            &store,
            &root,
            PlatformJob::new(
                RuntimePlatformRequest::new(
                    playback_runtime::RuntimePlatformEffect::StoreListPresets,
                    "save".into(),
                    None,
                ),
                PlatformJobKind::SavePreset {
                    name: "taken".into(),
                    payload: serde_json::json!({}),
                    overwrite,
                },
            ),
            update_executor.as_ref(),
        )
    };

    let refused = format!("{:?}", save(false));
    assert!(refused.contains("preset already exists"), "{refused}");
    assert_eq!(std::fs::read(&path).unwrap(), b"{\"kept\": true}");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn system_info_does_not_wait_for_store_lock() {
    let root = crate::test_temp_dir::unique_temp_path("octessera-platform-system-info-lock");
    let service = PiPlatformService::new(root.join("store"), root.join("samples"));
    let store_lock = Arc::clone(&service.store_lock);
    let poison = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _guard = store_lock.lock().unwrap();
        panic!("poison the store lock for the bypass test");
    }));
    assert!(poison.is_err());
    assert!(service.store_lock.is_poisoned());
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
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    let mut found = false;
    while !found && std::time::Instant::now() < deadline {
        found = service.drain_results(4).into_iter().any(|message| {
            matches!(
                message,
                HostMessage::RuntimeResult {
                    result: RuntimeStoreResult::Identified { request_id, result, .. }
                } if request_id == "system-info-unblocked"
                    && matches!(result.as_ref(), RuntimeStoreResult::SystemInfoResult { .. })
            )
        });
        if !found {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
    let _ = std::fs::remove_dir_all(root);
    assert!(
        found,
        "SystemInfo must return typed data with the store lock poisoned"
    );
}

struct HeldUpdate {
    release: Mutex<std::sync::mpsc::Receiver<()>>,
}

impl device_update::UpdateExecutor for HeldUpdate {
    fn output(
        &self,
        _command: &mut std::process::Command,
    ) -> std::io::Result<std::process::Output> {
        let _ = self
            .release
            .lock()
            .unwrap()
            .recv_timeout(std::time::Duration::from_secs(10));
        Err(std::io::Error::other("held update released"))
    }
}

#[test]
fn a_running_update_does_not_hold_back_store_jobs() {
    let root = crate::test_temp_dir::unique_temp_path("octessera-platform-update-lane");
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let service = PiPlatformService::new_with_update_executor(
        root.join("store"),
        root.join("samples"),
        Arc::new(HeldUpdate {
            release: Mutex::new(release_rx),
        }),
    );
    let request = |effect, id: &str| RuntimePlatformRequest::new(effect, id.into(), None);
    service
        .enqueue(PlatformJob::new(
            request(
                playback_runtime::RuntimePlatformEffect::UpdateCheck,
                "held-update",
            ),
            PlatformJobKind::UpdateCheck,
        ))
        .unwrap();
    let (completed_tx, completed_rx) = std::sync::mpsc::sync_channel(1);
    service
        .enqueue(PlatformJob::new(
            request(
                playback_runtime::RuntimePlatformEffect::SystemInfoRequest,
                "after-update",
            ),
            PlatformJobKind::QueueBarrier {
                completed: completed_tx,
            },
        ))
        .unwrap();
    completed_rx
        .recv_timeout(std::time::Duration::from_secs(3))
        .expect("store job waited behind the update");

    release_tx.send(()).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    let mut found = false;
    while !found && std::time::Instant::now() < deadline {
        found = service.drain_results(4).into_iter().any(|message| {
            matches!(
                message,
                HostMessage::RuntimeResult {
                    result: RuntimeStoreResult::Identified { request_id, result, .. }
                } if request_id == "held-update"
                    && matches!(result.as_ref(), RuntimeStoreResult::DeviceUpdateStatus { ok: false, .. })
            )
        });
        if !found {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
    let _ = std::fs::remove_dir_all(root);
    assert!(found, "the update result must still arrive");
}
