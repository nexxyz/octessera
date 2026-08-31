use super::*;
use crate::usb_config::UsbAudioOut;
use playback_runtime::{
    HostMessage, RuntimePlatformEffect, RuntimePlatformRequest, RuntimeSetupPortalPhase,
    RuntimeStoreResult,
};

fn assert_sd2_store_error(response: &[HostMessage], message: &str) {
    let [HostMessage::RuntimeResult {
        result: RuntimeStoreResult::StoreError { message: actual },
    }] = response
    else {
        panic!("expected one SD2 gate failure");
    };
    assert_eq!(actual, message);
}

#[test]
fn raspberry_sd2_start_rejects_active_usb_audio() {
    let root =
        std::env::temp_dir().join(format!("octessera-pi-sd2-usb-audio-{}", std::process::id()));
    let mut adapter = PiPlaybackHostAdapter::new(
        None,
        root.join("store"),
        root.join("samples"),
        Arc::new(|_| {}),
        false,
        UsbAudioOut::Usb,
    );
    let request = RuntimePlatformRequest::new(
        RuntimePlatformEffect::UsbSdTransferStart,
        "sd2-usb-audio".into(),
        None,
    );
    let response = adapter.handle_platform_effect(&request).unwrap();
    assert_sd2_store_error(
        &response,
        "USB SD2 transfer blocked while USB audio out is active",
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn raspberry_sd2_start_rejects_enabled_usb_midi() {
    let root =
        std::env::temp_dir().join(format!("octessera-pi-sd2-usb-midi-{}", std::process::id()));
    let mut adapter = PiPlaybackHostAdapter::new(
        None,
        root.join("store"),
        root.join("samples"),
        Arc::new(|_| {}),
        true,
        UsbAudioOut::Jack,
    );
    let request = RuntimePlatformRequest::new(
        RuntimePlatformEffect::UsbSdTransferStart,
        "sd2-usb-midi".into(),
        None,
    );
    let response = adapter.handle_platform_effect(&request).unwrap();
    assert_sd2_store_error(
        &response,
        "USB SD2 transfer blocked while USB MIDI out is enabled",
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn raspberry_sd2_start_rejects_active_recording() {
    let root =
        std::env::temp_dir().join(format!("octessera-pi-sd2-recording-{}", std::process::id()));
    let audio = crate::audio::test_service_with_prep_worker();
    audio.start_recording(1).unwrap();
    let mut adapter = PiPlaybackHostAdapter::new(
        Some(audio.clone()),
        root.join("store"),
        root.join("samples"),
        Arc::new(|_| {}),
        false,
        UsbAudioOut::Jack,
    );
    let request = RuntimePlatformRequest::new(
        RuntimePlatformEffect::UsbSdTransferStart,
        "sd2-recording".into(),
        None,
    );
    let response = adapter.handle_platform_effect(&request).unwrap();
    assert_sd2_store_error(
        &response,
        "USB SD2 transfer blocked while recording is active",
    );
    assert!(audio.is_recording().unwrap());
    audio.stop_recording().unwrap();
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn preset_path_rejects_unsafe_names() {
    let store_dir = PathBuf::from("store");
    let _adapter = PiPlaybackHostAdapter::new(
        None,
        PathBuf::from("store"),
        PathBuf::from("samples"),
        Arc::new(|_| {}),
        false,
        UsbAudioOut::Jack,
    );
    assert!(crate::platform_service::preset_path(&store_dir, "safe").is_ok());
    for name in ["bad/name", r"bad\name", r"C:\x", "CON", "bad:name"] {
        assert!(
            crate::platform_service::preset_path(&store_dir, name).is_err(),
            "{name:?}"
        );
    }
}

#[test]
fn raspberry_power_request_requires_recovery_save_before_acceptance() {
    let root = std::env::temp_dir().join(format!(
        "octessera-pi-power-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::create_dir_all(root.join("store")).unwrap();
    std::fs::create_dir_all(root.join("samples")).unwrap();
    let mut adapter = PiPlaybackHostAdapter::new(
        None,
        root.join("store"),
        root.join("samples"),
        Arc::new(|_| {}),
        false,
        UsbAudioOut::Jack,
    );
    let reboot = RuntimePlatformRequest::new(RuntimePlatformEffect::Reboot, "reboot".into(), None);
    assert!(!adapter.handle_platform_effect(&reboot).unwrap().is_empty());
    assert!(adapter.take_power_request().is_none());

    let recovery = RuntimePlatformRequest::new(
        RuntimePlatformEffect::StoreSaveRecovery {
            payload: serde_json::json!({"runtimeConfig": {}}),
        },
        "recovery".into(),
        None,
    );
    assert_eq!(adapter.handle_platform_effect(&recovery).unwrap().len(), 1);
    assert!(adapter.handle_platform_effect(&reboot).unwrap().is_empty());
    assert!(!adapter.handle_platform_effect(&reboot).unwrap().is_empty());
    assert_eq!(adapter.take_power_request(), Some(PiPowerRequest::Reboot));
    assert_eq!(adapter.save_recovery_for_power(), Ok(()));
    let _ = std::fs::remove_dir_all(root);
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
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    let root = std::env::temp_dir().join(format!(
        "octessera-pi-setup-adapter-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    let public = root.join("public");
    let paths = SetupPortalPaths {
        request: root.join("request").join("inbox").join("start"),
        current: public.join("current.json"),
        public,
    };
    fs::create_dir_all(paths.request.parent().unwrap()).unwrap();
    fs::create_dir_all(&paths.public).unwrap();
    fs::set_permissions(paths.request.parent().unwrap(), permissions(0o700)).unwrap();
    fs::set_permissions(&paths.public, permissions(0o750)).unwrap();
    let environment = SetupPortalEnvironment::test(paths.clone(), 0);
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
    assert_eq!(request_id, "pi-setup");
    assert_eq!(*revision, Some(2));
    let RuntimeStoreResult::SetupPortalStatus { status } = result.as_ref() else {
        panic!("starting setup portal result");
    };
    assert_eq!(status.phase, RuntimeSetupPortalPhase::Starting);
    assert_eq!(fs::read(&paths.request).unwrap(), b"start\n");
    fs::remove_file(&paths.request).unwrap();
    let payload = serde_json::json!({"schema":1,"status":{"type":"setup_portal_status","phase":"starting","disposition":"accepted","rebootRequired":false}});
    fs::write(&paths.current, serde_json::to_vec(&payload).unwrap()).unwrap();
    fs::set_permissions(&paths.current, permissions(0o640)).unwrap();
    std::thread::sleep(Duration::from_millis(40));
    let ready = serde_json::json!({"schema":1,"status":{"type":"setup_portal_status","phase":"portal_ready","portalSuffix":"abcd","rebootRequired":false}});
    fs::write(&paths.current, serde_json::to_vec(&ready).unwrap()).unwrap();
    fs::set_permissions(&paths.current, permissions(0o640)).unwrap();
    let timeout = Duration::from_secs(5);
    let deadline = Instant::now() + timeout;
    let mut responses = Vec::new();
    let mut found = false;
    while Instant::now() < deadline {
        responses.extend(adapter.drain_platform_results(4));
        found = responses.iter().any(|message| {
            matches!(
                message,
                HostMessage::RuntimeResult {
                    result: RuntimeStoreResult::Identified {
                        request_id,
                        revision: Some(2),
                        ..
                    }
                } if request_id == "pi-setup"
            )
        });
        if found {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(
        found,
        "timed out after {timeout:?} waiting for pi-setup revision 2 result; drained responses: {responses:?}"
    );
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
