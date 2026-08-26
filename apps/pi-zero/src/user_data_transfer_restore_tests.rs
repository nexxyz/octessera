use super::*;
use crate::user_data_archive::{build_export_plan, write_archive};
use playback_runtime::{
    HostMessage, RuntimePlatformEffect, RuntimePlatformRequest, RuntimeStoreResult,
    RuntimeUserDataRestorePhase,
};
use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

fn restore_root(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "octessera-user-transfer-restore-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    root
}

fn restore_service(name: &str) -> (UserDataTransferService, PathBuf) {
    let root = restore_root(name);
    let store = root.join("store");
    let samples = root.join("samples");
    fs::create_dir_all(&store).unwrap();
    fs::create_dir_all(&samples).unwrap();
    fs::create_dir_all(root.join("recordings")).unwrap();
    fs::create_dir_all(root.join("screen-recordings")).unwrap();
    fs::write(
        store.join("default.json"),
        serde_json::to_vec(&crate::user_data_archive::canonical_defaults()).unwrap(),
    )
    .unwrap();
    (
        UserDataTransferService::test(store, samples, restore_random_source()),
        root,
    )
}

fn restore_random_source() -> RandomSource {
    Arc::new(|bytes: &mut [u8]| {
        for (index, byte) in bytes.iter_mut().enumerate() {
            *byte = index as u8;
        }
        Ok(())
    })
}

fn restore_request(
    service: &UserDataTransferService,
    method: &str,
    path: &str,
    code: &str,
    body: &[u8],
) -> (u16, Vec<u8>) {
    let endpoint = service.test_endpoint().unwrap();
    let mut stream = TcpStream::connect(endpoint).unwrap();
    write!(
        stream,
        "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nX-Octessera-Transfer-Code: {code}\r\nContent-Length: {}\r\n\r\n",
        endpoint.port(),
        body.len()
    )
    .unwrap();
    stream.write_all(body).unwrap();
    let mut response = Vec::new();
    stream.read_to_end(&mut response).unwrap();
    let header_end = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .unwrap();
    let status = response[..header_end]
        .split(|byte| *byte == b' ')
        .nth(1)
        .and_then(|value| std::str::from_utf8(value).ok())
        .unwrap()
        .parse()
        .unwrap();
    (status, response[header_end + 4..].to_vec())
}

fn restore_archive(service: &UserDataTransferService, include_media: bool) -> Vec<u8> {
    let inner = service.inner.clone();
    let plan = build_export_plan(
        &inner.store_dir,
        &inner.samples_dir,
        &inner.recordings_dir,
        &inner.screen_recordings_dir,
        include_media,
    )
    .unwrap();
    let mut bytes = Vec::new();
    write_archive(&plan, &mut bytes).unwrap();
    bytes
}

#[test]
fn upload_requires_limits_and_physical_cancel_before_mutation() {
    let (service, root) = restore_service("cancel");
    service.start().unwrap();
    let code = service.test_code().unwrap();
    let original = fs::read(service.inner.store_dir.join("default.json")).unwrap();
    let invalid = restore_request(&service, "POST", "/restore", &code, b"invalid");
    assert_eq!(invalid.0, 400);
    assert_eq!(
        fs::read(service.inner.store_dir.join("default.json")).unwrap(),
        original
    );
    let oversized = {
        let endpoint = service.test_endpoint().unwrap();
        let mut stream = TcpStream::connect(endpoint).unwrap();
        write!(
            stream,
            "POST /restore HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nX-Octessera-Transfer-Code: {code}\r\nContent-Length: {}\r\n\r\n",
            endpoint.port(),
            crate::user_data_archive::max_archive_bytes() + 1
        )
        .unwrap();
        let mut response = Vec::new();
        stream.read_to_end(&mut response).unwrap();
        let header_end = response
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .unwrap();
        let status = response[..header_end]
            .split(|byte| *byte == b' ')
            .nth(1)
            .and_then(|value| std::str::from_utf8(value).ok())
            .unwrap()
            .parse::<u16>()
            .unwrap();
        status
    };
    assert_eq!(oversized, 413);

    let archive = restore_archive(&service, false);
    let staged = restore_request(&service, "POST", "/restore", &code, &archive);
    assert_eq!(staged.0, 202);
    service.handle_physical_input(&serde_json::json!({
        "type":"button_a",
        "pressed":true
    }));
    let status = restore_request(&service, "GET", "/restore/status", &code, &[]);
    assert_eq!(status.0, 200);
    assert!(String::from_utf8(status.1).unwrap().contains("cancelled"));
    assert_eq!(
        fs::read(service.inner.store_dir.join("default.json")).unwrap(),
        original
    );
    service.stop();
    let _ = fs::remove_dir_all(root);
}

#[test]
fn confirmed_restore_writes_backup_and_preserves_protected_files() {
    let (service, root) = restore_service("restore");
    fs::write(service.inner.samples_dir.join("User.wav"), b"custom sample").unwrap();
    fs::write(
        service.inner.recordings_dir.join("take.wav"),
        b"audio recording",
    )
    .unwrap();
    fs::write(
        service.inner.screen_recordings_dir.join("screen.webm"),
        b"screen recording",
    )
    .unwrap();
    fs::write(
        service.inner.store_dir.join("device.json"),
        br#"{"hardware":"fresh"}"#,
    )
    .unwrap();
    fs::write(
        service.inner.store_dir.join("recovery-save.json"),
        serde_json::to_vec(&crate::user_data_archive::canonical_defaults()).unwrap(),
    )
    .unwrap();
    service.start().unwrap();
    let code = service.test_code().unwrap();
    let generation = service.store_write_barrier().current_generation();
    let archive = restore_archive(&service, true);
    assert_eq!(
        restore_request(&service, "POST", "/restore", &code, &archive).0,
        202
    );
    service.handle_physical_input(&serde_json::json!({
        "type":"encoder_press",
        "id":"main"
    }));
    assert_eq!(
        service.store_write_barrier().current_generation(),
        generation + 1
    );
    let mut status_body = String::new();
    for _ in 0..100 {
        let status = restore_request(&service, "GET", "/restore/status", &code, &[]);
        status_body = String::from_utf8(status.1).unwrap();
        if status_body.contains("restored") {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(status_body.contains("restored"), "{status_body}");
    assert!(service.store_write_barrier().is_blocked());
    service.store_write_barrier().acknowledge();
    let default = crate::platform_service::load_json(&service.inner.store_dir.join("default.json"))
        .unwrap()
        .unwrap();
    assert_eq!(default["kind"], "octessera.config");
    assert_eq!(
        fs::read(service.inner.samples_dir.join("User.wav")).unwrap(),
        b"custom sample"
    );
    assert_eq!(
        fs::read(service.inner.recordings_dir.join("take.wav")).unwrap(),
        b"audio recording"
    );
    assert_eq!(
        fs::read(service.inner.screen_recordings_dir.join("screen.webm")).unwrap(),
        b"screen recording"
    );
    assert_eq!(
        fs::read(service.inner.store_dir.join("device.json")).unwrap(),
        br#"{"hardware":"fresh"}"#
    );
    assert_eq!(
        fs::read(service.inner.store_dir.join("recovery-save.json")).unwrap(),
        serde_json::to_vec(&crate::user_data_archive::canonical_defaults()).unwrap()
    );
    assert!(fs::read_dir(&root)
        .unwrap()
        .filter_map(Result::ok)
        .any(|entry| entry
            .file_name()
            .to_string_lossy()
            .starts_with("octessera-pre-restore-")));
    service.stop();
    let _ = fs::remove_dir_all(root);
}

#[test]
fn active_recording_restore_is_rejected_before_tree_mutation() {
    let (service, root) = restore_service("active-recording-restore");
    let recording = Arc::new(AtomicBool::new(true));
    let recording_for_preflight = recording.clone();
    service.set_restore_preflight(Arc::new(move || {
        if recording_for_preflight.load(Ordering::Acquire) {
            return Err(
                "restore blocked: audio recording is active; stop recording before restore".into(),
            );
        }
        Ok(())
    }));
    let original = fs::read(service.inner.store_dir.join("default.json")).unwrap();
    service.start().unwrap();
    let code = service.test_code().unwrap();
    let archive = restore_archive(&service, false);
    assert_eq!(
        restore_request(&service, "POST", "/restore", &code, &archive).0,
        202
    );
    service.handle_physical_input(&serde_json::json!({
        "type":"encoder_press",
        "id":"main"
    }));

    let mut status_body = String::new();
    for _ in 0..100 {
        let status = restore_request(&service, "GET", "/restore/status", &code, &[]);
        status_body = String::from_utf8(status.1).unwrap();
        if status_body.contains("blocked_recording_active") {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(
        status_body.contains("blocked_recording_active"),
        "{status_body}"
    );
    assert!(!service.store_write_barrier().is_blocked());
    assert_eq!(
        fs::read(service.inner.store_dir.join("default.json")).unwrap(),
        original
    );
    assert!(!root
        .read_dir()
        .unwrap()
        .filter_map(Result::ok)
        .any(|entry| entry
            .file_name()
            .to_string_lossy()
            .starts_with("octessera-pre-restore-")));
    assert!(!service
        .inner
        .store_dir
        .read_dir()
        .unwrap()
        .filter_map(Result::ok)
        .any(|entry| entry
            .file_name()
            .to_string_lossy()
            .starts_with(".user-data-stage-")));
    recording.store(false, Ordering::Release);
    service.stop();
    let _ = fs::remove_dir_all(root);
}

#[test]
fn pending_restore_times_out_and_removes_staged_data() {
    let (service, root) = restore_service("restore-timeout");
    service.start().unwrap();
    let code = service.test_code().unwrap();
    let archive = restore_archive(&service, false);
    assert_eq!(
        restore_request(&service, "POST", "/restore", &code, &archive).0,
        202
    );
    if let Ok(mut state) = service.inner.state.lock() {
        if let RestoreState::Pending(pending) = &mut state.restore {
            pending.expires_at = Instant::now() - Duration::from_secs(1);
        }
    }
    service.expire_if_needed();
    let status = restore_request(&service, "GET", "/restore/status", &code, &[]);
    assert!(String::from_utf8(status.1).unwrap().contains("timed_out"));
    assert!(!service
        .inner
        .store_dir
        .read_dir()
        .unwrap()
        .filter_map(Result::ok)
        .any(|entry| entry
            .file_name()
            .to_string_lossy()
            .starts_with(".user-data-stage-")));
    service.stop();
    let _ = fs::remove_dir_all(root);
}

#[test]
fn confirmation_publishes_blocking_status_before_store_worker_runs() {
    let (service, root) = restore_service("blocking-status");
    let request_identity = RuntimePlatformRequest::new(
        RuntimePlatformEffect::SetupPortalOpen,
        "restore-status".into(),
        Some(4),
    );
    service.start_session(Some(&request_identity)).unwrap();
    let code = service.test_code().unwrap();
    let archive = restore_archive(&service, false);
    assert_eq!(
        restore_request(&service, "POST", "/restore", &code, &archive).0,
        202
    );
    let store_guard = service.inner.store_lock.lock().unwrap();
    assert!(!service.handle_physical_input(&serde_json::json!({
        "type":"encoder_press",
        "id":"main"
    })));
    let status = service.take_runtime_status().unwrap();
    assert!(matches!(
        status,
        HostMessage::RuntimeResult {
            result: RuntimeStoreResult::Identified { result, request_id, revision: Some(4) }
        } if request_id == "restore-status"
            && matches!(result.as_ref(), RuntimeStoreResult::UserDataRestoreStatus { status } if status.phase == RuntimeUserDataRestorePhase::Restoring)
    ));
    assert!(!service.handle_physical_input(&serde_json::json!({"type":"grid_press"})));
    drop(store_guard);
    service.stop();
    let _ = fs::remove_dir_all(root);
}

#[test]
fn confirmation_input_is_consumed_when_restore_finishes_immediately() {
    let (service, root) = restore_service("fast-confirmation");
    service.start().unwrap();
    let code = service.test_code().unwrap();
    let archive = restore_archive(&service, false);
    assert_eq!(
        restore_request(&service, "POST", "/restore", &code, &archive).0,
        202
    );
    assert!(!service.handle_physical_input(&serde_json::json!({
        "type":"encoder_press",
        "id":"main"
    })));
    service.stop();
    let _ = fs::remove_dir_all(root);
}
