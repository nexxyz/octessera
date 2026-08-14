#[cfg(unix)]
use std::io::{Read, Write};
#[cfg(unix)]
use std::os::unix::net::UnixStream;
#[cfg(unix)]
use std::path::Path;
#[cfg(unix)]
use std::time::Duration;

#[cfg(unix)]
const REBOOT_SOCKET: &str = "/run/octessera-device-apply/reboot.sock";
#[cfg(unix)]
const REQUEST: &[u8] = b"reboot\n";
#[cfg(unix)]
const ACCEPTED: &[u8] = b"accepted\n";
#[cfg(unix)]
const REJECTED: &[u8] = b"rejected\n";
#[cfg(unix)]
const TIMEOUT: Duration = Duration::from_secs(2);
#[cfg(unix)]
const MAX_RESPONSE_BYTES: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum OrangeHelperOutcome {
    Accepted,
    Rejected,
    NotSubmitted,
    Indeterminate,
}

pub(crate) fn request_reboot() -> OrangeHelperOutcome {
    #[cfg(unix)]
    {
        request_reboot_at(Path::new(REBOOT_SOCKET))
    }
    #[cfg(not(unix))]
    {
        OrangeHelperOutcome::NotSubmitted
    }
}

#[cfg(unix)]
fn request_reboot_at(path: &Path) -> OrangeHelperOutcome {
    let mut stream = match UnixStream::connect(path) {
        Ok(stream) => stream,
        Err(_) => return OrangeHelperOutcome::NotSubmitted,
    };
    if stream
        .set_read_timeout(Some(TIMEOUT))
        .and_then(|_| stream.set_write_timeout(Some(TIMEOUT)))
        .is_err()
    {
        return OrangeHelperOutcome::NotSubmitted;
    }
    let mut submitted = 0;
    while submitted < REQUEST.len() {
        match stream.write(&REQUEST[submitted..]) {
            Ok(0) => return OrangeHelperOutcome::Indeterminate,
            Ok(bytes) => submitted += bytes,
            Err(_) if submitted == 0 => return OrangeHelperOutcome::NotSubmitted,
            Err(_) => return OrangeHelperOutcome::Indeterminate,
        }
    }
    if stream.shutdown(std::net::Shutdown::Write).is_err() {
        return OrangeHelperOutcome::Indeterminate;
    }
    let mut response = Vec::new();
    let mut buffer = [0; 8];
    loop {
        match stream.read(&mut buffer) {
            Ok(0) if response == ACCEPTED => return OrangeHelperOutcome::Accepted,
            Ok(0) if response == REJECTED => return OrangeHelperOutcome::Rejected,
            Ok(0) => return OrangeHelperOutcome::Indeterminate,
            Ok(bytes) if response.len() + bytes <= MAX_RESPONSE_BYTES => {
                response.extend_from_slice(&buffer[..bytes]);
            }
            Ok(_) => return OrangeHelperOutcome::Indeterminate,
            Err(_) => return OrangeHelperOutcome::Indeterminate,
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::net::UnixListener;
    use std::thread;

    fn socket_path(label: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("octessera-reboot-{label}-{}", std::process::id()))
    }

    fn serve(path: &Path, response: &'static [u8]) -> thread::JoinHandle<()> {
        let listener = UnixListener::bind(path).unwrap();
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            stream.read_to_end(&mut request).unwrap();
            assert_eq!(request, REQUEST);
            stream.write_all(response).unwrap();
        })
    }

    #[test]
    fn exact_request_and_ack_are_required() {
        let path = socket_path("accepted");
        let join = serve(&path, ACCEPTED);
        assert_eq!(request_reboot_at(&path), OrangeHelperOutcome::Accepted);
        join.join().unwrap();
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn exact_rejection_is_typed() {
        let path = socket_path("rejected");
        let join = serve(&path, REJECTED);
        assert_eq!(request_reboot_at(&path), OrangeHelperOutcome::Rejected);
        join.join().unwrap();
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn malformed_ack_and_timeout_are_indeterminate() {
        let path = socket_path("malformed");
        let join = serve(&path, b"accepted\nextra");
        assert_eq!(request_reboot_at(&path), OrangeHelperOutcome::Indeterminate);
        join.join().unwrap();
        let _ = std::fs::remove_file(path);

        let path = socket_path("timeout");
        let listener = UnixListener::bind(&path).unwrap();
        let join = thread::spawn(move || {
            let (_stream, _) = listener.accept().unwrap();
            thread::sleep(TIMEOUT + Duration::from_millis(50));
        });
        assert_eq!(request_reboot_at(&path), OrangeHelperOutcome::Indeterminate);
        join.join().unwrap();
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn missing_socket_is_not_submitted() {
        assert_eq!(
            request_reboot_at(Path::new("/tmp/octessera-missing-reboot.sock")),
            OrangeHelperOutcome::NotSubmitted
        );
    }
}
