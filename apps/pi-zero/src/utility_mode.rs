#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UtilityMode {
    Normal,
    InteractiveHardware,
    InteractiveNoise,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct EnvironmentSelectors<'a> {
    pub(crate) hardware_test: Option<&'a str>,
    pub(crate) hardware_noise_test: Option<&'a str>,
}

pub(crate) fn from_process() -> Result<UtilityMode, String> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let hardware_test = std::env::var("OCTESSERA_PI_HARDWARE_TEST").ok();
    let hardware_noise_test = std::env::var("OCTESSERA_PI_HARDWARE_NOISE_TEST").ok();
    parse(
        &args,
        EnvironmentSelectors {
            hardware_test: hardware_test.as_deref(),
            hardware_noise_test: hardware_noise_test.as_deref(),
        },
    )
}

pub(crate) fn parse(
    args: &[String],
    environment: EnvironmentSelectors<'_>,
) -> Result<UtilityMode, String> {
    let interactive_hardware =
        args.iter().any(|arg| arg == "--hardware-test") || truthy(environment.hardware_test);
    let interactive_noise = args.iter().any(|arg| arg == "--hardware-noise-test")
        || truthy(environment.hardware_noise_test);
    match (interactive_hardware, interactive_noise) {
        (true, true) => {
            Err("--hardware-test and --hardware-noise-test cannot be combined; choose one".into())
        }
        (true, false) => Ok(UtilityMode::InteractiveHardware),
        (false, true) => Ok(UtilityMode::InteractiveNoise),
        (false, false) => Ok(UtilityMode::Normal),
    }
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

    fn env() -> EnvironmentSelectors<'static> {
        EnvironmentSelectors::default()
    }

    #[test]
    fn normal_mode_is_selected_without_utility_flags() {
        assert_eq!(parse(&[], env()), Ok(UtilityMode::Normal));
        assert_eq!(
            parse(&args(&["--print-build-metadata"]), env()),
            Ok(UtilityMode::Normal)
        );
    }

    #[test]
    fn interactive_modes_are_explicit_and_mutually_exclusive() {
        assert_eq!(
            parse(&args(&["--hardware-test"]), env()),
            Ok(UtilityMode::InteractiveHardware)
        );
        assert_eq!(
            parse(&args(&["--hardware-noise-test"]), env()),
            Ok(UtilityMode::InteractiveNoise)
        );
        assert!(parse(&args(&["--hardware-test", "--hardware-noise-test"]), env()).is_err());
    }
}
