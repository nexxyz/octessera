use playback_runtime::RuntimeStoreResult;
use std::io;
use std::process::{Command, Output};

#[cfg(board_orange)]
mod orange_update_broker;
#[cfg(board_not_orange)]
mod raspberry_update_command;
#[cfg(board_orange)]
pub(super) use orange_update_broker::production_executor;
#[cfg(board_orange)]
use orange_update_broker::update_command;
#[cfg(board_not_orange)]
pub(super) use raspberry_update_command::production_executor;
#[cfg(board_not_orange)]
use raspberry_update_command::update_command;

const MAX_UPDATE_OUTPUT_CHARS: usize = 512;

pub(super) trait UpdateExecutor: Send + Sync {
    fn output(&self, command: &mut Command) -> io::Result<Output>;
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
        format!("Update {action} started")
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
                if message == "Update apply started"
        ));
        assert!(matches!(
            report("check", false, b"", b""),
            RuntimeStoreResult::DeviceUpdateStatus { ok: false, message }
                if message == "Update check failed"
        ));
    }
}
