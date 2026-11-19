use super::UpdateExecutor;
#[cfg(unix)]
use super::MAX_UPDATE_OUTPUT_CHARS;
use std::io;
#[cfg(unix)]
use std::io::{Read, Write};
#[cfg(unix)]
use std::net::Shutdown;
#[cfg(unix)]
use std::os::unix::net::UnixStream;
use std::process::{Command, Output};
use std::sync::Arc;

const ORANGE_UPDATE_SOCKET: &str = "/run/octessera-update/update.sock";
#[cfg(unix)]
const MAX_BROKER_RESPONSE_BYTES: usize = MAX_UPDATE_OUTPUT_CHARS + 8;

struct OrangeUpdateExecutor;

impl UpdateExecutor for OrangeUpdateExecutor {
    fn output(&self, command: &mut Command) -> io::Result<Output> {
        #[cfg(unix)]
        {
            let mut arguments = command.get_args();
            let action = arguments
                .next()
                .map(|value| value.to_string_lossy().into_owned())
                .ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "missing update action")
                })?;
            if arguments.next().is_some()
                || !matches!(action.as_str(), "check" | "apply" | "rollback")
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "invalid update action",
                ));
            }
            let mut socket = UnixStream::connect(ORANGE_UPDATE_SOCKET)?;
            socket.write_all(action.as_bytes())?;
            socket.write_all(b"\n")?;
            socket.shutdown(Shutdown::Write)?;
            let mut response = Vec::new();
            socket
                .take((MAX_BROKER_RESPONSE_BYTES + 1) as u64)
                .read_to_end(&mut response)?;
            if response.len() > MAX_BROKER_RESPONSE_BYTES {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "update broker response is too large",
                ));
            }
            broker_output(&response)
        }
        #[cfg(not(unix))]
        {
            let _ = command;
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "Orange update broker requires Unix",
            ))
        }
    }
}

pub(crate) fn production_executor() -> Arc<dyn UpdateExecutor> {
    Arc::new(OrangeUpdateExecutor)
}

pub(super) fn update_command(action: &str) -> Command {
    let mut command = Command::new(ORANGE_UPDATE_SOCKET);
    command.arg(action);
    command
}

#[cfg(unix)]
fn broker_output(response: &[u8]) -> io::Result<Output> {
    let (success, payload) = if let Some(payload) = response.strip_prefix(b"ok\n") {
        (true, payload)
    } else if let Some(payload) = response.strip_prefix(b"error\n") {
        (false, payload)
    } else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "update broker response is malformed",
        ));
    };
    use std::os::unix::process::ExitStatusExt;
    Ok(Output {
        status: std::process::ExitStatus::from_raw(if success { 0 } else { 1 }),
        stdout: if success {
            payload.to_vec()
        } else {
            Vec::new()
        },
        stderr: if success {
            Vec::new()
        } else {
            payload.to_vec()
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_command_names_the_broker_socket_and_action() {
        let command = update_command("apply");
        assert_eq!(
            command.get_program().to_string_lossy(),
            ORANGE_UPDATE_SOCKET
        );
        assert_eq!(
            command
                .get_args()
                .map(|value| value.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            vec!["apply"]
        );
    }

    #[cfg(unix)]
    #[test]
    fn broker_response_preserves_status_and_bounded_payload() {
        let output = broker_output(b"error\nfailed\n").unwrap();
        assert!(!output.status.success());
        assert_eq!(output.stderr, b"failed\n");
        assert!(broker_output(b"unexpected").is_err());
    }
}
