use super::http_protocol::{copy_request_body, read_request, respond, response_header};
use super::restore_space::{validate_free_space, TRANSFER_TEMP_PREFIX};
use super::*;
use crate::user_data_archive::{self, io_error, ArchiveError, ExportPlan};
use serde_json::json;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
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
                let _ = stream.set_nonblocking(false);
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
    let plan = {
        let Ok(_store_guard) = inner.store_lock.lock() else {
            respond(
                stream,
                503,
                "application/json",
                br#"{"error":"export_unavailable"}"#,
            );
            return;
        };
        match user_export_plan(inner, include_media) {
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
    let mut header = response_header(
        200,
        "application/octet-stream",
        length,
        Some("octessera-user-data.oct"),
    );
    let blank_line = header.len() - 2;
    for warning in &plan.warnings {
        eprintln!("User-data export: {warning}");
        let printable = warning
            .chars()
            .map(|c| {
                if c.is_ascii_graphic() || c == ' ' {
                    c
                } else {
                    '?'
                }
            })
            .collect::<String>();
        let line = format!("X-Octessera-Export-Warning: {printable}\r\n");
        header.splice(blank_line..blank_line, line.into_bytes());
    }
    let skipped = plan.warnings.len();
    if skipped > 0 {
        if let Ok(mut state) = inner.state.lock() {
            let noun = if skipped == 1 { "item" } else { "items" };
            queue_runtime_status(
                &mut state,
                RuntimeStoreResult::UserDataExportStatus {
                    message: format!("Export: {skipped} {noun} skipped"),
                },
            );
        }
    }
    if stream.write_all(&header).is_err() {
        return;
    }
    let _ = user_data_archive::write_archive(
        &plan,
        &mut StoppableStream {
            stream,
            stop: &inner.stop,
        },
    );
}

struct StoppableStream<'a> {
    stream: &'a mut TcpStream,
    stop: &'a AtomicBool,
}

impl Write for StoppableStream<'_> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if self.stop.load(Ordering::Acquire) {
            return Err(io::Error::other("user-data transfer closed"));
        }
        self.stream.write(buffer)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stream.flush()
    }
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
    let Some(transfer_code) = restore_slot_code(inner) else {
        respond(
            stream,
            409,
            "application/json",
            br#"{"error":"restore_not_available"}"#,
        );
        return;
    };
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
    let upload_path = inner
        .store_dir
        .join(format!("{TRANSFER_TEMP_PREFIX}upload-{session}"));
    let stage_root = inner
        .store_dir
        .join(format!("{TRANSFER_TEMP_PREFIX}stage-{session}"));
    let result = (|| {
        fs::create_dir_all(&inner.store_dir).map_err(io_error)?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&upload_path)
            .map_err(io_error)?;
        copy_request_body(stream, &mut file, length, &inner.stop)?;
        file.sync_all().map_err(io_error)?;
        let _staging = inner
            .stage_gate
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        user_data_archive::stage_archive(&upload_path, &stage_root)
    })();
    let _ = fs::remove_file(&upload_path);
    let staged = match result {
        Ok(staged) => staged,
        Err(ArchiveError::StorageFull) => {
            let _ = fs::remove_dir_all(&stage_root);
            respond(
                stream,
                507,
                "application/json",
                br#"{"error":"insufficient_space"}"#,
            );
            return;
        }
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
    if validate_free_space(
        &crate::user_data_restore::LiveFilesystems,
        &inner.store_dir,
        &inner.samples_dir,
        &inner.recordings_dir,
        &inner.screen_recordings_dir,
        length,
    )
    .is_err()
    {
        super::remove_stage(&staged);
        respond(
            stream,
            507,
            "application/json",
            br#"{"error":"insufficient_space"}"#,
        );
        return;
    }
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
        if inner.stop.load(Ordering::Acquire)
            || !state.active
            || state.code.as_ref() != Some(&transfer_code)
        {
            drop(state);
            super::remove_stage(&staged);
            respond(
                stream,
                410,
                "application/json",
                br#"{"error":"transfer_closed"}"#,
            );
            return;
        }
        state.restore = RestoreState::Pending(Box::new(PendingRestore {
            session: session.clone(),
            staged,
            expires_at: Instant::now() + RESTORE_CONFIRM_LIFETIME,
        }));
        super::queue_transfer_status(&mut state);
        json!({"status":"confirmation_required","session":session})
    };
    let body =
        serde_json::to_vec(&response).unwrap_or_else(|_| b"{\"error\":\"unavailable\"}".to_vec());
    respond(stream, 202, "application/json", &body);
}

fn restore_slot_code(inner: &Arc<TransferInner>) -> Option<String> {
    let state = inner.state.lock().ok()?;
    let available = state.active
        && matches!(
            &state.restore,
            RestoreState::None | RestoreState::Finished { .. }
        );
    available.then(|| state.code.clone()).flatten()
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
            drop(state);
            if let Some(pending) = super::revoke_inner(inner) {
                super::remove_stage(&pending.staged);
            }
            return false;
        }
    }
    valid
}

fn peer_allowed(inner: &Arc<TransferInner>, peer: SocketAddr) -> bool {
    if inner.config.loopback_peer {
        return peer.ip() == IpAddr::V4(Ipv4Addr::LOCALHOST);
    }
    inner
        .state
        .lock()
        .ok()
        .and_then(|state| state.network)
        .is_some_and(|network| matches!(peer.ip(), IpAddr::V4(ip) if network.contains(ip)))
}

fn expected_host(inner: &Arc<TransferInner>) -> String {
    inner
        .state
        .lock()
        .ok()
        .and_then(|state| state.endpoint.zip(state.network))
        .map(|(endpoint, network)| format!("{}:{}", network.address, endpoint.port()))
        .unwrap_or_else(|| format!("0.0.0.0:{TRANSFER_PORT}"))
}

fn random_session(random: &RandomSource) -> Result<String, String> {
    random_alphabet_code(random, 16)
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
