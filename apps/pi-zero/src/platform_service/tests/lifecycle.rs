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
    use playback_runtime::{RuntimePlatformEffect, RuntimeSetupPortalPhase, RuntimeStoreResult};
    use std::time::Duration;

    let root = crate::test_temp_dir::unique_temp_path("octessera-platform-setup");
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
    let mut found_starting = false;
    for _ in 0..200 {
        found_starting |= service.drain_results(64).into_iter().any(|message| {
            matches!(
                message,
                HostMessage::RuntimeResult {
                    result: RuntimeStoreResult::Identified { request_id, revision, result, .. }
                } if request_id == "setup-after-queue"
                    && revision == Some(9)
                    && matches!(result.as_ref(), RuntimeStoreResult::SetupPortalStatus { status }
                        if status.phase == RuntimeSetupPortalPhase::Starting)
            )
        });
        if found_starting {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(found_starting);
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
                    && matches!(result.as_ref(), RuntimeStoreResult::SetupPortalStatus { status }
                        if status.phase == RuntimeSetupPortalPhase::PortalReady)
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
