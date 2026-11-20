use super::restore_tests::{restore_archive, restore_request, restore_service};
use super::*;
use playback_runtime::{RuntimePlatformEffect, RuntimePlatformRequest};
use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
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

pub(super) fn wait_for_received_upload(store: &Path, length: u64) {
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
            .any(|entry| fs::metadata(entry.path()).is_ok_and(|metadata| metadata.len() == length));
        if received {
            return;
        }
        thread::sleep(Duration::from_millis(1));
    }
    panic!("upload body never arrived");
}

pub(super) fn wait_for_server_exit(service: &UserDataTransferService) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while service
        .inner
        .server
        .lock()
        .unwrap()
        .as_ref()
        .is_some_and(|join| !join.is_finished())
    {
        assert!(Instant::now() < deadline, "transfer server never stopped");
        thread::sleep(Duration::from_millis(5));
    }
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
    let store_guard = service.inner.stage_gate.lock().unwrap();
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

#[test]
fn reopening_while_an_upload_still_finishes_reports_still_closing() {
    let (service, root) = restore_service("reopen-while-closing");
    service.start().unwrap();
    let code = service.test_code().unwrap();
    let store = service.inner.store_dir.clone();
    let archive = restore_archive(&service, false);
    let length = archive.len() as u64;
    let store_guard = service.inner.stage_gate.lock().unwrap();
    let uploader = {
        let service = service.clone();
        thread::spawn(move || restore_request(&service, "POST", "/restore", &code, &archive))
    };
    wait_for_received_upload(&store, length);
    service.close(&close_request());

    assert_eq!(
        service.start(),
        Err("Still closing, try again in a moment".to_string())
    );
    drop(store_guard);
    assert_eq!(uploader.join().unwrap().0, 410);
    wait_for_server_exit(&service);
    service.start().unwrap();
    assert!(service.test_code().is_some());
    service.stop();
    let _ = fs::remove_dir_all(root);
}

#[test]
fn closing_during_a_mid_body_upload_aborts_it_and_allows_reopening() {
    let (service, root) = restore_service("close-mid-body");
    service.start().unwrap();
    let endpoint = service.test_endpoint().unwrap();
    let code = service.test_code().unwrap();
    let store = service.inner.store_dir.clone();
    let mut stream = TcpStream::connect(endpoint).unwrap();
    write!(
        stream,
        "POST /restore HTTP/1.1\r\nHost: {endpoint}\r\nX-Octessera-Transfer-Code: {code}\r\nContent-Length: 1000000\r\n\r\n"
    )
    .unwrap();
    stream.write_all(&[0; 16 * 1024]).unwrap();
    wait_for_received_upload(&store, 16 * 1024);

    let started = Instant::now();
    service.close(&close_request());
    assert!(started.elapsed() < Duration::from_secs(2));
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut response = Vec::new();
    let _ = stream.read_to_end(&mut response);

    assert!(!response.starts_with(b"HTTP/1.1 202"));
    assert!(!service.confirm_pending_restore(true));
    wait_for_server_exit(&service);
    assert!(transfer_entries(&store).is_empty());
    service.start().unwrap();
    service.stop();
    let _ = fs::remove_dir_all(root);
}
