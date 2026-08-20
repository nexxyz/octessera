#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UtilityMode {
    Normal,
    LegacyDiagnostic,
    FatDiagnostic,
    InteractiveHardware,
    InteractiveNoise,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct EnvironmentSelectors<'a> {
    pub(crate) diagnostic: Option<&'a str>,
    pub(crate) hardware_test: Option<&'a str>,
    pub(crate) hardware_noise_test: Option<&'a str>,
}

pub(crate) fn from_process() -> Result<UtilityMode, String> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let diagnostic = std::env::var("OCTESSERA_PI_DIAGNOSTIC").ok();
    let hardware_test = std::env::var("OCTESSERA_PI_HARDWARE_TEST").ok();
    let hardware_noise_test = std::env::var("OCTESSERA_PI_HARDWARE_NOISE_TEST").ok();
    parse(
        &args,
        EnvironmentSelectors {
            diagnostic: diagnostic.as_deref(),
            hardware_test: hardware_test.as_deref(),
            hardware_noise_test: hardware_noise_test.as_deref(),
        },
    )
}

pub(crate) fn parse(
    args: &[String],
    environment: EnvironmentSelectors<'_>,
) -> Result<UtilityMode, String> {
    let diagnostic_arg = args.iter().any(|arg| arg == "--diagnostic");
    let fat_diagnostic_arg = args.iter().any(|arg| arg == "--fat-diagnostic");
    if diagnostic_arg && fat_diagnostic_arg {
        return Err("--diagnostic and --fat-diagnostic cannot be combined; choose one".into());
    }
    let profile = profile_option(args)?;
    let interactive_hardware =
        args.iter().any(|arg| arg == "--hardware-test") || truthy(environment.hardware_test);
    let interactive_noise = args.iter().any(|arg| arg == "--hardware-noise-test")
        || truthy(environment.hardware_noise_test);
    if interactive_hardware && interactive_noise {
        return Err(
            "--hardware-test and --hardware-noise-test cannot be combined; choose one".into(),
        );
    }
    let diagnostic_environment = diagnostic_environment_selected(environment.diagnostic);
    if diagnostic_environment {
        if profile.is_some() {
            return Err(
                "OCTESSERA_PI_DIAGNOSTIC cannot be combined with --board-profile or --profile"
                    .into(),
            );
        }
        if interactive_hardware || interactive_noise {
            return Err(
                "OCTESSERA_PI_DIAGNOSTIC cannot be combined with interactive hardware-test modes"
                    .into(),
            );
        }
        if fat_diagnostic_arg {
            return Err("OCTESSERA_PI_DIAGNOSTIC cannot be combined with --fat-diagnostic".into());
        }
        return Ok(UtilityMode::LegacyDiagnostic);
    }
    if interactive_hardware || interactive_noise {
        if diagnostic_arg || fat_diagnostic_arg || profile.is_some() {
            return Err("interactive hardware-test modes cannot be combined with diagnostic or profile options".into());
        }
        return Ok(if interactive_hardware {
            UtilityMode::InteractiveHardware
        } else {
            UtilityMode::InteractiveNoise
        });
    }
    if profile.is_some() && !diagnostic_arg && !fat_diagnostic_arg {
        return Err("--board-profile/--profile requires --diagnostic or --fat-diagnostic".into());
    }
    if (diagnostic_arg || fat_diagnostic_arg)
        && args
            .iter()
            .any(|arg| matches!(arg.as_str(), "--help" | "-h"))
    {
        return Ok(UtilityMode::FatDiagnostic);
    }
    if fat_diagnostic_arg && profile.is_none() {
        return Err(
            "--board-profile is required with --fat-diagnostic; choose a fixed board profile"
                .into(),
        );
    }
    if fat_diagnostic_arg {
        return Ok(UtilityMode::FatDiagnostic);
    }
    if diagnostic_arg {
        if profile.is_none() && diagnostic_options_present(args) {
            return Err(
                "diagnostic options require --fat-diagnostic with an explicit board profile".into(),
            );
        }
        return Ok(if profile.is_some() {
            UtilityMode::FatDiagnostic
        } else {
            UtilityMode::LegacyDiagnostic
        });
    }
    if diagnostic_options_present(args) {
        return Err("diagnostic options require --diagnostic or --fat-diagnostic".into());
    }
    Ok(UtilityMode::Normal)
}

fn profile_option(args: &[String]) -> Result<Option<&str>, String> {
    let mut profile = None;
    let mut index = 0;
    while index < args.len() {
        if matches!(args[index].as_str(), "--board-profile" | "--profile") {
            if profile.is_some() {
                return Err("only one --board-profile/--profile option is allowed".into());
            }
            let value = args
                .get(index + 1)
                .ok_or_else(|| format!("{} requires a value", args[index]))?;
            if value.is_empty() || value.starts_with("--") {
                return Err(format!("{} requires a profile value", args[index]));
            }
            profile = Some(value.as_str());
            index += 2;
        } else {
            index += 1;
        }
    }
    Ok(profile)
}

fn diagnostic_options_present(args: &[String]) -> bool {
    args.iter()
        .any(|arg| matches!(arg.as_str(), "--evidence-dir" | "--timeout-seconds"))
}

fn diagnostic_environment_selected(value: Option<&str>) -> bool {
    value.is_some_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "preflight" | "hardware"
        )
    })
}

fn truthy(value: Option<&str>) -> bool {
    value.is_some_and(|value| matches!(value.trim().to_ascii_lowercase().as_str(), "1" | "true"))
}

#[cfg(test)]
mod tests {
    use super::{parse, EnvironmentSelectors, UtilityMode};

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).into()).collect()
    }

    fn env(diagnostic: Option<&str>) -> EnvironmentSelectors<'_> {
        EnvironmentSelectors {
            diagnostic,
            ..EnvironmentSelectors::default()
        }
    }

    #[test]
    fn normal_mode_is_selected_without_utility_flags() {
        assert_eq!(parse(&[], env(None)), Ok(UtilityMode::Normal));
        assert_eq!(
            parse(&args(&["--print-build-metadata"]), env(None)),
            Ok(UtilityMode::Normal)
        );
    }

    #[test]
    fn diagnostic_selectors_distinguish_legacy_and_profile_modes() {
        assert_eq!(
            parse(&args(&["--diagnostic"]), env(None)),
            Ok(UtilityMode::LegacyDiagnostic)
        );
        assert_eq!(
            parse(
                &args(&["--diagnostic", "--board-profile", "orange-pi-zero-2w"]),
                env(None),
            ),
            Ok(UtilityMode::FatDiagnostic)
        );
        assert_eq!(
            parse(
                &args(&["--fat-diagnostic", "--profile", "raspberry-pi-zero-2w"]),
                env(None),
            ),
            Ok(UtilityMode::FatDiagnostic)
        );
    }

    #[test]
    fn diagnostic_environment_is_a_selector_and_rejects_ambiguous_modes() {
        assert_eq!(
            parse(&[], env(Some("1"))),
            Ok(UtilityMode::LegacyDiagnostic)
        );
        assert!(parse(
            &args(&["--board-profile", "orange-pi-zero-2w"]),
            env(Some("1"))
        )
        .unwrap_err()
        .contains("cannot be combined"));
        assert!(parse(&args(&["--hardware-test"]), env(Some("1")))
            .unwrap_err()
            .contains("interactive"));
    }

    #[test]
    fn interactive_modes_are_explicit_and_mutually_exclusive() {
        assert_eq!(
            parse(&args(&["--hardware-test"]), env(None)),
            Ok(UtilityMode::InteractiveHardware)
        );
        assert_eq!(
            parse(&args(&["--hardware-noise-test"]), env(None)),
            Ok(UtilityMode::InteractiveNoise)
        );
        assert!(parse(
            &args(&["--hardware-test", "--hardware-noise-test"]),
            env(None)
        )
        .is_err());
    }

    #[test]
    fn invalid_or_missing_profile_modes_fail_closed() {
        assert!(parse(&args(&["--fat-diagnostic"]), env(None))
            .unwrap_err()
            .contains("required"));
        assert!(
            parse(&args(&["--board-profile", "orange-pi-zero-2w"]), env(None))
                .unwrap_err()
                .contains("requires")
        );
        assert!(parse(&args(&["--diagnostic", "--board-profile"]), env(None)).is_err());
        assert!(parse(&args(&["--diagnostic", "--hardware-test"]), env(None)).is_err());
        assert!(parse(
            &args(&["--diagnostic", "--evidence-dir", "evidence"]),
            env(None)
        )
        .is_err());
    }
}
