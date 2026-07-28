use octessera_hal::board_profiles::{ORANGE_PI_ZERO_2W_ID, RASPBERRY_PI_ZERO_2W_ID};

#[cfg(any(
    all(
        feature = "hardware-orange-pi-zero-2w",
        feature = "hardware-raspberry-pi-zero-2w"
    ),
    all(
        feature = "hardware-orange-pi-zero-2w",
        feature = "hardware-rpi-zero-2w"
    ),
    all(feature = "hardware-orange-pi-zero-2w", feature = "hardware-pi")
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

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
pub fn validate_runtime_profile() -> Result<(), String> {
    if let Ok(expected) = std::env::var("OCTESSERA_EXPECTED_BOARD_PROFILE") {
        if expected != BOARD_PROFILE_ID {
            return Err(format!(
                "board profile mismatch: binary={BOARD_PROFILE_ID}, expected={expected}"
            ));
        }
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
}
