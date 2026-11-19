use super::restore_tests::{restore_archive, restore_request, restore_service};
use super::*;
use playback_runtime::{RuntimePlatformEffect, RuntimePlatformRequest};
use std::fs;
use std::thread;
use std::time::{Duration, Instant};

fn close_request() -> RuntimePlatformRequest {
    RuntimePlatformRequest::new(
        RuntimePlatformEffect::UserDataTransferClose,
        "close".into(),
        None,
    )
}

fn transfer_entries(store: &Path) -> Vec<String> {
    fs::read_dir(store)
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with(".user-data-"))
        .collect()
}

fn wait_for_received_upload(store: &Path, length: u64) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        let received = fs::read_dir(store)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".user-data-upload-")
            })
            .any(|entry| {
                entry
                    .metadata()
                    .is_ok_and(|metadata| metadata.len() == length)
            });
        if received {
            return;
        }
        thread::sleep(Duration::from_millis(1));
    }
    panic!("upload body never arrived");
}

#[test]
fn closing_during_an_upload_never_arms_a_restore() {
    let (service, root) = restore_service("close-during-upload");
    service.start().unwrap();
    let code = service.test_code().unwrap();
    let store = service.inner.store_dir.clone();
    let original = fs::read(store.join("system.json")).unwrap();
    let archive = restore_archive(&service, false);
    let length = archive.len() as u64;
    let store_guard = service.inner.store_lock.lock().unwrap();
    let uploader = {
        let service = service.clone();
        thread::spawn(move || restore_request(&service, "POST", "/restore", &code, &archive))
    };
    wait_for_received_upload(&store, length);
    service.close(&close_request());
    drop(store_guard);
    let (status, body) = uploader.join().unwrap();

    assert_eq!(status, 410);
    assert_eq!(body, br#"{"error":"transfer_closed"}"#);
    assert!(!service.confirm_pending_restore(true));
    assert!(transfer_entries(&store).is_empty());
    assert_eq!(fs::read(store.join("system.json")).unwrap(), original);
    service.stop();
    let _ = fs::remove_dir_all(root);
}
