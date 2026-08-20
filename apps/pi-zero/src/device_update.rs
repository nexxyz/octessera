use playback_runtime::RuntimeStoreResult;
use std::io;
use std::process::{Command, Output};
use std::sync::Arc;

#[cfg(all(feature = "hardware-orange-pi-zero-2w", unix))]
use std::io::{Read, Write};
#[cfg(all(feature = "hardware-orange-pi-zero-2w", unix))]
use std::net::Shutdown;
#[cfg(all(feature = "hardware-orange-pi-zero-2w", unix))]
use std::os::unix::net::UnixStream;

const MAX_UPDATE_OUTPUT_CHARS: usize = 512;
#[cfg(feature = "hardware-orange-pi-zero-2w")]
const ORANGE_UPDATE_SOCKET: &str = "/run/octessera-update/update.sock";
#[cfg(all(feature = "hardware-orange-pi-zero-2w", unix))]
const MAX_BROKER_RESPONSE_BYTES: usize = MAX_UPDATE_OUTPUT_CHARS + 8;

pub(super) trait UpdateExecutor: Send + Sync {
    fn output(&self, command: &mut Command) -> io::Result<Output>;
}

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
struct CommandUpdateExecutor;

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
impl UpdateExecutor for CommandUpdateExecutor {
    fn output(&self, command: &mut Command) -> io::Result<Output> {
        command.output()
    }
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
struct OrangeUpdateExecutor;

#[cfg(feature = "hardware-orange-pi-zero-2w")]
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

pub(super) fn production_executor() -> Arc<dyn UpdateExecutor> {
    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    {
        Arc::new(OrangeUpdateExecutor)
    }
    #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
    {
        Arc::new(CommandUpdateExecutor)
    }
}

pub(super) fn run(action: &str, executor: &dyn UpdateExecutor) -> RuntimeStoreResult {
    let mut command = update_command(action);
    match executor.output(&mut command) {
        Ok(output) => report(
            action,
            output.status.success(),
            &output.stderr,
            &output.stdout,
        ),
        Err(error) => RuntimeStoreResult::DeviceUpdateStatus {
            ok: false,
            message: fallback_message(action, false, &format!("Update {action} failed: {error}")),
        },
    }
}

fn update_command(action: &str) -> Command {
    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    {
        let mut command = Command::new(ORANGE_UPDATE_SOCKET);
        command.arg(action);
        command
    }
    #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
    {
        let mut command = Command::new("sudo");
        command.args(["-n", "/usr/local/sbin/octessera-update", action]);
        command
    }
}

#[cfg(all(feature = "hardware-orange-pi-zero-2w", unix))]
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

fn report(action: &str, ok: bool, stderr: &[u8], stdout: &[u8]) -> RuntimeStoreResult {
    let message = if ok {
        bounded_sanitized_text(stdout)
    } else {
        bounded_sanitized_text(stderr).or_else(|| bounded_sanitized_text(stdout))
    }
    .unwrap_or_else(|| fallback_message(action, ok, ""));
    RuntimeStoreResult::DeviceUpdateStatus { ok, message }
}

fn fallback_message(action: &str, ok: bool, detail: &str) -> String {
    if !detail.is_empty() {
        if let Some(detail) = bounded_sanitized_text(detail.as_bytes()) {
            return detail;
        }
    }
    if ok && matches!(action, "apply" | "rollback") {
        format!("Update {action} health validation scheduled")
    } else if ok {
        format!("Update {action} completed")
    } else {
        format!("Update {action} failed")
    }
}

fn bounded_sanitized_text(output: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(output)
        .chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .take(MAX_UPDATE_OUTPUT_CHARS)
        .collect::<String>();
    let text = text.trim().to_string();
    (!text.is_empty()).then_some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_update_prefers_sanitized_bounded_stderr() {
        let result = report(
            "apply",
            false,
            format!("helper failed\n{}", "x".repeat(600)).as_bytes(),
            b"stdout fallback",
        );
        let RuntimeStoreResult::DeviceUpdateStatus { ok, message } = result else {
            panic!("expected update status");
        };
        assert!(!ok);
        assert!(message.starts_with("helper failed"));
        assert!(!message.chars().any(char::is_control));
        assert!(message.chars().count() <= MAX_UPDATE_OUTPUT_CHARS);
    }

    #[test]
    fn failed_update_uses_stdout_when_stderr_is_empty() {
        let result = report("check", false, b" \n\t", b"helper stdout\n");
        assert!(matches!(
            result,
            RuntimeStoreResult::DeviceUpdateStatus { ok: false, message }
                if message == "helper stdout"
        ));
    }

    #[test]
    fn update_status_uses_fallback_when_helper_has_no_output() {
        assert!(matches!(
            report("apply", true, b"", b""),
            RuntimeStoreResult::DeviceUpdateStatus { ok: true, message }
                if message == "Update apply health validation scheduled"
        ));
        assert!(matches!(
            report("check", false, b"", b""),
            RuntimeStoreResult::DeviceUpdateStatus { ok: false, message }
                if message == "Update check failed"
        ));
    }

    #[test]
    fn update_command_is_scoped_to_the_platform_lane() {
        let command = update_command("apply");
        #[cfg(feature = "hardware-orange-pi-zero-2w")]
        assert_eq!(
            command.get_program().to_string_lossy(),
            ORANGE_UPDATE_SOCKET
        );
        #[cfg(feature = "hardware-orange-pi-zero-2w")]
        assert_eq!(
            command
                .get_args()
                .map(|value| value.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            vec!["apply"]
        );
        #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
        assert_eq!(
            command
                .get_args()
                .map(|value| value.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            vec!["-n", "/usr/local/sbin/octessera-update", "apply"]
        );
    }

    #[cfg(all(feature = "hardware-orange-pi-zero-2w", unix))]
    #[test]
    fn broker_response_preserves_status_and_bounded_payload() {
        let output = broker_output(b"error\nfailed\n").unwrap();
        assert!(!output.status.success());
        assert_eq!(output.stderr, b"failed\n");
        assert!(broker_output(b"unexpected").is_err());
    }
}
