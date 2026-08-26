use super::*;
use crate::user_data_archive::{build_export_plan, write_archive};
use playback_runtime::{
    HostMessage, RuntimeErrorCode, RuntimePlatformEffect, RuntimePlatformRequest,
    RuntimeStoreResult, RuntimeUserDataTransferPhase, RuntimeUserDataTransferStatus,
};
use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::sync::Mutex;

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
    (
        UserDataTransferService::test(store, samples, random_source()),
        root,
    )
}

fn loopback_production_service(name: &str) -> (UserDataTransferService, PathBuf) {
    let root = root(name);
    (
        UserDataTransferService::new(
            root.join("store"),
            root.join("samples"),
            root.join("recordings"),
            root.join("screen-recordings"),
            random_source(),
            Arc::new(Mutex::new(())),
            TransferConfig {
                bind: SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0),
                network: TransferNetworkSource::Fixed(RegularWlan0Ipv4 {
                    address: "192.168.1.20".parse().unwrap(),
                    netmask: "255.255.255.0".parse().unwrap(),
                }),
                loopback_peer: true,
            },
        ),
        root,
    )
}

fn random_source() -> RandomSource {
    Arc::new(|bytes: &mut [u8]| {
        for (index, byte) in bytes.iter_mut().enumerate() {
            *byte = index as u8;
        }
        Ok(())
    })
}

fn transfer_request(id: &str, revision: u64) -> RuntimePlatformRequest {
    RuntimePlatformRequest::new(
        RuntimePlatformEffect::UserDataTransferOpen,
        id.into(),
        Some(revision),
    )
}

fn transfer_status(message: HostMessage) -> (RuntimeUserDataTransferStatus, String, Option<u64>) {
    let HostMessage::RuntimeResult {
        result:
            RuntimeStoreResult::Identified {
                result,
                request_id,
                revision,
            },
    } = message
    else {
        panic!("expected identified transfer status");
    };
    let RuntimeStoreResult::UserDataTransferStatus { status } = *result else {
        panic!("expected user-data transfer status");
    };
    (status, request_id, revision)
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
    let connect = if endpoint.ip().is_unspecified() {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), endpoint.port())
    } else {
        endpoint
    };
    let network = service.inner.state.lock().unwrap().network.unwrap();
    let host = format!("{}:{}", network.address, endpoint.port());
    request_at(connect, &host, method, path, code, body, extra_headers)
}

fn request_at(
    connect: SocketAddr,
    host: &str,
    method: &str,
    path: &str,
    code: &str,
    body: &[u8],
    extra_headers: &str,
) -> (u16, Vec<u8>) {
    let mut stream = TcpStream::connect(connect).unwrap();
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

#[test]
fn production_transfer_config_separates_listener_and_public_endpoint() {
    let root = root("production-config");
    let service = UserDataTransferService::production(
        root.join("store"),
        root.join("samples"),
        random_source(),
        Arc::new(Mutex::new(())),
    );
    assert_eq!(
        service.inner.config.bind,
        SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 8081)
    );
    assert!(matches!(
        service.inner.config.network,
        TransferNetworkSource::RegularWlan0
    ));
    assert!(!service.inner.config.loopback_peer);
    assert_eq!(
        service.inner.recordings_dir,
        crate::main_paths::default_recordings_dir()
    );
    assert_eq!(
        service.inner.screen_recordings_dir,
        crate::main_paths::default_screen_recordings_dir()
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn loopback_transfer_lifecycle_preserves_advertised_host_and_admission() {
    let (service, root) = loopback_production_service("loopback-bind");

    service.start().unwrap();
    let endpoint = service.test_endpoint().unwrap();
    assert_eq!(endpoint.ip(), IpAddr::V4(Ipv4Addr::UNSPECIFIED));
    assert_ne!(endpoint.port(), 0);

    let loopback = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), endpoint.port());
    let rejected = request_at(
        loopback,
        &format!("192.168.42.20:{}", endpoint.port()),
        "GET",
        "/restore/status",
        &service.test_code().unwrap(),
        &[],
        "",
    );
    assert_eq!(rejected.0, 403);

    let allowed = request_at(
        loopback,
        &format!("192.168.1.20:{}", endpoint.port()),
        "GET",
        "/restore/status",
        &service.test_code().unwrap(),
        &[],
        &format!("Origin: http://192.168.1.20:{}\r\n", endpoint.port()),
    );
    assert_eq!(allowed.0, 200);

    service.stop();
    let _ = fs::remove_dir_all(root);
}

#[test]
fn no_regular_network_returns_unavailable_without_binding() {
    let root = root("no-network");
    let service = UserDataTransferService::new(
        root.join("store"),
        root.join("samples"),
        root.join("recordings"),
        root.join("screen-recordings"),
        random_source(),
        Arc::new(Mutex::new(())),
        TransferConfig {
            bind: SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0),
            network: TransferNetworkSource::Unavailable,
            loopback_peer: false,
        },
    );
    let HostMessage::RuntimeResult {
        result: RuntimeStoreResult::RuntimeFailure { error },
    } = service.open(&transfer_request("no-network", 1))
    else {
        panic!("expected unavailable transfer failure");
    };
    assert_eq!(error.code, RuntimeErrorCode::Unavailable);
    assert_eq!(error.request_id.as_deref(), Some("no-network"));
    assert!(service.test_endpoint().is_none());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn reopen_preserves_code_and_deadline_and_returns_remaining_lifetime() {
    let (service, root) = service("reopen");
    let first = transfer_request("open-first", 1);
    let (first_status, first_id, first_revision) = transfer_status(service.open(&first));
    assert_eq!(first_status.phase, RuntimeUserDataTransferPhase::Ready);
    assert_eq!(first_id, "open-first");
    assert_eq!(first_revision, Some(1));
    let first_code = first_status.code.clone();
    let first_url = first_status.url.clone();
    let deadline = service.inner.state.lock().unwrap().expires_at.unwrap();

    std::thread::sleep(Duration::from_millis(5));
    let second = transfer_request("open-second", 2);
    let (second_status, second_id, second_revision) = transfer_status(service.open(&second));
    assert_eq!(second_status.phase, RuntimeUserDataTransferPhase::Ready);
    assert_eq!(second_status.code, first_code);
    assert_eq!(second_status.url, first_url);
    assert!(second_status.expires_in_seconds.unwrap() <= first_status.expires_in_seconds.unwrap());
    assert_eq!(
        service.inner.state.lock().unwrap().expires_at,
        Some(deadline)
    );
    assert_eq!(second_id, "open-second");
    assert_eq!(second_revision, Some(2));
    service.stop();
    let _ = fs::remove_dir_all(root);
}

#[test]
fn close_expiry_and_auth_revocation_emit_closed_with_their_open_identity() {
    let (service, root) = service("closed-status");
    let open = transfer_request("close-open", 3);
    let _ = service.open(&open);
    let close_request = RuntimePlatformRequest::new(
        RuntimePlatformEffect::UserDataTransferClose,
        "close-request".into(),
        Some(4),
    );
    let (closed, request_id, revision) = transfer_status(service.close(&close_request));
    assert_eq!(closed.phase, RuntimeUserDataTransferPhase::Closed);
    assert_eq!(request_id, "close-request");
    assert_eq!(revision, Some(4));

    let open = transfer_request("expiry-open", 5);
    let _ = service.open(&open);
    service.inner.state.lock().unwrap().expires_at = Some(Instant::now() - Duration::from_secs(1));
    service.expire_if_needed();
    let (closed, request_id, revision) = transfer_status(service.take_runtime_status().unwrap());
    assert_eq!(closed.phase, RuntimeUserDataTransferPhase::Closed);
    assert_eq!(request_id, "expiry-open");
    assert_eq!(revision, Some(5));

    let open = transfer_request("auth-open", 6);
    let _ = service.open(&open);
    for _ in 0..MAX_AUTH_FAILURES {
        assert_eq!(
            request(&service, "GET", "/restore/status", "wrong", &[], "").0,
            401
        );
    }
    let (closed, request_id, revision) = transfer_status(service.take_runtime_status().unwrap());
    assert_eq!(closed.phase, RuntimeUserDataTransferPhase::Closed);
    assert_eq!(request_id, "auth-open");
    assert_eq!(revision, Some(6));
    let _ = fs::remove_dir_all(root);
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
