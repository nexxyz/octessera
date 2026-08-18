use super::*;
use crate::user_data_archive::{build_export_plan, write_archive};
use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::sync::Arc;

fn root(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "octessera-user-transfer-{name}-{}-{}",
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

fn service(name: &str) -> (UserDataTransferService, PathBuf) {
    let root = root(name);
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
    let random = Arc::new(|bytes: &mut [u8]| {
        for (index, byte) in bytes.iter_mut().enumerate() {
            *byte = index as u8;
        }
        Ok(())
    });
    (UserDataTransferService::test(store, samples, random), root)
}

fn request(
    service: &UserDataTransferService,
    method: &str,
    path: &str,
    code: &str,
    body: &[u8],
    extra_headers: &str,
) -> (u16, Vec<u8>) {
    let endpoint = service.test_endpoint().unwrap();
    let host = format!("127.0.0.1:{}", endpoint.port());
    let mut stream = TcpStream::connect(endpoint).unwrap();
    let length_header = if extra_headers.contains("Content-Length:") {
        String::new()
    } else {
        format!("Content-Length: {}\r\n", body.len())
    };
    write!(
        stream,
        "{method} {path} HTTP/1.1\r\nHost: {host}\r\nX-Octessera-Transfer-Code: {code}\r\n{length_header}{extra_headers}\r\n"
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

fn archive_for(service: &UserDataTransferService, include_media: bool) -> Vec<u8> {
    let (store, samples, recordings, screen_recordings) = {
        let inner = service.inner.clone();
        (
            inner.store_dir.clone(),
            inner.samples_dir.clone(),
            inner.recordings_dir.clone(),
            inner.screen_recordings_dir.clone(),
        )
    };
    let plan = build_export_plan(
        &store,
        &samples,
        &recordings,
        &screen_recordings,
        include_media,
    )
    .unwrap();
    let mut bytes = Vec::new();
    write_archive(&plan, &mut bytes).unwrap();
    bytes
}

#[test]
fn auth_and_exact_origin_are_required_without_cors() {
    let (service, root) = service("auth");
    service.start().unwrap();
    let endpoint = service.test_endpoint().unwrap();
    let wrong = request(&service, "GET", "/restore/status", "wrong", &[], "");
    assert_eq!(wrong.0, 401);
    let origin = request(
        &service,
        "GET",
        "/restore/status",
        &service.test_code().unwrap(),
        &[],
        "Origin: http://wrong.example\r\n",
    );
    assert_eq!(origin.0, 403);
    let correct = request(
        &service,
        "GET",
        "/restore/status",
        &service.test_code().unwrap(),
        &[],
        &format!("Origin: http://127.0.0.1:{}\r\n", endpoint.port()),
    );
    assert_eq!(correct.0, 200);
    let exported = request(
        &service,
        "GET",
        "/export",
        &service.test_code().unwrap(),
        &[],
        "",
    );
    assert_eq!(exported.0, 200);
    assert!(exported.1.starts_with(b"OCTESSERA-USER-DATA\0"));
    service.stop();
    let _ = fs::remove_dir_all(root);
}

#[test]
fn repeated_bad_codes_revoke_the_transfer_session() {
    let (service, root) = service("auth-limit");
    service.start().unwrap();
    for _ in 0..MAX_AUTH_FAILURES {
        assert_eq!(
            request(&service, "GET", "/restore/status", "wrong", &[], "").0,
            401
        );
    }
    assert!(service.test_code().is_none());
    service.stop();
    let _ = fs::remove_dir_all(root);
}

#[test]
fn upload_requires_limits_and_physical_cancel_before_mutation() {
    let (service, root) = service("cancel");
    service.start().unwrap();
    let code = service.test_code().unwrap();
    let original = fs::read(service.inner.store_dir.join("default.json")).unwrap();
    let invalid = request(&service, "POST", "/restore", &code, b"invalid", "");
    assert_eq!(invalid.0, 400);
    assert_eq!(
        fs::read(service.inner.store_dir.join("default.json")).unwrap(),
        original
    );
    let oversized = request(
        &service,
        "POST",
        "/restore",
        &code,
        &[],
        &format!(
            "Content-Length: {}\r\n",
            crate::user_data_archive::max_archive_bytes() + 1
        ),
    );
    assert_eq!(oversized.0, 413);

    let archive = archive_for(&service, false);
    let staged = request(&service, "POST", "/restore", &code, &archive, "");
    assert_eq!(staged.0, 202);
    service.handle_physical_input(&serde_json::json!({
        "type":"button_a",
        "pressed":true
    }));
    let status = request(&service, "GET", "/restore/status", &code, &[], "");
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
    let (service, root) = service("restore");
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
    let archive = archive_for(&service, true);
    assert_eq!(
        request(&service, "POST", "/restore", &code, &archive, "").0,
        202
    );
    service.handle_physical_input(&serde_json::json!({
        "type":"encoder_press",
        "id":"main"
    }));
    let status = request(&service, "GET", "/restore/status", &code, &[], "");
    assert!(String::from_utf8(status.1).unwrap().contains("restored"));
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
fn pending_restore_times_out_and_removes_staged_data() {
    let (service, root) = service("restore-timeout");
    service.start().unwrap();
    let code = service.test_code().unwrap();
    let archive = archive_for(&service, false);
    assert_eq!(
        request(&service, "POST", "/restore", &code, &archive, "").0,
        202
    );
    if let Ok(mut state) = service.inner.state.lock() {
        if let RestoreState::Pending(pending) = &mut state.restore {
            pending.expires_at = Instant::now() - Duration::from_secs(1);
        }
    }
    service.expire_if_needed();
    let status = request(&service, "GET", "/restore/status", &code, &[], "");
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
