use octessera_hal::board_profiles::{ORANGE_PI_ZERO_2W_ID, RASPBERRY_PI_ZERO_2W_ID};

#[cfg(all(
    feature = "hardware-orange-pi-zero-2w",
    feature = "hardware-raspberry-pi-zero-2w"
))]
compile_error!("Orange and Raspberry Pi app profiles are mutually exclusive");

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BoardComposition {
    RaspberryPiZero2w,
    OrangePiZero2w,
}

impl BoardComposition {
    pub const fn profile_id(self) -> &'static str {
        match self {
            Self::RaspberryPiZero2w => RASPBERRY_PI_ZERO_2W_ID,
            Self::OrangePiZero2w => ORANGE_PI_ZERO_2W_ID,
        }
    }
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
pub const BOARD_COMPOSITION: BoardComposition = BoardComposition::OrangePiZero2w;

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
pub const BOARD_COMPOSITION: BoardComposition = BoardComposition::RaspberryPiZero2w;

pub const BOARD_PROFILE_ID: &str = BOARD_COMPOSITION.profile_id();
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
pub const BINARY_NAME: &str = "octessera-pi";

#[cfg(all(
    feature = "hardware-raspberry-pi-zero-2w",
    not(any(
        feature = "legacy-hardware-rpi-zero-2w",
        feature = "legacy-hardware-pi"
    ))
))]
pub const COMPILED_FAT_DIAGNOSTIC_PROFILE: Option<&str> = Some(RASPBERRY_PI_ZERO_2W_ID);

#[cfg(all(
    feature = "hardware-orange-pi-zero-2w",
    not(any(
        feature = "legacy-hardware-rpi-zero-2w",
        feature = "legacy-hardware-pi"
    ))
))]
pub const COMPILED_FAT_DIAGNOSTIC_PROFILE: Option<&str> = Some(ORANGE_PI_ZERO_2W_ID);

#[cfg(not(any(
    feature = "hardware-raspberry-pi-zero-2w",
    feature = "hardware-orange-pi-zero-2w",
    feature = "legacy-hardware-rpi-zero-2w",
    feature = "legacy-hardware-pi"
)))]
pub const COMPILED_FAT_DIAGNOSTIC_PROFILE: Option<&str> = None;

#[cfg(any(
    feature = "legacy-hardware-rpi-zero-2w",
    feature = "legacy-hardware-pi"
))]
pub const COMPILED_FAT_DIAGNOSTIC_PROFILE: Option<&str> = None;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FatDiagnosticBoard {
    pub profile_id: &'static str,
    pub model_fragment: &'static str,
    pub profile_contract_path: &'static str,
    pub service_unit: &'static str,
    pub service_user: &'static str,
    pub store_dir: &'static str,
    pub samples_dir: &'static str,
    pub setup_status_dir: &'static str,
    pub oled_handoff_dir: &'static str,
    pub readiness_path: &'static str,
    pub i2c_path: &'static str,
    pub spi_path: &'static str,
    pub audio_card_fragments: &'static [&'static str],
    pub audio_route: &'static str,
    pub usb_service_unit: &'static str,
    pub required_udc: Option<&'static str>,
}

const RASPBERRY_AUDIO_CARD_FRAGMENTS: &[&str] = &["hifiberry", "pcm5102a", "snd_rpi_hifiberry"];
const ORANGE_AUDIO_CARD_FRAGMENTS: &[&str] = &["octesseradac"];

pub const FAT_RASPBERRY_PI_ZERO_2W: FatDiagnosticBoard = FatDiagnosticBoard {
    profile_id: RASPBERRY_PI_ZERO_2W_ID,
    model_fragment: "Raspberry Pi Zero 2 W",
    profile_contract_path: "/etc/octessera/board-profile.env",
    service_unit: "octessera.service",
    service_user: "pi",
    store_dir: "/home/pi/presets",
    samples_dir: "/home/pi/samples",
    setup_status_dir: "/run/octessera-setup-status",
    oled_handoff_dir: "/run/octessera-boot",
    readiness_path: "/run/octessera/candidate-ready.json",
    i2c_path: "/dev/i2c-1",
    spi_path: "/dev/spidev0.0",
    audio_card_fragments: RASPBERRY_AUDIO_CARD_FRAGMENTS,
    audio_route: "hw:0,0",
    usb_service_unit: "octessera-usb-gadget.service",
    required_udc: None,
};

pub const FAT_ORANGE_PI_ZERO_2W: FatDiagnosticBoard = FatDiagnosticBoard {
    profile_id: ORANGE_PI_ZERO_2W_ID,
    model_fragment: "OrangePi Zero 2W",
    profile_contract_path: "/etc/octessera/build-metadata.env",
    service_unit: "octessera.service",
    service_user: "octessera-runtime",
    store_dir: "/var/lib/octessera/presets",
    samples_dir: "/var/lib/octessera/samples",
    setup_status_dir: "/run/octessera-setup-status",
    oled_handoff_dir: "/run/octessera-boot",
    readiness_path: "/run/octessera/candidate-ready.json",
    i2c_path: "/dev/i2c-2",
    spi_path: "/dev/spidev1.0",
    audio_card_fragments: ORANGE_AUDIO_CARD_FRAGMENTS,
    audio_route: "hw:CARD=octesseradac,DEV=0",
    usb_service_unit: "octessera-orange-usb-gadget.service",
    required_udc: Some("musb-hdrc.4.auto"),
};

pub const fn fat_diagnostic_board(profile_id: &str) -> Option<FatDiagnosticBoard> {
    if same_profile_id(profile_id, RASPBERRY_PI_ZERO_2W_ID) {
        Some(FAT_RASPBERRY_PI_ZERO_2W)
    } else if same_profile_id(profile_id, ORANGE_PI_ZERO_2W_ID) {
        Some(FAT_ORANGE_PI_ZERO_2W)
    } else {
        None
    }
}

const fn same_profile_id(left: &str, right: &str) -> bool {
    let left = left.as_bytes();
    let right = right.as_bytes();
    if left.len() != right.len() {
        return false;
    }
    let mut index = 0;
    while index < left.len() {
        if left[index] != right[index] {
            return false;
        }
        index += 1;
    }
    true
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
const _: () = assert!(
    same_profile_id(
        BOARD_PROFILE_ID,
        octessera_hal::board_profiles::ACTIVE_BOARD_PROFILE_ID
    ) && same_profile_id(BOARD_PROFILE_ID, ORANGE_PI_ZERO_2W_ID)
);

pub fn validate_runtime_profile() -> Result<(), String> {
    validate_runtime_profile_value(
        std::env::var("OCTESSERA_EXPECTED_BOARD_PROFILE")
            .ok()
            .as_deref(),
    )
}

pub fn validate_fat_diagnostic_profile(profile_id: &str) -> Result<(), String> {
    let Some(compiled_profile) = COMPILED_FAT_DIAGNOSTIC_PROFILE else {
        return Err("FAT diagnostics require a canonical hardware build; rebuild with --no-default-features --features hardware-raspberry-pi-zero-2w or hardware-orange-pi-zero-2w".into());
    };
    if profile_id != compiled_profile {
        return Err(format!(
            "FAT diagnostic profile {profile_id} does not match canonical compiled hardware profile {compiled_profile}"
        ));
    }
    Ok(())
}

fn validate_runtime_profile_value(expected: Option<&str>) -> Result<(), String> {
    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    let expected = expected.ok_or_else(|| {
        format!("OCTESSERA_EXPECTED_BOARD_PROFILE must be set to {BOARD_PROFILE_ID}")
    })?;
    #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
    let Some(expected) = expected
    else {
        return Ok(());
    };
    if expected != BOARD_PROFILE_ID {
        return Err(format!(
            "board profile mismatch: binary={BOARD_PROFILE_ID}, expected={expected}"
        ));
    }
    Ok(())
}

pub fn print_build_metadata() {
    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    {
        if let Err(error) = octessera_hal::orange_metadata::print_runtime_candidate_metadata() {
            eprintln!("Orange runtime-candidate metadata check failed: {error}");
            std::process::exit(1);
        }
    }
    #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
    println!(
        "{}",
        serde_json::json!({
            "schema_version": 1,
            "board_profile": BOARD_PROFILE_ID,
            "binary": BINARY_NAME,
            "arch": std::env::consts::ARCH,
            "package_version": env!("CARGO_PKG_VERSION"),
        })
    );
}

pub fn metadata_requested() -> bool {
    std::env::args()
        .skip(1)
        .any(|arg| arg == "--print-build-metadata")
}

#[cfg(test)]
mod tests {
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
}
