use std::process::Command;

fn clean_command() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_octessera-pi"));
    command
        .env_remove("OCTESSERA_PI_DIAGNOSTIC")
        .env_remove("OCTESSERA_PI_HARDWARE_TEST")
        .env_remove("OCTESSERA_PI_HARDWARE_NOISE_TEST")
        .env_remove("OCTESSERA_PI_PROFILE_DSP")
        .env_remove("OCTESSERA_PI_TIMING_PROBE")
        .env_remove("OCTESSERA_PI_TIMING_PROBE_DURATIONS")
        .env_remove("OCTESSERA_PI_TIMING_PROBE_SCENARIOS")
        .env_remove("OCTESSERA_PI_TIMING_PROBE_CONFIG")
        .env_remove("OCTESSERA_PI_TIMING_PROBE_RUNTIME_ONLY")
        .env_remove("OCTESSERA_PI_TIMING_PROBE_AUDIO_DRAIN")
        .env_remove("OCTESSERA_PI_TIMING_PROBE_AUDIO_DRAIN_INTERVAL_MS");
    command
}

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
#[test]
fn normal_metadata_mode_does_not_enter_hardware_runtime() {
    let output = clean_command()
        .arg("--print-build-metadata")
        .output()
        .expect("metadata command should start");

    assert_eq!(output.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&output.stdout).contains("board_profile"));
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
#[test]
fn orange_normal_profile_utility_does_not_select_diagnostics() {
    let output = clean_command()
        .arg("--profile-dsp")
        .output()
        .expect("profile utility should start");

    assert_eq!(output.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&output.stdout).contains("board_profile,orange-pi-zero-2w"));
}

#[test]
fn diagnostic_environment_with_a_profile_is_rejected_before_hardware_access() {
    let output = clean_command()
        .env("OCTESSERA_PI_DIAGNOSTIC", "1")
        .args(["--profile", "orange-pi-zero-2w"])
        .output()
        .expect("diagnostic command should start");

    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr)
        .contains("OCTESSERA_PI_DIAGNOSTIC cannot be combined with --board-profile or --profile"));
}

#[test]
fn diagnostic_environment_with_interactive_mode_is_rejected_before_hardware_access() {
    let output = clean_command()
        .env("OCTESSERA_PI_DIAGNOSTIC", "1")
        .arg("--hardware-test")
        .output()
        .expect("diagnostic command should start");

    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr)
        .contains("OCTESSERA_PI_DIAGNOSTIC cannot be combined with interactive"));
}

#[test]
fn fat_diagnostic_without_a_profile_is_rejected_before_hardware_access() {
    let output = clean_command()
        .arg("--fat-diagnostic")
        .output()
        .expect("diagnostic command should start");

    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("--board-profile is required"));
}

#[cfg(not(any(
    feature = "hardware-raspberry-pi-zero-2w",
    feature = "hardware-orange-pi-zero-2w"
)))]
#[test]
fn default_build_rejects_the_diagnostic_environment_alias_before_hardware_access() {
    let output = clean_command()
        .env("OCTESSERA_PI_DIAGNOSTIC", "1")
        .output()
        .expect("diagnostic command should start");

    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("canonical hardware build"));
}

#[cfg(not(any(
    feature = "hardware-raspberry-pi-zero-2w",
    feature = "hardware-orange-pi-zero-2w"
)))]
#[test]
fn default_build_rejects_fat_diagnostics_before_hardware_access() {
    let output = clean_command()
        .args([
            "--fat-diagnostic",
            "--board-profile",
            "raspberry-pi-zero-2w",
        ])
        .output()
        .expect("diagnostic command should start");

    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("canonical hardware build"));
}

#[cfg(not(any(
    feature = "hardware-raspberry-pi-zero-2w",
    feature = "hardware-orange-pi-zero-2w"
)))]
#[test]
fn default_build_rejects_the_deprecated_raspberry_alias_before_hardware_access() {
    let output = clean_command()
        .arg("--diagnostic")
        .output()
        .expect("diagnostic command should start");

    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stdout).contains("deprecated"));
    assert!(String::from_utf8_lossy(&output.stderr).contains("canonical hardware build"));
}

#[cfg(not(any(
    feature = "hardware-raspberry-pi-zero-2w",
    feature = "hardware-orange-pi-zero-2w"
)))]
#[test]
fn diagnostic_and_interactive_modes_are_rejected_together() {
    let output = clean_command()
        .args(["--diagnostic", "--hardware-test"])
        .output()
        .expect("diagnostic command should start");

    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("cannot be combined"));
}

#[cfg(not(any(
    feature = "hardware-raspberry-pi-zero-2w",
    feature = "hardware-orange-pi-zero-2w"
)))]
#[test]
fn interactive_modes_are_rejected_together() {
    let output = clean_command()
        .args(["--hardware-test", "--hardware-noise-test"])
        .output()
        .expect("hardware-test command should start");

    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("cannot be combined; choose one"));
}

#[cfg(all(
    feature = "hardware-raspberry-pi-zero-2w",
    not(any(
        feature = "legacy-hardware-rpi-zero-2w",
        feature = "legacy-hardware-pi"
    ))
))]
#[test]
fn raspberry_build_rejects_orange_fat_diagnostics() {
    let output = clean_command()
        .args(["--fat-diagnostic", "--board-profile", "orange-pi-zero-2w"])
        .output()
        .expect("diagnostic command should start");

    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr)
        .contains("canonical compiled hardware profile raspberry-pi-zero-2w"));
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
#[test]
fn orange_build_rejects_the_diagnostic_environment_alias_before_hardware_access() {
    let output = clean_command()
        .env("OCTESSERA_PI_DIAGNOSTIC", "1")
        .output()
        .expect("diagnostic command should start");

    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr)
        .contains("canonical compiled hardware profile orange-pi-zero-2w"));
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
#[test]
fn orange_build_rejects_raspberry_fat_diagnostics() {
    let output = clean_command()
        .args([
            "--fat-diagnostic",
            "--board-profile",
            "raspberry-pi-zero-2w",
        ])
        .output()
        .expect("diagnostic command should start");

    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr)
        .contains("canonical compiled hardware profile orange-pi-zero-2w"));
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
#[test]
fn orange_build_rejects_interactive_hardware_test_mode() {
    let output = Command::new(env!("CARGO_BIN_EXE_octessera-pi"))
        .arg("--hardware-test")
        .output()
        .expect("hardware-test command should start");

    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr)
        .contains("only available on the canonical Raspberry build"));
}
