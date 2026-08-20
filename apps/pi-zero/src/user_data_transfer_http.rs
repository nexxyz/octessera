use super::http_protocol::{copy_request_body, read_request, respond, response_header};
use super::*;
use crate::user_data_archive::{self, ExportPlan};
use serde_json::json;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::Path;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread;

pub(super) fn run_server(inner: Arc<TransferInner>, listener: TcpListener) {
    while !inner.stop.load(Ordering::Acquire) {
        if !inner
            .state
            .lock()
            .map(|state| state.active)
            .unwrap_or(false)
        {
            break;
        }
        match listener.accept() {
            Ok((mut stream, peer)) => {
                let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
                let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
                handle_connection(&inner, &mut stream, peer);
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(ACCEPT_POLL);
            }
            Err(_) => break,
        }
    }
}

fn handle_connection(inner: &Arc<TransferInner>, stream: &mut TcpStream, peer: SocketAddr) {
    let request = match read_request(stream) {
        Ok(request) => request,
        Err(_) => {
            respond(
                stream,
                400,
                "application/json",
                br#"{"error":"bad_request"}"#,
            );
            return;
        }
    };
    if !peer_allowed(inner, peer)
        || request.headers.get("host") != Some(&expected_host(inner))
        || request
            .headers
            .get("origin")
            .is_some_and(|origin| origin != &format!("http://{}", expected_host(inner)))
    {
        respond(
            stream,
            403,
            "application/json",
            br#"{"error":"local_origin_required"}"#,
        );
        return;
    }
    if request.path != "/" && !authorized(inner, request.headers.get("x-octessera-transfer-code")) {
        respond(
            stream,
            401,
            "application/json",
            br#"{"error":"unauthorized"}"#,
        );
        return;
    }
    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/") => respond(
            stream,
            200,
            "text/html; charset=utf-8",
            b"<h1>Octessera transfer</h1>",
        ),
        ("GET", "/export") => handle_export(inner, stream, request.query.as_deref()),
        ("GET", "/restore/status") => handle_restore_status(inner, stream),
        ("POST", "/restore") => handle_restore_upload(inner, stream, request.content_length),
        _ => respond(
            stream,
            405,
            "application/json",
            br#"{"error":"method_not_allowed"}"#,
        ),
    }
}

fn handle_export(inner: &Arc<TransferInner>, stream: &mut TcpStream, query: Option<&str>) {
    let include_media = match query {
        None => false,
        Some("media=1" | "includeMedia=1" | "includeMedia=true") => true,
        Some(_) => {
            respond(
                stream,
                400,
                "application/json",
                br#"{"error":"invalid_query"}"#,
            );
            return;
        }
    };
    let Ok(_store_guard) = inner.store_lock.lock() else {
        respond(
            stream,
            503,
            "application/json",
            br#"{"error":"export_unavailable"}"#,
        );
        return;
    };
    let plan = match user_export_plan(inner, include_media) {
        Ok(plan) => plan,
        Err(_) => {
            respond(
                stream,
                409,
                "application/json",
                br#"{"error":"export_unavailable"}"#,
            );
            return;
        }
    };
    let length = match user_data_archive::archive_len(&plan) {
        Ok(length) => length,
        Err(_) => {
            respond(
                stream,
                409,
                "application/json",
                br#"{"error":"export_too_large"}"#,
            );
            return;
        }
    };
    let header = response_header(
        200,
        "application/octet-stream",
        length,
        Some("octessera-user-data.oct"),
    );
    if stream.write_all(&header).is_err() {
        return;
    }
    let _ = user_data_archive::write_archive(&plan, stream);
}

fn handle_restore_status(inner: &Arc<TransferInner>, stream: &mut TcpStream) {
    let status = inner
        .state
        .lock()
        .ok()
        .map(|state| match &state.restore {
            RestoreState::None => json!({"status":"idle"}),
            RestoreState::Pending(pending) => {
                json!({"status":"confirmation_required","session":pending.session})
            }
            RestoreState::Restoring { session } => {
                json!({"status":"restoring","session":session})
            }
            RestoreState::Finished { session, status } => {
                json!({"status":status,"session":session})
            }
        })
        .unwrap_or_else(|| json!({"status":"unavailable"}));
    let body =
        serde_json::to_vec(&status).unwrap_or_else(|_| b"{\"status\":\"unavailable\"}".to_vec());
    respond(stream, 200, "application/json", &body);
}

fn handle_restore_upload(inner: &Arc<TransferInner>, stream: &mut TcpStream, length: Option<u64>) {
    let Some(length) = length else {
        respond(
            stream,
            411,
            "application/json",
            br#"{"error":"content_length_required"}"#,
        );
        return;
    };
    if length == 0 || length > user_data_archive::max_archive_bytes() {
        respond(
            stream,
            413,
            "application/json",
            br#"{"error":"upload_too_large"}"#,
        );
        return;
    }
    if !restore_slot_available(inner) {
        respond(
            stream,
            409,
            "application/json",
            br#"{"error":"restore_not_available"}"#,
        );
        return;
    }
    let session = match random_session(&inner.random) {
        Ok(session) => session,
        Err(_) => {
            respond(
                stream,
                503,
                "application/json",
                br#"{"error":"random_unavailable"}"#,
            );
            return;
        }
    };
    let Ok(_store_guard) = inner.store_lock.lock() else {
        respond(
            stream,
            503,
            "application/json",
            br#"{"error":"restore_unavailable"}"#,
        );
        return;
    };
    let upload_path = inner.store_dir.join(format!(".user-data-upload-{session}"));
    let stage_root = inner.store_dir.join(format!(".user-data-stage-{session}"));
    let result = (|| {
        fs::create_dir_all(&inner.store_dir).map_err(io_error)?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&upload_path)
            .map_err(io_error)?;
        copy_request_body(stream, &mut file, length)?;
        file.sync_all().map_err(io_error)?;
        let staged = user_data_archive::stage_archive(&upload_path, &stage_root)?;
        validate_free_space(
            &inner.store_dir,
            &inner.samples_dir,
            &inner.recordings_dir,
            &inner.screen_recordings_dir,
            length,
        )?;
        Ok::<_, String>(staged)
    })();
    let _ = fs::remove_file(&upload_path);
    let staged = match result {
        Ok(staged) => staged,
        Err(_) => {
            let _ = fs::remove_dir_all(&stage_root);
            respond(
                stream,
                400,
                "application/json",
                br#"{"error":"invalid_archive"}"#,
            );
            return;
        }
    };
    let response = {
        let Ok(mut state) = inner.state.lock() else {
            super::remove_stage(&staged);
            respond(
                stream,
                503,
                "application/json",
                br#"{"error":"unavailable"}"#,
            );
            return;
        };
        state.restore = RestoreState::Pending(Box::new(PendingRestore {
            session: session.clone(),
            staged,
            expires_at: Instant::now() + RESTORE_CONFIRM_LIFETIME,
        }));
        json!({"status":"confirmation_required","session":session})
    };
    let body =
        serde_json::to_vec(&response).unwrap_or_else(|_| b"{\"error\":\"unavailable\"}".to_vec());
    respond(stream, 202, "application/json", &body);
}

fn restore_slot_available(inner: &Arc<TransferInner>) -> bool {
    inner
        .state
        .lock()
        .map(|state| {
            state.active
                && matches!(
                    &state.restore,
                    RestoreState::None | RestoreState::Finished { .. }
                )
        })
        .unwrap_or(false)
}

fn user_export_plan(inner: &Arc<TransferInner>, include_media: bool) -> Result<ExportPlan, String> {
    user_data_archive::build_export_plan(
        &inner.store_dir,
        &inner.samples_dir,
        &inner.recordings_dir,
        &inner.screen_recordings_dir,
        include_media,
    )
}

fn authorized(inner: &Arc<TransferInner>, candidate: Option<&String>) -> bool {
    let Ok(mut state) = inner.state.lock() else {
        return false;
    };
    let valid = match (state.active, state.code.as_deref(), candidate) {
        (true, Some(expected), Some(candidate)) => {
            constant_time_equal(expected.as_bytes(), candidate.as_bytes())
        }
        _ => false,
    };
    if !valid {
        state.auth_failures = state.auth_failures.saturating_add(1);
        if state.auth_failures >= MAX_AUTH_FAILURES {
            state.active = false;
            state.code = None;
            inner.stop.store(true, Ordering::Release);
        }
    }
    valid
}

fn peer_allowed(inner: &Arc<TransferInner>, peer: SocketAddr) -> bool {
    if inner.config.loopback_peer {
        return peer.ip() == IpAddr::V4(Ipv4Addr::LOCALHOST);
    }
    matches!(peer.ip(), IpAddr::V4(ip) if ip.octets()[0..3] == [192, 168, 42])
}

fn expected_host(inner: &Arc<TransferInner>) -> String {
    inner
        .state
        .lock()
        .ok()
        .and_then(|state| state.endpoint)
        .map(|endpoint| format!("{}:{}", inner.config.public_host, endpoint.port()))
        .unwrap_or_else(|| format!("{}:{}", inner.config.public_host, TRANSFER_PORT))
}

fn validate_free_space(
    store_dir: &Path,
    samples_dir: &Path,
    recordings_dir: &Path,
    screen_recordings_dir: &Path,
    incoming: u64,
) -> Result<(), String> {
    let existing = directory_size(store_dir)
        .saturating_add(directory_size(samples_dir))
        .saturating_add(directory_size(recordings_dir))
        .saturating_add(directory_size(screen_recordings_dir));
    let required = existing
        .saturating_add(incoming)
        .saturating_add(4 * 1024 * 1024);
    let Some(available) = available_space(store_dir) else {
        return Ok(());
    };
    if available < required {
        return Err("insufficient space for restore".into());
    }
    Ok(())
}

fn directory_size(path: &Path) -> u64 {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return 0;
    };
    if metadata.is_file() {
        return metadata.len();
    }
    if !metadata.is_dir() {
        return 0;
    }
    fs::read_dir(path)
        .ok()
        .into_iter()
        .flat_map(|entries| entries.filter_map(Result::ok))
        .map(|entry| directory_size(&entry.path()))
        .sum()
}

#[cfg(unix)]
fn available_space(path: &Path) -> Option<u64> {
    use std::ffi::CString;
    let path = CString::new(path.as_os_str().as_encoded_bytes()).ok()?;
    let mut stats = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    let result = unsafe { libc::statvfs(path.as_ptr(), stats.as_mut_ptr()) };
    if result != 0 {
        return None;
    }
    let stats = unsafe { stats.assume_init() };
    Some(stats.f_bavail.saturating_mul(stats.f_frsize))
}

#[cfg(not(unix))]
fn available_space(_path: &Path) -> Option<u64> {
    None
}

fn random_session(random: &RandomSource) -> Result<String, String> {
    let mut bytes = [0; 16];
    random(&mut bytes)?;
    Ok(bytes
        .into_iter()
        .map(|byte| CODE_ALPHABET[byte as usize % CODE_ALPHABET.len()] as char)
        .collect())
}

fn constant_time_equal(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter()
        .zip(right)
        .fold(0_u8, |difference, (left, right)| {
            difference | (left ^ right)
        })
        == 0
}

fn io_error(error: io::Error) -> String {
    error.to_string()
}
