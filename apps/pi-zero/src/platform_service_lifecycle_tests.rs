use super::*;

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
        request: root.join("request").join("inbox").join("start"),
        current: public.join("current.json"),
        public,
    };
    std::fs::create_dir_all(paths.request.parent().unwrap()).unwrap();
    set_mode(paths.request.parent().unwrap(), 0o700);
    std::fs::create_dir_all(&paths.public).unwrap();
    set_mode(&paths.public, 0o750);
    let service = PiPlatformService::new_with_setup_environment(
        root.join("store"),
        root.join("samples"),
        SetupPortalEnvironment::test(paths.clone(), 0),
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
    assert_eq!(std::fs::read(&paths.request).unwrap(), b"start\n");
    assert!(service.take_transfer_status().is_none());
    std::fs::remove_file(&paths.request).unwrap();
    let starting = serde_json::json!({
        "schema": 1,
        "status": {"type":"setup_portal_status","phase":"starting","disposition":"accepted","rebootRequired":false}
    });
    std::fs::write(&paths.current, serde_json::to_vec(&starting).unwrap()).unwrap();
    set_mode(&paths.current, 0o640);
    std::thread::sleep(Duration::from_millis(40));
    let ready = serde_json::json!({
        "schema": 1,
        "status": {"type":"setup_portal_status","phase":"portal_ready","portalSuffix":"abcd","rebootRequired":false}
    });
    std::fs::write(&paths.current, serde_json::to_vec(&ready).unwrap()).unwrap();
    set_mode(&paths.current, 0o640);
    let mut found = false;
    for _ in 0..200 {
        found |= service.drain_results(64).into_iter().any(|message| {
            matches!(
                message,
                HostMessage::RuntimeResult {
                    result: RuntimeStoreResult::Identified { request_id, revision, result, .. }
                } if request_id == "setup-after-queue"
                    && revision == Some(9)
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

#[cfg(all(feature = "hardware-orange-pi-zero-2w", any(unix, windows)))]
#[test]
fn orange_apply_preserves_mixed_platform_and_setup_fifo() {
    #[cfg(unix)]
    if unsafe { libc::geteuid() } != 0 {
        return;
    }
    use crate::setup_portal::SetupPortalEnvironment;
    use crate::setup_portal_files::SetupPortalPaths;
    use playback_runtime::RuntimePlatformEffect;
    use std::time::Duration;

    let root = std::env::temp_dir().join(format!(
        "octessera-orange-mixed-queue-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let public = root.join("public");
    let paths = SetupPortalPaths {
        request: root.join("request").join("inbox").join("start"),
        current: public.join("current.json"),
        public,
    };
    std::fs::create_dir_all(paths.request.parent().unwrap()).unwrap();
    set_mode(paths.request.parent().unwrap(), 0o700);
    std::fs::create_dir_all(&paths.public).unwrap();
    set_mode(&paths.public, 0o750);
    let service = PiPlatformService::new_with_setup_environment(
        root.join("store"),
        root.join("samples"),
        SetupPortalEnvironment::test(paths.clone(), 0),
    );
    for index in 0..31 {
        enqueue_system_info(&service, index);
    }
    let barrier = service.enqueue_test_barrier().unwrap();
    barrier.recv_timeout(Duration::from_secs(1)).unwrap();
    enqueue_system_info(&service, 31);
    let barrier = service.enqueue_test_barrier().unwrap();
    barrier.recv_timeout(Duration::from_secs(1)).unwrap();

    let request = RuntimePlatformRequest::new(
        RuntimePlatformEffect::SetupPortalOpen,
        "setup-mixed-queue".into(),
        Some(9),
    );
    service.start_setup_portal(&request).unwrap();
    assert_eq!(std::fs::read(&paths.request).unwrap(), b"start\n");
    std::fs::remove_file(&paths.request).unwrap();
    let starting = serde_json::json!({
        "schema": 1,
        "status": {"type":"setup_portal_status","phase":"starting","disposition":"accepted","rebootRequired":false}
    });
    std::fs::write(&paths.current, serde_json::to_vec(&starting).unwrap()).unwrap();
    set_mode(&paths.current, 0o640);
    std::thread::sleep(Duration::from_millis(40));
    let ready = serde_json::json!({
        "schema": 1,
        "status": {"type":"setup_portal_status","phase":"portal_ready","portalSuffix":"abcd","rebootRequired":false}
    });
    std::fs::write(&paths.current, serde_json::to_vec(&ready).unwrap()).unwrap();
    set_mode(&paths.current, 0o640);

    for _ in 0..200 {
        if service.result_lane.setup_send_waiting() {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(service.result_lane.setup_send_waiting());
    enqueue_system_info(&service, 32);

    service
        .prepare_orange_device_apply(&serde_json::json!({"applied": true}))
        .unwrap();

    let results = service.drain_results(64);
    let keys: Vec<_> = results.iter().map(result_request_id).collect();
    let mut expected = (0..32)
        .map(|index| format!("system-info-{index}"))
        .collect::<Vec<_>>();
    expected.push("setup-mixed-queue".into());
    expected.push("system-info-32".into());
    assert_eq!(keys, expected);
    assert!(service.drain_results(64).is_empty());
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
