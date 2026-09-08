#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
use super::BINARY_NAME;
use super::{BoardComposition, BOARD_COMPOSITION, BOARD_PROFILE_ID};

#[test]
fn fat_diagnostic_selects_only_the_two_fixed_board_descriptors() {
    let raspberry = super::fat_diagnostic_board("raspberry-pi-zero-2w").unwrap();
    let orange = super::fat_diagnostic_board("orange-pi-zero-2w").unwrap();
    assert_eq!(raspberry.store_dir, "/home/pi/presets");
    assert_eq!(
        raspberry.profile_contract_path,
        "/etc/octessera/board-profile.env"
    );
    assert_eq!(raspberry.i2c_path, "/dev/i2c-1");
    assert_eq!(orange.store_dir, "/var/lib/octessera/presets");
    assert_eq!(
        orange.profile_contract_path,
        "/etc/octessera/build-metadata.env"
    );
    assert_eq!(orange.required_udc, Some("musb-hdrc.4.auto"));
    assert!(super::fat_diagnostic_board("desktop").is_none());
}

#[test]
fn audio_card_identity_keeps_raspberry_fragments_but_requires_orange_octesseradac() {
    assert_eq!(
        super::FAT_ORANGE_PI_ZERO_2W.audio_card_fragments,
        &["octesseradac"]
    );
    assert_eq!(
        super::FAT_RASPBERRY_PI_ZERO_2W.audio_card_fragments,
        &["hifiberry", "pcm5102a", "snd_rpi_hifiberry"]
    );
}

#[cfg(not(any(
    feature = "hardware-raspberry-pi-zero-2w",
    feature = "hardware-orange-pi-zero-2w",
    feature = "legacy-hardware-rpi-zero-2w",
    feature = "legacy-hardware-pi"
)))]
#[test]
fn fat_diagnostics_reject_an_unprofiled_default_build() {
    let error = super::validate_fat_diagnostic_profile("raspberry-pi-zero-2w")
        .expect_err("default builds must not run board diagnostics");
    assert!(error.contains("canonical hardware build"));
}

#[cfg(any(
    feature = "legacy-hardware-rpi-zero-2w",
    feature = "legacy-hardware-pi"
))]
#[test]
fn deprecated_raspberry_features_cannot_run_fat_diagnostics() {
    let error = super::validate_fat_diagnostic_profile("raspberry-pi-zero-2w")
        .expect_err("deprecated Raspberry features must not run diagnostics");
    assert!(error.contains("canonical hardware build"));
}

#[cfg(all(
    feature = "hardware-raspberry-pi-zero-2w",
    not(any(
        feature = "legacy-hardware-rpi-zero-2w",
        feature = "legacy-hardware-pi"
    ))
))]
#[test]
fn raspberry_fat_diagnostics_reject_the_other_board_profile() {
    let error = super::validate_fat_diagnostic_profile("orange-pi-zero-2w")
        .expect_err("Raspberry diagnostics must reject Orange profile selection");
    assert!(error.contains("canonical compiled hardware profile raspberry-pi-zero-2w"));
}

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
#[test]
fn raspberry_composition_uses_canonical_profile() {
    assert_eq!(BOARD_COMPOSITION, BoardComposition::RaspberryPiZero2w);
    assert_eq!(BOARD_PROFILE_ID, "raspberry-pi-zero-2w");
    assert_eq!(BOARD_COMPOSITION.profile_id(), BOARD_PROFILE_ID);
    assert_eq!(BINARY_NAME, "octessera-pi");
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
#[test]
fn orange_composition_selects_only_its_canonical_identity() {
    assert_eq!(BOARD_COMPOSITION, BoardComposition::OrangePiZero2w);
    assert_eq!(BOARD_PROFILE_ID, "orange-pi-zero-2w");
    assert_eq!(BOARD_COMPOSITION.profile_id(), BOARD_PROFILE_ID);
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
#[test]
fn orange_fat_diagnostics_reject_the_other_board_profile() {
    let error = super::validate_fat_diagnostic_profile("raspberry-pi-zero-2w")
        .expect_err("Orange diagnostics must reject Raspberry profile selection");
    assert!(error.contains("canonical compiled hardware profile orange-pi-zero-2w"));
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
#[test]
fn wrong_runtime_profile_is_rejected_before_hardware_startup() {
    let error = super::validate_runtime_profile_value(Some("raspberry-pi-zero-2w"))
        .expect_err("wrong board profile must fail closed");
    assert_eq!(
        error,
        "board profile mismatch: binary=orange-pi-zero-2w, expected=raspberry-pi-zero-2w"
    );
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
#[test]
fn orange_runtime_requires_an_explicit_profile_contract() {
    let error = super::validate_runtime_profile_value(None)
        .expect_err("Orange startup must require its profile contract");
    assert_eq!(
        error,
        "OCTESSERA_EXPECTED_BOARD_PROFILE must be set to orange-pi-zero-2w"
    );
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
#[test]
fn orange_metadata_selection_matches_the_compiled_contract() {
    #[cfg(all(
        feature = "routing-tree-benchmark",
        feature = "benchmark-voice-pools-128"
    ))]
    assert_eq!(
        super::ORANGE_METADATA_CARGO_FEATURE,
        octessera_hal::orange_metadata::RUNTIME_BENCHMARK_DIAGNOSTIC_CARGO_FEATURE_ROUTING_128
    );
    #[cfg(all(
        feature = "routing-tree-benchmark",
        feature = "benchmark-voice-pools-256"
    ))]
    assert_eq!(
        super::ORANGE_METADATA_CARGO_FEATURE,
        octessera_hal::orange_metadata::RUNTIME_BENCHMARK_DIAGNOSTIC_CARGO_FEATURE_ROUTING_256
    );
    #[cfg(all(
        feature = "routing-tree-benchmark",
        not(any(
            feature = "benchmark-voice-pools-128",
            feature = "benchmark-voice-pools-256"
        ))
    ))]
    assert_eq!(
        super::ORANGE_METADATA_CARGO_FEATURE,
        octessera_hal::orange_metadata::RUNTIME_BENCHMARK_DIAGNOSTIC_CARGO_FEATURE_ROUTING
    );
    #[cfg(feature = "benchmark-voice-pools-128")]
    #[cfg(not(feature = "routing-tree-benchmark"))]
    assert_eq!(
        super::ORANGE_METADATA_CARGO_FEATURE,
        octessera_hal::orange_metadata::RUNTIME_BENCHMARK_DIAGNOSTIC_CARGO_FEATURE_128
    );
    #[cfg(feature = "benchmark-voice-pools-256")]
    #[cfg(not(feature = "routing-tree-benchmark"))]
    assert_eq!(
        super::ORANGE_METADATA_CARGO_FEATURE,
        octessera_hal::orange_metadata::RUNTIME_BENCHMARK_DIAGNOSTIC_CARGO_FEATURE_256
    );
    #[cfg(any(
        feature = "routing-tree-benchmark",
        feature = "benchmark-voice-pools-128",
        feature = "benchmark-voice-pools-256"
    ))]
    assert_eq!(
        super::ORANGE_METADATA_CONTRACT_NAME,
        "runtime benchmark diagnostic"
    );
    #[cfg(not(any(
        feature = "routing-tree-benchmark",
        feature = "benchmark-voice-pools-128",
        feature = "benchmark-voice-pools-256"
    )))]
    assert_eq!(super::ORANGE_METADATA_CONTRACT_NAME, "runtime-candidate");
}
