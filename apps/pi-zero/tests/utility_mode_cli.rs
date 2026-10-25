use std::process::Command;

fn clean_command() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_octessera-pi"));
    command
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

#[cfg(not(feature = "profiling"))]
#[test]
fn production_builds_refuse_profiling_modes() {
    for args in [["--profile-dsp"], ["--timing-probe"]] {
        let output = clean_command()
            .args(args)
            .output()
            .expect("profiling request should start");

        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("build"),
            "{args:?}"
        );
    }
}

#[cfg(all(feature = "hardware-orange-pi-zero-2w", feature = "profiling"))]
#[test]
fn orange_normal_profile_utility_does_not_select_diagnostics() {
    let output = clean_command()
        .arg("--profile-dsp")
        .output()
        .expect("profile utility should start");

    assert_eq!(output.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&output.stdout).contains("board_profile,orange-pi-zero-2w"));
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
