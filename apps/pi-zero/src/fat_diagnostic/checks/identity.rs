use super::super::model::{CheckOutcome, CheckStatus};
#[cfg(feature = "hardware-orange-pi-zero-2w")]
use super::installed_identity;
use super::support::{
    has_assignment, outcome, outcome_with_content, output_text, read_small, run_metadata_command,
    CommandResult,
};
use super::CheckContext;
use std::path::Path;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
use std::path::PathBuf;

pub(super) fn identity_check(context: &CheckContext) -> CheckOutcome {
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
    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    let (orange_executable, orange_production) = if context.board.profile_id == "orange-pi-zero-2w"
    {
        let Some(path) = context.executable.as_deref() else {
            return outcome(
                CheckStatus::Fail,
                "Orange runtime executable path is unavailable",
                "01-identity.txt",
            );
        };
        match installed_identity::canonicalize_and_is_production(path) {
            Ok((canonical, production)) => (Some(canonical), production),
            Err(error) => return outcome(CheckStatus::Fail, &error, "01-identity.txt"),
        }
    } else {
        (None, false)
    };
    #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
    let (orange_executable, orange_production): (Option<PathBuf>, bool) = (None, false);
    if orange_production {
        #[cfg(feature = "hardware-orange-pi-zero-2w")]
        {
            let path = orange_executable
                .as_deref()
                .expect("production identity has a canonical executable");
            match installed_identity::validate(path) {
                Ok(identity) => {
                    artifact.push_str(&format!(
                        "\napplication_identity=production-runtime\nresolved_executable={}\nproduction_metadata={}\nimage_fat_claim=not_evaluated",
                        path.display(),
                        identity.trim_end()
                    ));
                }
                Err(error) => {
                    let artifact_content = format!(
                        "{artifact}\napplication_identity=production-runtime\nresolved_executable={}\nimage_fat_claim=not_evaluated\nproduction_metadata_error={error}",
                        path.display()
                    );
                    return outcome_with_content(
                        CheckStatus::Fail,
                        &format!("Orange production identity validation failed: {error}"),
                        "01-identity.txt",
                        &artifact_content,
                    );
                }
            }
        }
    } else if let Some(path) = orange_executable
        .as_deref()
        .or(context.executable.as_deref())
    {
        #[cfg(feature = "hardware-orange-pi-zero-2w")]
        if context.board.profile_id == "orange-pi-zero-2w" {
            artifact.push_str(&format!(
                "\napplication_identity=runtime-candidate\nresolved_executable={}\nimage_fat_claim=not_evaluated",
                path.display()
            ));
        }
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
