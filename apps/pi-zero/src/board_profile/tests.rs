#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
use super::BINARY_NAME;
use super::{BoardComposition, BOARD_COMPOSITION, BOARD_PROFILE_ID};

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
