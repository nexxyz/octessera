use super::super::model::{CheckOutcome, CheckStatus};
use super::CheckContext;
use serde_json::Value;
use std::fs;
use std::io::Read;
use std::path::Path;
use std::process::{Child, Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const UTILITY_MODE_ENVIRONMENT: &[&str] = &[
    "OCTESSERA_PI_DIAGNOSTIC",
    "OCTESSERA_PI_HARDWARE_TEST",
    "OCTESSERA_PI_HARDWARE_NOISE_TEST",
    "OCTESSERA_PI_PROFILE_DSP",
    "OCTESSERA_PI_TIMING_PROBE",
    "OCTESSERA_PI_TIMING_PROBE_DURATIONS",
    "OCTESSERA_PI_TIMING_PROBE_SCENARIOS",
    "OCTESSERA_PI_TIMING_PROBE_CONFIG",
    "OCTESSERA_PI_TIMING_PROBE_RUNTIME_ONLY",
    "OCTESSERA_PI_TIMING_PROBE_AUDIO_DRAIN",
    "OCTESSERA_PI_TIMING_PROBE_AUDIO_DRAIN_INTERVAL_MS",
];

pub(super) fn artifact_check(context: &CheckContext) -> CheckOutcome {
    let mut artifact = String::new();
    for path in [context.board.profile_contract_path] {
        match read_small(Path::new(path)) {
            Ok(value) => artifact.push_str(&format!("== {path} ==\n{value}\n")),
            Err(error) => {
                return outcome(
                    CheckStatus::Fail,
                    &format!("mandatory artifact/log collection failed: {error}"),
                    "10-artifacts.txt",
                )
            }
        }
    }
    if let Ok(value) = read_small(Path::new("/etc/armbian-release")) {
        artifact.push_str(&format!("== /etc/armbian-release ==\n{value}\n"));
    }
    artifact.push_str(&format!(
        "== fixed paths ==\ni2c={}\nspi={}\nstore={}\nsamples={}\nreadiness={}\n",
        context.board.i2c_path,
        context.board.spi_path,
        context.board.store_dir,
        context.board.samples_dir,
        context.board.readiness_path
    ));
    outcome_with_content(
        CheckStatus::Pass,
        "safe identity and path artifacts collected",
        "10-artifacts.txt",
        &artifact,
    )
}

pub(super) fn service_check(context: &CheckContext) -> CheckOutcome {
    let enabled = run_systemctl(&["is-enabled", context.board.service_unit], context.timeout);
    let active = run_systemctl(&["is-active", context.board.service_unit], context.timeout);
    let details = run_systemctl(
        &[
            "show",
            context.board.service_unit,
            "--property=ActiveState,SubState,ExecMainPID,User,UnitFileState",
        ],
        context.timeout,
    );
    let artifact = format!(
        "enabled={}\nactive={}\nstructured_status={}\nexpected_user={}\n",
        command_record(&enabled),
        command_record(&active),
        command_record(&details),
        context.board.service_user
    );
    if [&enabled, &active, &details]
        .iter()
        .any(|result| matches!(result, CommandResult::TimedOut))
    {
        return outcome_with_content(
            CheckStatus::Timeout,
            "service state command timed out",
            "02-service.txt",
            &artifact,
        );
    }
    if [&enabled, &active, &details]
        .iter()
        .any(|result| required_command_status(result) != CheckStatus::Pass)
    {
        return outcome_with_content(
            CheckStatus::Fail,
            "runtime service state or mandatory structured status collection failed",
            "02-service.txt",
            &artifact,
        );
    }
    let expected_user = format!("User={}", context.board.service_user);
    let details_match = matches!(&details, CommandResult::Completed(output) if output_text(output).contains(&expected_user));
    if !details_match {
        return outcome_with_content(
            CheckStatus::Fail,
            "runtime service user does not match the fixed board contract",
            "02-service.txt",
            &artifact,
        );
    }
    outcome_with_content(
        CheckStatus::Pass,
        "runtime service is enabled and active",
        "02-service.txt",
        &artifact,
    )
}

pub(super) fn validate_json_object(path: &Path, required: &[&str]) -> Result<(), String> {
    let payload = read_small(path)?;
    let value = serde_json::from_str::<Value>(&payload)
        .map_err(|error| format!("invalid JSON at {}: {error}", path.display()))?;
    let Some(object) = value.as_object() else {
        return Err(format!("JSON at {} is not an object", path.display()));
    };
    if required.iter().any(|key| !object.contains_key(*key)) {
        return Err(format!(
            "JSON at {} is missing a required field",
            path.display()
        ));
    }
    Ok(())
}

pub(super) fn has_assignment(content: &str, key: &str, expected: &str) -> bool {
    content.lines().any(|line| {
        let Some((actual_key, actual_value)) = line.split_once('=') else {
            return false;
        };
        actual_key.trim() == key && actual_value.trim() == expected
    })
}

pub(super) fn read_small(path: &Path) -> Result<String, String> {
    let metadata =
        fs::symlink_metadata(path).map_err(|error| format!("{}: {error}", path.display()))?;
    if metadata.file_type().is_symlink() {
        return Err(format!("{} is a symlink", path.display()));
    }
    if !metadata.is_file() {
        return Err(format!("{} is not a regular file", path.display()));
    }
    if metadata.len() > 128 * 1024 {
        return Err(format!("{} exceeds diagnostic read limit", path.display()));
    }
    fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))
}

pub(super) fn outcome(status: CheckStatus, message: &str, artifact: &str) -> CheckOutcome {
    CheckOutcome {
        status,
        message: message.into(),
        artifact: artifact.into(),
        artifact_content: message.into(),
    }
}

pub(super) fn outcome_with_content(
    status: CheckStatus,
    message: &str,
    artifact: &str,
    artifact_content: &str,
) -> CheckOutcome {
    CheckOutcome {
        status,
        message: message.into(),
        artifact: artifact.into(),
        artifact_content: artifact_content.into(),
    }
}

pub(super) fn run_systemctl(args: &[&str], timeout: Duration) -> CommandResult {
    run_command("systemctl", args, timeout)
}

pub(super) fn run_command(program: &str, args: &[&str], timeout: Duration) -> CommandResult {
    run_command_with_environment(program, args, timeout, false)
}

pub(super) fn run_metadata_command(
    program: &str,
    args: &[&str],
    timeout: Duration,
) -> CommandResult {
    run_command_with_environment(program, args, timeout, true)
}

fn run_command_with_environment(
    program: &str,
    args: &[&str],
    timeout: Duration,
    clear_utility_environment: bool,
) -> CommandResult {
    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if clear_utility_environment {
        for variable in UTILITY_MODE_ENVIRONMENT {
            command.env_remove(variable);
        }
    }
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => return CommandResult::SpawnFailed(format!("{program} unavailable: {error}")),
    };
    wait_for_command(&mut child, timeout)
}

pub(super) fn wait_for_command(child: &mut Child, timeout: Duration) -> CommandResult {
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                return CommandResult::Completed(collect_output(child, status));
            }
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return CommandResult::TimedOut;
            }
            Err(error) => {
                return CommandResult::SpawnFailed(format!("command wait failed: {error}"))
            }
        }
    }
}

fn collect_output(child: &mut Child, status: std::process::ExitStatus) -> Output {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    if let Some(mut pipe) = child.stdout.take() {
        let _ = pipe.read_to_end(&mut stdout);
    }
    if let Some(mut pipe) = child.stderr.take() {
        let _ = pipe.read_to_end(&mut stderr);
    }
    Output {
        status,
        stdout,
        stderr,
    }
}

pub(super) enum CommandResult {
    Completed(Output),
    TimedOut,
    SpawnFailed(String),
}

pub(super) fn command_succeeded(result: &CommandResult) -> bool {
    matches!(result, CommandResult::Completed(output) if output.status.success())
}

pub(super) fn required_command_status(result: &CommandResult) -> CheckStatus {
    match result {
        CommandResult::Completed(output) if output.status.success() => CheckStatus::Pass,
        CommandResult::TimedOut => CheckStatus::Timeout,
        CommandResult::Completed(_) | CommandResult::SpawnFailed(_) => CheckStatus::Fail,
    }
}

pub(super) fn command_record(result: &CommandResult) -> String {
    match result {
        CommandResult::Completed(output) => {
            format!("exit={} {}", output.status, output_text(output))
        }
        CommandResult::TimedOut => "timeout".into(),
        CommandResult::SpawnFailed(error) => format!("spawn_failed={error}"),
    }
}

pub(super) fn output_text(output: &Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    format!("stdout={} stderr={}", stdout.trim(), stderr.trim())
}

#[cfg(test)]
mod tests {
    use super::super::super::model::CheckStatus;
    use super::{
        command_succeeded, has_assignment, output_text, required_command_status, run_command,
        run_metadata_command, CommandResult,
    };
    use std::sync::Mutex;

    static ENVIRONMENT_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn timeout_is_not_successful_and_is_kept_distinct_from_spawn_failure() {
        assert!(!command_succeeded(&CommandResult::TimedOut));
        assert!(!command_succeeded(&CommandResult::SpawnFailed(
            "fixed command unavailable".into(),
        )));
    }

    #[test]
    fn failed_or_inactive_required_command_is_not_a_pass() {
        let result = if cfg!(windows) {
            run_command("cmd", &["/C", "exit 1"], std::time::Duration::from_secs(1))
        } else {
            run_command("false", &[], std::time::Duration::from_secs(1))
        };
        assert_eq!(required_command_status(&result), CheckStatus::Fail);
        assert_eq!(
            required_command_status(&CommandResult::TimedOut),
            CheckStatus::Timeout
        );
    }

    #[cfg(any(unix, windows))]
    use super::wait_for_command;
    #[cfg(any(unix, windows))]
    use std::process::{Command, Stdio};
    #[cfg(any(unix, windows))]
    use std::time::Duration;

    #[test]
    fn profile_assignment_requires_exact_value() {
        assert!(has_assignment(
            "OCTESSERA_BOARD_PROFILE_ID=orange-pi-zero-2w\n",
            "OCTESSERA_BOARD_PROFILE_ID",
            "orange-pi-zero-2w"
        ));
        assert!(!has_assignment(
            "OCTESSERA_BOARD_PROFILE_ID=orange-pi-zero-2w-extra\n",
            "OCTESSERA_BOARD_PROFILE_ID",
            "orange-pi-zero-2w"
        ));
    }

    #[test]
    fn metadata_command_clears_a_valid_diagnostic_environment() {
        let _guard = ENVIRONMENT_LOCK.lock().unwrap();
        let previous = std::env::var_os("OCTESSERA_PI_DIAGNOSTIC");
        let marker = std::env::temp_dir().join(format!(
            "octessera-metadata-recursion-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&marker);
        let marker = marker.to_string_lossy().into_owned();
        std::env::set_var("OCTESSERA_PI_DIAGNOSTIC", "1");
        let result = if cfg!(windows) {
            let command = format!(
                "if defined OCTESSERA_PI_DIAGNOSTIC (mkdir \"{marker}\" >nul 2>&1 & exit /b 1) else (echo metadata)"
            );
            run_metadata_command("cmd", &["/C", &command], Duration::from_secs(1))
        } else {
            let command = format!(
                "if [ -n \"$OCTESSERA_PI_DIAGNOSTIC\" ]; then mkdir -p '{marker}'; exit 1; else printf metadata; fi"
            );
            run_metadata_command("sh", &["-c", &command], Duration::from_secs(1))
        };
        match previous {
            Some(value) => std::env::set_var("OCTESSERA_PI_DIAGNOSTIC", value),
            None => std::env::remove_var("OCTESSERA_PI_DIAGNOSTIC"),
        }
        assert!(command_succeeded(&result));
        assert!(!std::path::Path::new(&marker).exists());
        let _ = std::fs::remove_dir_all(marker);
        if let CommandResult::Completed(output) = result {
            assert!(output_text(&output).contains("metadata"));
        }
    }

    #[cfg(unix)]
    #[test]
    fn command_timeout_is_classified_without_waiting_indefinitely() {
        let started = std::time::Instant::now();
        let mut child = Command::new("sh")
            .args(["-c", "sleep 2"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        assert!(matches!(
            wait_for_command(&mut child, Duration::from_millis(20)),
            CommandResult::TimedOut
        ));
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[cfg(windows)]
    #[test]
    fn command_timeout_is_classified_without_waiting_indefinitely() {
        let started = std::time::Instant::now();
        let mut child = Command::new("powershell")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "Start-Sleep -Seconds 2",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        assert!(matches!(
            wait_for_command(&mut child, Duration::from_millis(20)),
            CommandResult::TimedOut
        ));
        assert!(started.elapsed() < Duration::from_secs(1));
    }
}
