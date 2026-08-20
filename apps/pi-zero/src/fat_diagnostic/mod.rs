mod checks;
mod evidence;
mod model;

use checks::{run_check, CheckContext};
use evidence::{format_check_log, sanitize_text, EvidenceWriter};
use model::{
    CheckStatus, DiagnosticStatus, EvidenceCheck, EvidenceReport, OperatorObservation, CHECK_ORDER,
};
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const DEFAULT_TIMEOUT_SECONDS: u64 = 30;
const MAX_TIMEOUT_SECONDS: u64 = 600;

#[derive(Debug)]
pub(crate) struct DiagnosticOptions {
    board_profile: String,
    evidence_dir: PathBuf,
    timeout_seconds: u64,
}

pub(crate) fn run() -> Result<bool, String> {
    let options = parse_args(std::env::args().skip(1).collect())?;
    run_with_options(options)
}

pub(crate) fn run_legacy_raspberry() -> Result<bool, String> {
    let evidence_dir = std::env::temp_dir().join(format!(
        "octessera-fat-diagnostic-legacy-{}-{}",
        std::process::id(),
        unix_nanos()
    ));
    run_with_options(DiagnosticOptions {
        board_profile: "raspberry-pi-zero-2w".into(),
        evidence_dir,
        timeout_seconds: DEFAULT_TIMEOUT_SECONDS,
    })
}

fn run_with_options(options: DiagnosticOptions) -> Result<bool, String> {
    crate::board_profile::validate_fat_diagnostic_profile(&options.board_profile)?;
    let board = crate::board_profile::fat_diagnostic_board(&options.board_profile)
        .ok_or_else(|| format!("unknown fixed board profile: {}", options.board_profile))?;
    if board.profile_id != crate::board_profile::BOARD_PROFILE_ID {
        return Err(format!(
            "diagnostic profile {} does not match this binary profile {}",
            board.profile_id,
            crate::board_profile::BOARD_PROFILE_ID
        ));
    }
    let writer = EvidenceWriter::new(&options.evidence_dir)?;
    let started = unix_seconds();
    let context = CheckContext {
        board,
        timeout: Duration::from_secs(options.timeout_seconds),
        executable: std::env::current_exe().ok(),
    };
    let mut checks = Vec::new();
    for id in CHECK_ORDER {
        let check_started = Instant::now();
        let outcome = run_check(*id, &context);
        let artifact = match writer.write_artifact(&outcome.artifact, &outcome.artifact_content) {
            Ok(artifact) => artifact,
            Err(error) => {
                checks.push(EvidenceCheck {
                    id: id.as_str().into(),
                    status: CheckStatus::Fail,
                    elapsed_ms: check_started.elapsed().as_millis(),
                    message: sanitize_text(&error),
                    artifact: outcome.artifact,
                });
                continue;
            }
        };
        checks.push(EvidenceCheck {
            id: id.as_str().into(),
            status: outcome.status,
            elapsed_ms: check_started.elapsed().as_millis(),
            message: sanitize_text(&outcome.message),
            artifact,
        });
    }
    let operator_observations = operator_observations();
    let automated_pass = checks
        .iter()
        .all(|check| !check.status.is_automated_failure());
    let overall_status = DiagnosticStatus::from_checks(&checks);
    writer.write_artifact("fat-diagnostic.log", &format_check_log(&checks))?;
    let report = EvidenceReport {
        schema_version: 1,
        board_profile: board.profile_id.into(),
        compiled_board_profile: crate::board_profile::BOARD_PROFILE_ID.into(),
        timeout_seconds: options.timeout_seconds,
        started_unix_seconds: started,
        finished_unix_seconds: unix_seconds(),
        automated_pass,
        overall_status,
        operator_observations_pending: true,
        checks,
        operator_observations,
    };
    writer.write_report(&report)?;
    println!(
        "FAT diagnostic evidence: {}\nAUTOMATED_PASS={}\nOVERALL_STATUS={}\nOPERATOR_OBSERVATIONS_REQUIRED=true",
        writer.root().display(),
        automated_pass,
        overall_status.as_str()
    );
    Ok(automated_pass)
}

fn parse_args(args: Vec<String>) -> Result<DiagnosticOptions, String> {
    let mut board_profile = None;
    let mut evidence_dir = PathBuf::from("/tmp/octessera-fat-diagnostic");
    let mut timeout_seconds = DEFAULT_TIMEOUT_SECONDS;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--fat-diagnostic" | "--diagnostic" => index += 1,
            "--help" | "-h" => {
                print_help();
                return Err("help requested".into());
            }
            "--board-profile" | "--profile" => {
                board_profile = Some(next_value(&args, index, "--board-profile")?);
                index += 2;
            }
            "--evidence-dir" => {
                evidence_dir = PathBuf::from(next_value(&args, index, "--evidence-dir")?);
                index += 2;
            }
            "--timeout-seconds" => {
                let value = next_value(&args, index, "--timeout-seconds")?;
                timeout_seconds = value
                    .parse::<u64>()
                    .map_err(|_| "--timeout-seconds must be an integer".to_string())?;
                index += 2;
            }
            other => return Err(format!("unknown FAT diagnostic argument: {other}")),
        }
    }
    let board_profile = board_profile.ok_or_else(|| {
        "--board-profile is required; choose raspberry-pi-zero-2w or orange-pi-zero-2w".to_string()
    })?;
    if !(1..=MAX_TIMEOUT_SECONDS).contains(&timeout_seconds) {
        return Err(format!(
            "--timeout-seconds must be between 1 and {MAX_TIMEOUT_SECONDS}"
        ));
    }
    if evidence_dir.as_os_str().is_empty() {
        return Err("--evidence-dir must not be empty".into());
    }
    Ok(DiagnosticOptions {
        board_profile,
        evidence_dir,
        timeout_seconds,
    })
}

fn next_value(args: &[String], index: usize, name: &str) -> Result<String, String> {
    let value = args
        .get(index + 1)
        .ok_or_else(|| format!("{name} requires a value"))?;
    if value.is_empty() || value.contains('\0') || value.chars().any(|c| c == '\r' || c == '\n') {
        return Err(format!("{name} value is empty or contains a line break"));
    }
    Ok(value.clone())
}

fn operator_observations() -> Vec<OperatorObservation> {
    vec![
        OperatorObservation {
            id: "oled_visual",
            status: CheckStatus::OperatorRequired,
            instruction: "Confirm the OLED is readable, stable, and shows the normal native menu.",
        },
        OperatorObservation {
            id: "audio_audible",
            status: CheckStatus::OperatorRequired,
            instruction: "With safe volume, run the documented route tone or patch and confirm audible sound.",
        },
        OperatorObservation {
            id: "physical_inputs",
            status: CheckStatus::OperatorRequired,
            instruction: "Press NeoKeys/grid cells and turn/click every encoder; record actual events and orientation.",
        },
        OperatorObservation {
            id: "usb_port_role",
            status: CheckStatus::OperatorRequired,
            instruction: "Confirm the authorized USB port role, cable direction, VBUS/CC behavior, and no-backfeed path.",
        },
        OperatorObservation {
            id: "board_and_enclosure",
            status: CheckStatus::OperatorRequired,
            instruction: "Confirm board revision, wiring, connector fit, and enclosure fit without force.",
        },
    ]
}

fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn unix_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0)
}

fn print_help() {
    println!(
        "Usage: octessera-pi --fat-diagnostic --board-profile <profile> [OPTIONS]\n\n\
Safe, non-destructive FAT evidence checks for the two fixed board profiles.\n\
  --board-profile <id>  Required fixed profile: raspberry-pi-zero-2w or orange-pi-zero-2w\n\
  --evidence-dir <path> New evidence directory (default: /tmp/octessera-fat-diagnostic)\n\
  --timeout-seconds <n> Per-command timeout, 1..600 (default: 30)\n\
  --diagnostic           Deprecated Raspberry compatibility alias when no profile is supplied\n\
No flashing, reboot, restore, gadget binding, tone playback, or visual qualification is performed."
    );
}

#[cfg(test)]
mod tests {
    use super::parse_args;
    use std::path::PathBuf;

    #[test]
    fn profile_selection_requires_an_explicit_fixed_profile() {
        assert!(parse_args(vec!["--fat-diagnostic".into()]).is_err());
        let options = parse_args(vec![
            "--diagnostic".into(),
            "--board-profile".into(),
            "orange-pi-zero-2w".into(),
        ])
        .unwrap();
        assert_eq!(options.board_profile, "orange-pi-zero-2w");
        assert_eq!(options.timeout_seconds, 30);
    }

    #[test]
    fn timeout_is_bounded_and_input_scan_is_not_an_option() {
        let options = parse_args(vec![
            "--fat-diagnostic".into(),
            "--profile".into(),
            "raspberry-pi-zero-2w".into(),
            "--timeout-seconds".into(),
            "60".into(),
            "--evidence-dir".into(),
            "evidence".into(),
        ])
        .unwrap();
        assert_eq!(options.timeout_seconds, 60);
        assert_eq!(options.evidence_dir, PathBuf::from("evidence"));
        assert!(parse_args(vec![
            "--fat-diagnostic".into(),
            "--profile".into(),
            "raspberry-pi-zero-2w".into(),
            "--scan-inputs".into(),
        ])
        .is_err());
        assert!(parse_args(vec![
            "--fat-diagnostic".into(),
            "--profile".into(),
            "raspberry-pi-zero-2w".into(),
            "--timeout-seconds".into(),
            "601".into(),
        ])
        .is_err());
    }
}
