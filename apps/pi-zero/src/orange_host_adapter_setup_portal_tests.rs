use super::*;
use crate::setup_portal::SetupPortalEnvironment;
use crate::setup_portal_files::SetupPortalPaths;
use playback_runtime::RuntimeSetupPortalPhase;
use serde_json::json;
use std::fs;
use std::time::Duration;

#[cfg(any(unix, windows))]
#[test]
fn orange_adapter_supports_setup_portal_effect() {
    #[cfg(unix)]
    let status_group = unsafe { libc::getegid() };
    #[cfg(windows)]
    let status_group = 0;

    let root = std::env::temp_dir().join(format!(
        "octessera-orange-setup-adapter-{}",
        std::process::id()
    ));
    let public = root.join("public");
    let paths = SetupPortalPaths {
        request: root.join("request").join("inbox").join("start"),
        current: public.join("current.json"),
        public,
    };
    fs::create_dir_all(paths.request.parent().unwrap()).unwrap();
    fs::set_permissions(paths.request.parent().unwrap(), permissions(0o700)).unwrap();
    fs::create_dir_all(&paths.public).unwrap();
    fs::set_permissions(&paths.public, permissions(0o750)).unwrap();
    let environment = SetupPortalEnvironment::test(paths.clone(), status_group);
    let (audio, _, _) = test_service();
    let mut adapter = OrangeHostAdapter::with_setup_environment(
        audio,
        root.join("store"),
        root.join("samples"),
        std::sync::Arc::new(|_| {}),
        false,
        environment,
    )
    .unwrap();
    let request = RuntimePlatformRequest::new(
        RuntimePlatformEffect::SetupPortalOpen,
        "orange-setup".into(),
        Some(3),
    );
    let started = adapter.handle_platform_effect(&request).unwrap();
    let HostMessage::RuntimeResult {
        result:
            RuntimeStoreResult::Identified {
                result,
                request_id,
                revision,
            },
    } = &started[0]
    else {
        panic!("starting setup portal status");
    };
    assert_eq!(request_id, "orange-setup");
    assert_eq!(*revision, Some(3));
    let RuntimeStoreResult::SetupPortalStatus { status } = result.as_ref() else {
        panic!("starting setup portal result");
    };
    assert_eq!(status.phase, RuntimeSetupPortalPhase::Starting);
    assert_eq!(fs::read(&paths.request).unwrap(), b"start\n");
    fs::remove_file(&paths.request).unwrap();
    let payload = json!({
        "schema": 1,
        "status": {"type":"setup_portal_status","phase":"starting","disposition":"accepted","rebootRequired":false}
    });
    fs::write(&paths.current, serde_json::to_vec(&payload).unwrap()).unwrap();
    fs::set_permissions(&paths.current, permissions(0o640)).unwrap();
    let mut responses = Vec::new();
    for _ in 0..100 {
        responses = adapter.drain_results(4);
        if !responses.is_empty() {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(matches!(
        responses.as_slice(),
        [HostMessage::RuntimeResult {
            result: RuntimeStoreResult::Identified { request_id, revision: Some(3), result, .. }
        }] if request_id == "orange-setup"
            && matches!(result.as_ref(), RuntimeStoreResult::SetupPortalStatus { status } if status.phase == RuntimeSetupPortalPhase::Starting)
    ));

    let current = json!({
        "schema": 1,
        "status": {"type":"setup_portal_status","phase":"portal_ready","portalSuffix":"cafe","rebootRequired":false}
    });
    fs::write(&paths.current, serde_json::to_vec(&current).unwrap()).unwrap();
    fs::set_permissions(&paths.current, permissions(0o640)).unwrap();
    responses.clear();
    for _ in 0..100 {
        responses = adapter.drain_results(4);
        if !responses.is_empty() {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(matches!(
        responses.as_slice(),
        [HostMessage::RuntimeResult {
            result: RuntimeStoreResult::Identified { request_id, revision: Some(3), result, .. }
        }] if request_id == "orange-setup"
            && matches!(result.as_ref(), RuntimeStoreResult::SetupPortalStatus { status } if status.phase == RuntimeSetupPortalPhase::PortalReady && status.portal_suffix.as_deref() == Some("cafe"))
    ));

    let succeeded = json!({
        "schema": 1,
        "status": {"type":"setup_portal_status","phase":"succeeded","rebootRequired":false}
    });
    fs::write(&paths.current, serde_json::to_vec(&succeeded).unwrap()).unwrap();
    fs::set_permissions(&paths.current, permissions(0o640)).unwrap();
    responses.clear();
    for _ in 0..100 {
        responses = adapter.drain_results(4);
        if !responses.is_empty() {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(matches!(
        responses.as_slice(),
        [HostMessage::RuntimeResult {
            result: RuntimeStoreResult::Identified { request_id, revision: Some(3), result, .. }
        }] if request_id == "orange-setup"
            && matches!(result.as_ref(), RuntimeStoreResult::SetupPortalStatus { status } if status.phase == RuntimeSetupPortalPhase::Succeeded && !status.reboot_required)
    ));
    let _ = fs::remove_dir_all(root);
}

#[cfg(any(unix, windows))]
fn permissions(mode: u32) -> std::fs::Permissions {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::Permissions::from_mode(mode)
    }
    #[cfg(windows)]
    {
        let _ = mode;
        std::fs::metadata(".").unwrap().permissions()
    }
}
