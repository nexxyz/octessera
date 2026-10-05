use super::super::model::{CheckOutcome, CheckStatus};
use super::support::{
    command_record, outcome, outcome_with_content, read_small, run_systemctl, CommandResult,
};
use serde_json::Value;
use std::path::Path;

const ARTIFACT: &str = "03-readiness.txt";

pub(super) fn readiness_check(context: &super::CheckContext) -> CheckOutcome {
    let marker_path = Path::new(context.board.readiness_path);
    let marker_payload = match read_small(marker_path) {
        Ok(payload) => payload,
        Err(error) => return outcome(CheckStatus::Fail, &error, ARTIFACT),
    };
    let marker = match serde_json::from_str::<Value>(&marker_payload) {
        Ok(value) => value,
        Err(error) => {
            return outcome(
                CheckStatus::Fail,
                &format!("invalid readiness JSON: {error}"),
                ARTIFACT,
            )
        }
    };
    let systemd = run_systemctl(
        &[
            "show",
            context.board.service_unit,
            "--no-pager",
            "--property=MainPID,InvocationID",
        ],
        context.timeout,
    );
    let systemd_identity = match parse_systemd_identity(&systemd) {
        Ok(identity) => identity,
        Err(status) => {
            return outcome_with_content(
                status,
                "active systemd identity could not be established",
                ARTIFACT,
                &format!(
                    "marker={marker_payload}\nsystemd={}",
                    command_record(&systemd)
                ),
            )
        }
    };
    let marker_pid = marker.get("pid").and_then(Value::as_u64).unwrap_or(0);
    let marker_invocation = marker
        .get("systemd_invocation_id")
        .and_then(Value::as_str)
        .unwrap_or("");
    if marker.get("schema_version") != Some(&Value::from(1))
        || marker.get("board_profile").and_then(Value::as_str) != Some(context.board.profile_id)
        || marker_pid == 0
        || marker_invocation.is_empty()
    {
        return outcome_with_content(
            CheckStatus::Fail,
            "readiness marker has the wrong contract",
            ARTIFACT,
            &format!(
                "marker={marker_payload}\nsystemd={}",
                command_record(&systemd)
            ),
        );
    }
    if !readiness_matches_systemd_identity(
        marker_pid,
        marker_invocation,
        systemd_identity.main_pid,
        &systemd_identity.invocation_id,
    ) {
        return outcome_with_content(
            CheckStatus::NotRun,
            "readiness marker is stale or does not match the active systemd invocation",
            ARTIFACT,
            &format!(
                "marker={marker_payload}\nsystemd={}",
                command_record(&systemd)
            ),
        );
    }
    outcome_with_content(
        CheckStatus::Pass,
        &format!("runtime readiness marker is current for pid {marker_pid}"),
        ARTIFACT,
        &format!(
            "marker={marker_payload}\nsystemd={}",
            command_record(&systemd)
        ),
    )
}

#[derive(Debug, Eq, PartialEq)]
struct SystemdIdentity {
    main_pid: u32,
    invocation_id: String,
}

fn parse_systemd_identity(result: &CommandResult) -> Result<SystemdIdentity, CheckStatus> {
    let output = match result {
        CommandResult::Completed(output) if output.status.success() => output,
        CommandResult::TimedOut => return Err(CheckStatus::Timeout),
        CommandResult::Completed(_) | CommandResult::SpawnFailed(_) => {
            return Err(CheckStatus::NotRun)
        }
    };
    let mut main_pid = None;
    let mut invocation_id = None;
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let Some((key, value)) = line.split_once('=') else {
            return Err(CheckStatus::NotRun);
        };
        match key {
            "MainPID" if main_pid.is_none() => {
                main_pid = value.parse::<u32>().ok();
            }
            "InvocationID" if invocation_id.is_none() => {
                invocation_id = Some(value.to_string());
            }
            _ => return Err(CheckStatus::NotRun),
        }
    }
    match (main_pid, invocation_id) {
        (Some(main_pid), Some(invocation_id)) if main_pid > 0 && !invocation_id.is_empty() => {
            Ok(SystemdIdentity {
                main_pid,
                invocation_id,
            })
        }
        _ => Err(CheckStatus::NotRun),
    }
}

fn readiness_matches_systemd_identity(
    marker_pid: u64,
    marker_invocation: &str,
    main_pid: u32,
    invocation_id: &str,
) -> bool {
    marker_pid == u64::from(main_pid) && marker_invocation == invocation_id
}

#[cfg(test)]
mod tests {
    use super::super::support::CommandResult;
    use super::{parse_systemd_identity, readiness_matches_systemd_identity};
    use std::process::Output;

    fn systemd_output(text: &str) -> CommandResult {
        CommandResult::Completed(Output {
            status: success_status(),
            stdout: text.as_bytes().to_vec(),
            stderr: Vec::new(),
        })
    }

    #[cfg(unix)]
    fn success_status() -> std::process::ExitStatus {
        use std::os::unix::process::ExitStatusExt;
        std::process::ExitStatus::from_raw(0)
    }

    #[cfg(windows)]
    fn success_status() -> std::process::ExitStatus {
        use std::os::windows::process::ExitStatusExt;
        std::process::ExitStatus::from_raw(0)
    }

    #[test]
    fn systemd_identity_requires_the_allowlisted_properties() {
        assert_eq!(
            parse_systemd_identity(&systemd_output("MainPID=123\nInvocationID=abc\n")),
            Ok(super::SystemdIdentity {
                main_pid: 123,
                invocation_id: "abc".into(),
            })
        );
        assert_eq!(
            parse_systemd_identity(&systemd_output("MainPID=123\nActiveState=active\n")),
            Err(super::CheckStatus::NotRun)
        );
    }

    #[test]
    fn stale_pid_or_invocation_is_not_current_readiness() {
        assert!(readiness_matches_systemd_identity(123, "abc", 123, "abc"));
        assert!(!readiness_matches_systemd_identity(122, "abc", 123, "abc"));
        assert!(!readiness_matches_systemd_identity(123, "old", 123, "abc"));
    }
}
