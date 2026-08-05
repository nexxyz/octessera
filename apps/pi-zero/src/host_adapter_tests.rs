use super::*;

#[test]
fn preset_path_rejects_unsafe_names() {
    let adapter = PiPlaybackHostAdapter::new(
        None,
        PathBuf::from("store"),
        PathBuf::from("samples"),
        Arc::new(|_| {}),
        false,
        UsbAudioOut::Jack,
    );
    assert!(crate::platform_service::preset_path(&adapter.store_dir, "safe").is_ok());
    for name in ["bad/name", r"bad\name", r"C:\x", "CON", "bad:name"] {
        assert!(
            crate::platform_service::preset_path(&adapter.store_dir, name).is_err(),
            "{name:?}"
        );
    }
}

#[cfg(any(unix, windows))]
#[test]
fn raspberry_adapter_supports_setup_portal_effect() {
    #[cfg(unix)]
    if unsafe { libc::geteuid() } != 0 {
        return;
    }
    use crate::setup_portal::SetupPortalEnvironment;
    use crate::setup_portal_files::SetupPortalPaths;
    use playback_runtime::{RuntimePlatformEffect, RuntimePlatformRequest, RuntimeStoreResult};
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    let root =
        std::env::temp_dir().join(format!("octessera-pi-setup-adapter-{}", std::process::id()));
    let public = root.join("public");
    let paths = SetupPortalPaths {
        request: root.join("request").join("setup-portal.request"),
        receipts: public.join("receipts"),
        current: public.join("current.json"),
        public,
        boot_id: root.join("boot-id"),
    };
    fs::create_dir_all(paths.request.parent().unwrap()).unwrap();
    fs::create_dir_all(&paths.receipts).unwrap();
    fs::set_permissions(&paths.public, permissions(0o750)).unwrap();
    fs::set_permissions(&paths.receipts, permissions(0o750)).unwrap();
    let clock = Arc::new(AtomicU64::new(1));
    let environment = SetupPortalEnvironment::test(
        paths.clone(),
        0,
        Arc::new(move || clock.load(Ordering::SeqCst)),
        Arc::new(|bytes| {
            bytes.fill(1);
            Ok(())
        }),
        Arc::new(|| Ok("01234567-89ab-cdef-0123-456789abcdef".into())),
    );
    let mut adapter = PiPlaybackHostAdapter::new_with_setup_environment(
        None,
        root.join("store"),
        root.join("samples"),
        Arc::new(|_| {}),
        false,
        UsbAudioOut::Jack,
        environment,
    );
    let request = RuntimePlatformRequest::new(
        RuntimePlatformEffect::SetupPortalOpen,
        "pi-setup".into(),
        Some(2),
    );
    assert!(adapter.handle_platform_effect(&request).unwrap().is_empty());
    let token = fs::read_to_string(&paths.request)
        .unwrap()
        .trim()
        .to_string();
    fs::remove_file(&paths.request).unwrap();
    let status = serde_json::json!({"type":"setup_portal_status","phase":"starting","disposition":"accepted","rebootRequired":false});
    let payload = serde_json::json!({"schema":1,"bootId":"01234567-89ab-cdef-0123-456789abcdef","attemptId":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","sequence":1,"status":status});
    fs::write(
        paths.receipts.join(format!("{token}.json")),
        serde_json::to_vec(&payload).unwrap(),
    )
    .unwrap();
    fs::set_permissions(
        paths.receipts.join(format!("{token}.json")),
        permissions(0o640),
    )
    .unwrap();
    let mut responses = Vec::new();
    for _ in 0..100 {
        responses = adapter.drain_platform_results(4);
        if !responses.is_empty() {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(matches!(
        responses.as_slice(),
        [HostMessage::RuntimeResult { result: RuntimeStoreResult::Identified { request_id, revision: Some(1), .. } }] if request_id == "pi-setup"
    ));
    let _ = fs::remove_dir_all(root);
}

#[cfg(any(unix, windows))]
fn permissions(mode: u32) -> std::fs::Permissions {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        return std::fs::Permissions::from_mode(mode);
    }
    #[cfg(windows)]
    {
        let _ = mode;
        std::fs::metadata(".").unwrap().permissions()
    }
}
