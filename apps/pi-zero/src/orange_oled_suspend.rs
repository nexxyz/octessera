use crate::render_loop::RenderWorker;
#[cfg(unix)]
mod unix {
    use super::*;
    use crate::orange_oled_suspend_policy::{TransactionAction, TransactionPolicy};
    use serde_json::Value;
    use std::fs;
    use std::io::{Read, Write};
    use std::os::fd::AsRawFd;
    use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::thread::{self, JoinHandle};
    use std::time::{Duration, Instant};

    pub(crate) const SOCKET_PATH: &str = "/run/octessera/oled-suspend.sock";
    const SCHEMA: u64 = 1;
    const MAX_MESSAGE_BYTES: usize = 1024;
    const IO_TIMEOUT: Duration = Duration::from_secs(2);
    const POLL_INTERVAL: Duration = Duration::from_millis(25);

    pub(crate) struct OrangeOledSuspendCoordinator {
        stop: Arc<AtomicBool>,
        thread: Option<JoinHandle<()>>,
    }

    impl OrangeOledSuspendCoordinator {
        pub(crate) fn spawn(render: RenderWorker) -> Result<Self, String> {
            let socket_path = PathBuf::from(SOCKET_PATH);
            let listener = bind_socket(&socket_path)?;
            listener
                .set_nonblocking(true)
                .map_err(|error| format!("cannot make OLED suspend socket nonblocking: {error}"))?;
            let stop = Arc::new(AtomicBool::new(false));
            let thread_stop = Arc::clone(&stop);
            let thread_socket_path = socket_path.clone();
            let thread = thread::Builder::new()
                .name("orange-oled-suspend".into())
                .spawn(move || run_server(listener, thread_stop, thread_socket_path, render))
                .map_err(|error| format!("cannot start OLED suspend coordinator: {error}"))?;
            Ok(Self {
                stop,
                thread: Some(thread),
            })
        }

        pub(crate) fn shutdown(mut self) -> Result<(), String> {
            self.stop.store(true, Ordering::Release);
            let thread = self
                .thread
                .take()
                .ok_or_else(|| "OLED suspend coordinator was already stopped".to_string())?;
            thread
                .join()
                .map_err(|_| "OLED suspend coordinator panicked".to_string())?;
            Ok(())
        }
    }

    fn bind_socket(path: &Path) -> Result<UnixListener, String> {
        if let Ok(metadata) = fs::symlink_metadata(path) {
            if !metadata.file_type().is_socket()
                || metadata.uid() != unsafe { libc::geteuid() }
                || metadata.gid() != unsafe { libc::getegid() }
                || metadata.mode() & 0o7777 != 0o600
            {
                return Err("OLED suspend socket has unsafe stale metadata".into());
            }
            if UnixStream::connect(path).is_ok() {
                return Err("OLED suspend socket is already owned by a live runtime".into());
            }
            fs::remove_file(path)
                .map_err(|error| format!("cannot remove stale OLED suspend socket: {error}"))?;
        }
        let listener = UnixListener::bind(path)
            .map_err(|error| format!("cannot bind OLED suspend socket: {error}"))?;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
            .map_err(|error| format!("cannot set OLED suspend socket mode: {error}"))?;
        Ok(listener)
    }

    fn run_server(
        listener: UnixListener,
        stop: Arc<AtomicBool>,
        socket_path: PathBuf,
        render: RenderWorker,
    ) {
        let mut transaction = TransactionPolicy::default();
        while !stop.load(Ordering::Acquire) {
            if let Some(error) = transaction.rollback_if_due(Instant::now(), &render) {
                eprintln!("Orange OLED suspend watchdog rollback failed: {error}");
            }
            match listener.accept() {
                Ok((stream, _)) => {
                    if let Err(error) = handle_connection(stream, &render, &mut transaction) {
                        eprintln!("Orange OLED suspend request rejected: {error}");
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(POLL_INTERVAL);
                }
                Err(error) => {
                    eprintln!("Orange OLED suspend listener failed: {error}");
                    break;
                }
            }
        }
        if let Some(error) = transaction.rollback_now(&render) {
            eprintln!("Orange OLED suspend shutdown rollback failed: {error}");
        }
        if let Err(error) = fs::remove_file(socket_path) {
            if error.kind() != std::io::ErrorKind::NotFound {
                eprintln!("Orange OLED suspend socket cleanup failed: {error}");
            }
        }
    }

    fn handle_connection(
        mut stream: UnixStream,
        render: &RenderWorker,
        transaction: &mut TransactionPolicy,
    ) -> Result<(), String> {
        stream
            .set_read_timeout(Some(IO_TIMEOUT))
            .and_then(|()| stream.set_write_timeout(Some(IO_TIMEOUT)))
            .map_err(|error| format!("cannot bound OLED suspend socket I/O: {error}"))?;
        validate_peer(&stream)?;
        let request = read_request(&mut stream)?;
        let boot_id = current_boot_id()?;
        let outcome = match process_request(&request, &boot_id, render, transaction) {
            Ok(outcome) => outcome,
            Err(error) => {
                let response = response(&request, &boot_id, false, Some(error));
                return write_response(&mut stream, &response);
            }
        };
        let response = response(&request, &boot_id, true, None);
        write_response_with_rollback(&mut stream, &response, outcome, render, transaction)
    }

    fn write_response(stream: &mut UnixStream, response: &Value) -> Result<(), String> {
        let bytes = serde_json::to_vec(&response).map_err(|error| error.to_string())?;
        if bytes.len() + 1 > MAX_MESSAGE_BYTES {
            return Err("OLED suspend response exceeded the message limit".into());
        }
        stream
            .write_all(&[bytes.as_slice(), b"\n"].concat())
            .map_err(|error| format!("cannot write OLED suspend response: {error}"))
    }

    fn write_response_with_rollback(
        stream: &mut UnixStream,
        response: &Value,
        outcome: crate::orange_oled_suspend_policy::ProcessOutcome,
        render: &RenderWorker,
        transaction: &mut TransactionPolicy,
    ) -> Result<(), String> {
        match write_response(stream, response) {
            Ok(()) => Ok(()),
            Err(error) => {
                if let Some(rollback_error) =
                    transaction.rollback_after_response_failure(outcome, Instant::now(), render)
                {
                    return Err(format!(
                        "{error}; response-failure rollback failed: {rollback_error}"
                    ));
                }
                Err(error)
            }
        }
    }

    fn validate_peer(stream: &UnixStream) -> Result<(), String> {
        #[cfg(target_os = "linux")]
        {
            let mut credential = unsafe { std::mem::zeroed::<libc::ucred>() };
            let mut length = std::mem::size_of_val(&credential) as libc::socklen_t;
            let result = unsafe {
                libc::getsockopt(
                    stream.as_raw_fd(),
                    libc::SOL_SOCKET,
                    libc::SO_PEERCRED,
                    (&mut credential as *mut libc::ucred).cast(),
                    &mut length,
                )
            };
            if result != 0
                || length != std::mem::size_of::<libc::ucred>() as libc::socklen_t
                || credential.uid != unsafe { libc::geteuid() }
                || credential.gid != unsafe { libc::getegid() }
            {
                return Err("OLED suspend socket peer is not the runtime user".into());
            }
            Ok(())
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = stream;
            Err("OLED suspend peer credentials require Linux".into())
        }
    }

    fn read_request(stream: &mut UnixStream) -> Result<Value, String> {
        let mut bytes = Vec::with_capacity(MAX_MESSAGE_BYTES + 1);
        stream
            .take((MAX_MESSAGE_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|error| format!("cannot read OLED suspend request: {error}"))?;
        if bytes.len() > MAX_MESSAGE_BYTES || !bytes.ends_with(b"\n") {
            return Err("OLED suspend request is oversized or missing its terminator".into());
        }
        bytes.pop();
        if bytes.contains(&b'\n') {
            return Err("OLED suspend request contains trailing data".into());
        }
        let value: Value = serde_json::from_slice(&bytes)
            .map_err(|error| format!("OLED suspend request is invalid JSON: {error}"))?;
        let object = value
            .as_object()
            .ok_or_else(|| "OLED suspend request must be an object".to_string())?;
        if object.len() != 4
            || object.get("schema").and_then(Value::as_u64) != Some(SCHEMA)
            || !object.get("action").is_some_and(Value::is_string)
            || !object.get("token").is_some_and(Value::is_string)
            || !object.get("bootId").is_some_and(Value::is_string)
        {
            return Err("OLED suspend request schema is not exact".into());
        }
        let action = object["action"].as_str().unwrap_or_default();
        let token = object["token"].as_str().unwrap_or_default();
        let boot_id = object["bootId"].as_str().unwrap_or_default();
        if TransactionAction::parse(action).is_none()
            || !valid_token(token)
            || !valid_boot_id(boot_id)
        {
            return Err("OLED suspend request has an invalid action, token, or boot ID".into());
        }
        Ok(value)
    }

    fn process_request(
        request: &Value,
        boot_id: &str,
        render: &RenderWorker,
        transaction: &mut TransactionPolicy,
    ) -> Result<crate::orange_oled_suspend_policy::ProcessOutcome, String> {
        let request_boot_id = request["bootId"].as_str().unwrap_or_default();
        if request_boot_id != boot_id {
            return Err("OLED suspend request belongs to another boot".into());
        }
        let action = TransactionAction::parse(request["action"].as_str().unwrap_or_default())
            .ok_or_else(|| "OLED suspend action is unsupported".to_string())?;
        let token = request["token"].as_str().unwrap_or_default();
        transaction.process(action, token, Instant::now(), render)
    }

    fn response(request: &Value, boot_id: &str, ok: bool, error: Option<String>) -> Value {
        serde_json::json!({
            "schema": SCHEMA,
            "action": request["action"],
            "token": request["token"],
            "bootId": boot_id,
            "ok": ok,
            "error": error,
        })
    }

    fn current_boot_id() -> Result<String, String> {
        let value = fs::read_to_string("/proc/sys/kernel/random/boot_id")
            .map_err(|error| format!("cannot read current boot ID: {error}"))?;
        let value = value.trim().to_string();
        if valid_boot_id(&value) {
            Ok(value)
        } else {
            Err("current boot ID is invalid".into())
        }
    }

    fn valid_token(value: &str) -> bool {
        value.len() == 32
            && value
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    }

    fn valid_boot_id(value: &str) -> bool {
        value.len() == 36
            && value.as_bytes()[8] == b'-'
            && value.as_bytes()[13] == b'-'
            && value.as_bytes()[18] == b'-'
            && value.as_bytes()[23] == b'-'
            && value.bytes().enumerate().all(|(index, byte)| {
                [8, 13, 18, 23].contains(&index)
                    || byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase()
            })
    }

    pub(crate) use OrangeOledSuspendCoordinator as Coordinator;

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn protocol_identifiers_are_strict() {
            assert!(valid_token("0123456789abcdef0123456789abcdef"));
            assert!(!valid_token("0123456789ABCDEF0123456789abcdef"));
            assert!(valid_boot_id("01234567-89ab-cdef-0123-456789abcdef"));
            assert!(!valid_boot_id("01234567-89ab-cdef-0123-456789abcde"));
            for action in [
                "prepare/release",
                "prepare/commit",
                "resume/release",
                "resume/complete",
                "rollback",
            ] {
                assert!(TransactionAction::parse(action).is_some());
            }
        }

        #[test]
        fn oversized_request_is_rejected_without_waiting_for_eof() {
            let (mut reader, mut writer) = UnixStream::pair().expect("pair");
            let (written_sender, written_receiver) = std::sync::mpsc::sync_channel(0);
            let (release_sender, release_receiver) = std::sync::mpsc::sync_channel(0);
            let writer_thread = thread::spawn(move || {
                let request = [b'x'; MAX_MESSAGE_BYTES + 1];
                writer.write_all(&request).expect("write request");
                written_sender.send(()).expect("signal request write");
                release_receiver.recv().expect("release writer");
            });
            written_receiver.recv().expect("request was written");

            let (result_sender, result_receiver) = std::sync::mpsc::sync_channel(0);
            let reader_thread = thread::spawn(move || {
                result_sender
                    .send(read_request(&mut reader))
                    .expect("send request result");
            });
            let result = match result_receiver.recv_timeout(Duration::from_millis(250)) {
                Ok(result) => result,
                Err(_) => {
                    release_sender.send(()).expect("release writer");
                    reader_thread.join().expect("reader thread");
                    writer_thread.join().expect("writer thread");
                    panic!("oversized request waited for EOF");
                }
            };
            release_sender.send(()).expect("release writer");
            reader_thread.join().expect("reader thread");
            writer_thread.join().expect("writer thread");

            let error = result.expect_err("oversized request was accepted");
            assert!(error.contains("oversized"), "unexpected error: {error}");
        }
    }
}

#[cfg(not(unix))]
mod non_unix {
    use super::*;

    pub(crate) struct Coordinator;

    impl Coordinator {
        pub(crate) fn spawn(_render: RenderWorker) -> Result<Self, String> {
            Err("Orange OLED suspend coordinator requires Unix sockets".into())
        }

        pub(crate) fn shutdown(self) -> Result<(), String> {
            let _ = self;
            Ok(())
        }
    }
}

#[cfg(not(unix))]
pub(crate) use non_unix::Coordinator as OrangeOledSuspendCoordinator;
#[cfg(unix)]
pub(crate) use unix::Coordinator as OrangeOledSuspendCoordinator;
