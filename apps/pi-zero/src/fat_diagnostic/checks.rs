#[path = "readiness.rs"]
mod readiness;
#[path = "checks_support.rs"]
mod support;
use super::model::{CheckId, CheckOutcome, CheckStatus};
use crate::board_profile::FatDiagnosticBoard;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use support::{
    artifact_check, command_record, has_assignment, outcome, outcome_with_content, output_text,
    read_small, run_command, run_metadata_command, run_systemctl, service_check,
    validate_json_object, CommandResult,
};

pub(crate) struct CheckContext {
    pub(crate) board: FatDiagnosticBoard,
    pub(crate) timeout: Duration,
    pub(crate) executable: Option<PathBuf>,
}

pub(crate) fn run_check(id: CheckId, context: &CheckContext) -> CheckOutcome {
    match id {
        CheckId::Platform => platform_check(),
        CheckId::Identity => identity_check(context),
        CheckId::Service => service_check(context),
        CheckId::Readiness => readiness::readiness_check(context),
        CheckId::Storage => storage_check(context),
        CheckId::SetupStatus => setup_status_check(context),
        CheckId::OledHandoff => oled_handoff_check(context),
        CheckId::AudioRoute => audio_route_check(context),
        CheckId::InputApi => input_check(context),
        CheckId::UsbState => usb_state_check(context),
        CheckId::Artifacts => artifact_check(context),
    }
}

fn platform_check() -> CheckOutcome {
    if cfg!(target_os = "linux") {
        outcome(CheckStatus::Pass, "Linux host confirmed", "00-platform.txt")
    } else {
        outcome(
            CheckStatus::Fail,
            "Linux host required for board diagnostics",
            "00-platform.txt",
        )
    }
}

fn identity_check(context: &CheckContext) -> CheckOutcome {
    let model = match read_small(Path::new("/proc/device-tree/model")) {
        Ok(value) => value.replace('\0', ""),
        Err(error) => return outcome(CheckStatus::Fail, &error, "01-identity.txt"),
    };
    let profile_env = match read_small(Path::new(context.board.profile_contract_path)) {
        Ok(value) => value,
        Err(error) => return outcome(CheckStatus::Fail, &error, "01-identity.txt"),
    };
    let mut artifact = format!(
        "model={model}\nprofile_contract_path={}\nprofile_contract={profile_env}",
        context.board.profile_contract_path
    );
    if !model.contains(context.board.model_fragment) {
        return outcome(
            CheckStatus::Fail,
            &format!("model does not identify {}", context.board.model_fragment),
            "01-identity.txt",
        );
    }
    if !has_assignment(
        &profile_env,
        "OCTESSERA_BOARD_PROFILE_ID",
        context.board.profile_id,
    ) {
        return outcome(
            CheckStatus::Fail,
            &format!(
                "profile contract does not select {}",
                context.board.profile_id
            ),
            "01-identity.txt",
        );
    }
    if let Some(path) = context.executable.as_deref() {
        match run_metadata_command(
            path.to_string_lossy().as_ref(),
            &["--print-build-metadata"],
            context.timeout,
        ) {
            CommandResult::Completed(output) if output.status.success() => {
                let text = output_text(&output);
                artifact.push_str(&format!("\nbinary_metadata={text}"));
                if !text.contains(context.board.profile_id) {
                    return outcome(
                        CheckStatus::Fail,
                        "runtime metadata does not match selected profile",
                        "01-identity.txt",
                    );
                }
            }
            CommandResult::TimedOut => {
                return outcome(
                    CheckStatus::Timeout,
                    "build metadata command timed out",
                    "01-identity.txt",
                )
            }
            CommandResult::Completed(output) => {
                artifact.push_str(&format!("\nbinary_metadata={}", output_text(&output)));
                return outcome(
                    CheckStatus::Fail,
                    "runtime metadata command failed",
                    "01-identity.txt",
                );
            }
            CommandResult::SpawnFailed(error) => {
                return outcome(CheckStatus::Fail, &error, "01-identity.txt")
            }
        }
    }
    if !Path::new(context.board.i2c_path).exists() || !Path::new(context.board.spi_path).exists() {
        return outcome(
            CheckStatus::Fail,
            "fixed I2C or SPI device path is missing",
            "01-identity.txt",
        );
    }
    artifact.push_str(&format!(
        "\ni2c_path={}\nspi_path={}\n",
        context.board.i2c_path, context.board.spi_path
    ));
    outcome_with_content(
        CheckStatus::Pass,
        "identity and fixed profile contract match",
        "01-identity.txt",
        &artifact,
    )
}

fn storage_check(context: &CheckContext) -> CheckOutcome {
    let store = Path::new(context.board.store_dir);
    let samples = Path::new(context.board.samples_dir);
    let backup = store.join("backups");
    if !store.is_dir() || !samples.is_dir() || !store.join("default.json").is_file() {
        return outcome(
            CheckStatus::Fail,
            "store, samples, or default preset path is not ready",
            "04-storage.txt",
        );
    }
    if backup.exists() && !backup.is_dir() {
        return outcome(
            CheckStatus::Fail,
            "backup path exists but is not a directory",
            "04-storage.txt",
        );
    }
    match read_small(&store.join("default.json")) {
        Ok(payload) if serde_json::from_str::<Value>(&payload).is_ok() => outcome(
            CheckStatus::Pass,
            "store and backup paths are safe to inspect",
            "04-storage.txt",
        ),
        Ok(_) => outcome(
            CheckStatus::Fail,
            "default preset is not valid JSON",
            "04-storage.txt",
        ),
        Err(error) => outcome(
            CheckStatus::Fail,
            &format!("cannot read default preset: {error}"),
            "04-storage.txt",
        ),
    }
}

fn setup_status_check(context: &CheckContext) -> CheckOutcome {
    setup_status_check_paths(
        Path::new(context.board.setup_status_dir),
        Path::new(context.board.setup_control_dir),
    )
}

fn setup_status_check_paths(public: &Path, control: &Path) -> CheckOutcome {
    let current = public.join("current.json");
    let receipts = public.join("receipts");
    let active = control.join("active.json");
    let mut observed_status = false;
    for directory in [public, control] {
        if directory.exists() && !directory.is_dir() {
            return outcome(
                CheckStatus::Fail,
                "setup status path is not a directory",
                "05-setup-status.txt",
            );
        }
    }
    let current_fields = ["schema", "bootId", "attemptId", "sequence", "status"];
    let active_fields = [
        "schema",
        "bootId",
        "attemptId",
        "requestToken",
        "sequence",
        "reentry",
        "priorSetupComplete",
        "startedMonotonic",
        "deadlineMonotonic",
        "servicePid",
        "serviceStartTicks",
        "claimPath",
    ];
    for (path, required) in [
        (&current, &current_fields[..]),
        (&active, &active_fields[..]),
    ] {
        if path.exists() {
            observed_status = true;
            if let Err(error) = validate_json_object(path, required) {
                return outcome(CheckStatus::Fail, &error, "05-setup-status.txt");
            }
        }
    }
    if receipts.exists() {
        if !receipts.is_dir() {
            return outcome(
                CheckStatus::Fail,
                "setup receipts path is not a directory",
                "05-setup-status.txt",
            );
        }
        let entries = match fs::read_dir(receipts) {
            Ok(entries) => entries,
            Err(error) => {
                return outcome(
                    CheckStatus::Fail,
                    &format!("cannot inspect receipts: {error}"),
                    "05-setup-status.txt",
                )
            }
        };
        let receipt_fields = ["schema", "bootId", "attemptId", "sequence", "status"];
        let mut count = 0usize;
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    return outcome(
                        CheckStatus::Fail,
                        &format!("cannot read receipt entry: {error}"),
                        "05-setup-status.txt",
                    )
                }
            };
            if entry.path().extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            observed_status = true;
            count += 1;
            if count > 64 {
                return outcome(
                    CheckStatus::Fail,
                    "setup receipt directory has too many entries",
                    "05-setup-status.txt",
                );
            }
            if let Err(error) = validate_json_object(&entry.path(), &receipt_fields) {
                return outcome(CheckStatus::Fail, &error, "05-setup-status.txt");
            }
        }
    }
    if observed_status {
        outcome(
            CheckStatus::Pass,
            "observed setup status and receipts are contract-shaped",
            "05-setup-status.txt",
        )
    } else {
        outcome(
            CheckStatus::NotRun,
            "setup status evidence is absent; hygiene remains pending",
            "05-setup-status.txt",
        )
    }
}

fn oled_handoff_check(context: &CheckContext) -> CheckOutcome {
    let root = Path::new(context.board.oled_handoff_dir);
    let status_path = root.join("status.json");
    let lock_path = root.join("oled.lock");
    let payload = match read_small(&status_path) {
        Ok(payload) => payload,
        Err(error) => return outcome(CheckStatus::Fail, &error, "06-oled-handoff.txt"),
    };
    let json = match serde_json::from_str::<Value>(&payload) {
        Ok(value) => value,
        Err(error) => {
            return outcome(
                CheckStatus::Fail,
                &format!("invalid OLED handoff JSON: {error}"),
                "06-oled-handoff.txt",
            )
        }
    };
    let phase = json.get("phase").and_then(Value::as_str).unwrap_or("");
    let good_phase = matches!(phase, "native_owned" | "first_menu_rendered");
    if !lock_path.exists() || !good_phase || json.get("schema") != Some(&Value::from(1)) {
        return outcome(
            CheckStatus::Fail,
            "OLED/native handoff marker is not in native ownership",
            "06-oled-handoff.txt",
        );
    }
    outcome(
        CheckStatus::Pass,
        &format!("OLED/native handoff phase={phase}"),
        "06-oled-handoff.txt",
    )
}

fn audio_route_check(context: &CheckContext) -> CheckOutcome {
    let result = run_command("aplay", &["-l"], context.timeout);
    let text = match &result {
        CommandResult::Completed(output) => output_text(output),
        CommandResult::TimedOut => {
            return outcome(
                CheckStatus::Timeout,
                "ALSA device listing timed out",
                "07-audio.txt",
            )
        }
        CommandResult::SpawnFailed(error) => {
            return outcome(CheckStatus::Fail, error, "07-audio.txt")
        }
    };
    let lower = text.to_ascii_lowercase();
    if !context
        .board
        .audio_card_fragments
        .iter()
        .any(|fragment| lower.contains(fragment))
    {
        return outcome_with_content(
            CheckStatus::Fail,
            "selected audio card is not listed",
            "07-audio.txt",
            &text,
        );
    }
    outcome_with_content(
        CheckStatus::Pass,
        &format!(
            "audio device is listed; selected route={}",
            context.board.audio_route
        ),
        "07-audio.txt",
        &text,
    )
}

fn input_check(_context: &CheckContext) -> CheckOutcome {
    outcome(
        CheckStatus::OperatorRequired,
        "physical NeoTrellis, NeoKey, and encoder checks remain operator-led",
        "08-input.txt",
    )
}

fn usb_state_check(context: &CheckContext) -> CheckOutcome {
    let service = run_systemctl(
        &["is-active", context.board.usb_service_unit],
        context.timeout,
    );
    let udc_root = Path::new("/sys/class/udc");
    let configfs = Path::new("/sys/kernel/config/usb_gadget");
    if !udc_root.is_dir() || !configfs.is_dir() {
        return outcome(
            CheckStatus::Fail,
            "USB UDC or configfs state is unavailable",
            "09-usb.txt",
        );
    }
    let mut udcs = Vec::new();
    if let Ok(entries) = fs::read_dir(udc_root) {
        for entry in entries.flatten() {
            udcs.push(entry.file_name().to_string_lossy().into_owned());
        }
    }
    if let Some(required) = context.board.required_udc {
        if !udcs.iter().any(|value| value == required) {
            return outcome(
                CheckStatus::Fail,
                "fixed Orange UDC is missing",
                "09-usb.txt",
            );
        }
    } else if udcs.is_empty() {
        return outcome(
            CheckStatus::Fail,
            "no USB device controller is present",
            "09-usb.txt",
        );
    }
    if matches!(service, CommandResult::TimedOut) {
        return outcome(
            CheckStatus::Timeout,
            "USB service state command timed out",
            "09-usb.txt",
        );
    }
    if !support::command_succeeded(&service) {
        return outcome(
            CheckStatus::Fail,
            "USB service is inactive or its state could not be collected",
            "09-usb.txt",
        );
    }
    let artifact = format!(
        "service={}\nudcs={}\nconfigfs={}\n",
        command_record(&service),
        udcs.join(","),
        configfs.display()
    );
    outcome_with_content(
        CheckStatus::Pass,
        &format!("UDC state collected; service={}", command_record(&service)),
        "09-usb.txt",
        &artifact,
    )
}

#[cfg(test)]
mod tests {
    use super::{input_check, setup_status_check_paths, CheckContext};
    use crate::board_profile::FAT_RASPBERRY_PI_ZERO_2W;
    use crate::fat_diagnostic::model::CheckStatus;
    use std::time::Duration;

    #[test]
    fn input_check_is_operator_required_without_hardware_access() {
        let outcome = input_check(&CheckContext {
            board: FAT_RASPBERRY_PI_ZERO_2W,
            timeout: Duration::from_secs(1),
            executable: None,
        });
        assert_eq!(outcome.status, CheckStatus::OperatorRequired);
    }

    #[test]
    fn absent_setup_status_is_not_run_instead_of_a_hygiene_pass() {
        let root = std::env::temp_dir().join(format!(
            "octessera-fat-setup-status-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let outcome = setup_status_check_paths(&root.join("public"), &root.join("control"));
        assert_eq!(outcome.status, CheckStatus::NotRun);
        assert!(!root.exists());
        let _ = std::fs::remove_dir_all(root);
    }
}
