use super::*;

#[test]
fn list_presets_filters_unsafe_legacy_files() {
    let dir = std::env::temp_dir().join(format!(
        "octessera-pi-preset-list-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("safe.json"), "{}").unwrap();
    std::fs::write(dir.join("default.json"), "{}").unwrap();
    std::fs::write(dir.join("recovery-save.json"), "{}").unwrap();
    std::fs::write(dir.join("bak-123.json"), "{}").unwrap();
    std::fs::write(dir.join("bad:name.json"), "{}").unwrap();
    std::fs::write(dir.join("CON.json"), "{}").unwrap();

    assert_eq!(list_presets(&dir).unwrap(), vec!["safe".to_string()]);

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn preset_patch_files_are_preferred_and_delete_removes_legacy_copy() {
    let dir = std::env::temp_dir().join(format!(
        "octessera-pi-preset-patch-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("Jam.json"), r#"{"legacy":true}"#).unwrap();
    std::fs::create_dir_all(dir.join("patches")).unwrap();
    std::fs::write(dir.join("patches").join("Jam.json"), r#"{"patch":true}"#).unwrap();
    std::fs::write(dir.join("Jam.patch.json"), r#"{"legacy_patch_name":true}"#).unwrap();

    assert_eq!(
        list_presets(&dir).unwrap(),
        vec!["Jam".to_string(), "Jam.patch".to_string()]
    );
    assert_eq!(
        load_json(&preset_load_path(&dir, "Jam").unwrap()).unwrap(),
        Some(serde_json::json!({ "patch": true }))
    );
    save_json(
        &preset_patch_path(&dir, "New").unwrap(),
        &serde_json::json!({ "kind": "octessera.patch" }),
    )
    .unwrap();
    assert!(dir.join("patches").join("New.json").is_file());
    assert!(!dir.join("New.json").is_file());
    assert!(delete_preset_payload(&dir, "Jam"));
    assert!(!dir.join("Jam.json").exists());
    assert!(!dir.join("patches").join("Jam.json").exists());
    assert!(dir.join("Jam.patch.json").exists());

    let _ = std::fs::remove_dir_all(dir);
}

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
    #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
    let result = {
        let update_executor = device_update::production_executor();
        handle_job(
            Path::new("."),
            Path::new("."),
            job,
            update_executor.as_ref(),
        )
    };
    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    let result = handle_job(Path::new("."), Path::new("."), job);
    assert!(matches!(
        result,
        RuntimeStoreResult::Identified { result, request_id, revision }
            if request_id == "system-info-test"
                && revision == Some(7)
                && matches!(*result, RuntimeStoreResult::SystemInfoResult { .. })
    ));
}

#[cfg(any(unix, windows))]
#[test]
fn setup_portal_survives_shared_queue_saturation_and_publishes_status() {
    #[cfg(unix)]
    if unsafe { libc::geteuid() } != 0 {
        return;
    }
    use crate::setup_portal::SetupPortalEnvironment;
    use crate::setup_portal_files::SetupPortalPaths;
    use playback_runtime::{RuntimePlatformEffect, RuntimeStoreResult};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Duration;

    let root = std::env::temp_dir().join(format!(
        "octessera-platform-setup-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let public = root.join("public");
    let paths = SetupPortalPaths {
        request: root.join("request").join("setup-portal.request"),
        receipts: public.join("receipts"),
        current: public.join("current.json"),
        public,
        boot_id: root.join("boot-id"),
    };
    std::fs::create_dir_all(paths.request.parent().unwrap()).unwrap();
    std::fs::create_dir_all(&paths.receipts).unwrap();
    set_mode(&paths.public, 0o750);
    set_mode(&paths.receipts, 0o750);
    let clock = Arc::new(AtomicU64::new(1));
    let service = PiPlatformService::new_with_setup_environment(
        root.join("store"),
        root.join("samples"),
        SetupPortalEnvironment::test(
            paths.clone(),
            0,
            Arc::new(move || clock.load(Ordering::SeqCst)),
            Arc::new(|bytes| {
                bytes.fill(3);
                Ok(())
            }),
            Arc::new(|| Ok("01234567-89ab-cdef-0123-456789abcdef".into())),
        ),
    );
    for index in 0..64 {
        let _ = service.enqueue(PlatformJob::new(
            RuntimePlatformRequest::new(
                RuntimePlatformEffect::StoreListPresets,
                format!("queue-{index}"),
                None,
            ),
            PlatformJobKind::ListPresets,
        ));
    }
    let request = RuntimePlatformRequest::new(
        RuntimePlatformEffect::SetupPortalOpen,
        "setup-after-queue".into(),
        Some(9),
    );
    service.start_setup_portal(&request).unwrap();
    let token = std::fs::read_to_string(&paths.request)
        .unwrap()
        .trim()
        .to_string();
    std::fs::remove_file(&paths.request).unwrap();
    let receipt = serde_json::json!({
        "schema": 1,
        "bootId": "01234567-89ab-cdef-0123-456789abcdef",
        "attemptId": "cccccccccccccccccccccccccccccccc",
        "sequence": 1,
        "status": {"type":"setup_portal_status","phase":"starting","disposition":"accepted","rebootRequired":false}
    });
    let receipt_path = paths.receipts.join(format!("{token}.json"));
    std::fs::write(&receipt_path, serde_json::to_vec(&receipt).unwrap()).unwrap();
    set_mode(&receipt_path, 0o640);
    let mut found = false;
    for _ in 0..200 {
        found |= service.drain_results(64).into_iter().any(|message| {
            matches!(
                message,
                HostMessage::RuntimeResult {
                    result: RuntimeStoreResult::Identified { request_id, result, .. }
                } if request_id == "setup-after-queue"
                    && matches!(*result, RuntimeStoreResult::SetupPortalStatus { .. })
            )
        });
        if found {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(found);
    let _ = std::fs::remove_dir_all(root);
}

fn set_mode(path: &Path, mode: u32) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
    }
    #[cfg(windows)]
    {
        let _ = (path, mode);
    }
}

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
#[test]
fn usb_storage_start_reports_waiting_until_host_configures_gadget() {
    assert_eq!(
        usb_storage_message("storage-start", "HOST_STATE=not attached\n"),
        "USB SD2 transfer waiting for host"
    );
    assert_eq!(
        usb_storage_message("storage-start", "HOST_STATE=configured\n"),
        "USB SD2 transfer active"
    );
}
