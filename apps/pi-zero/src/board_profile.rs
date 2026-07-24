use octessera_hal::board_profiles::RASPBERRY_PI_ZERO_2W_ID;

pub const BOARD_PROFILE_ID: &str = RASPBERRY_PI_ZERO_2W_ID;
pub const BINARY_NAME: &str = "octessera-pi";

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
    use super::{BINARY_NAME, BOARD_PROFILE_ID};
    use octessera_hal::board_profiles::{ORANGE_PI_ZERO_2W_ID, RASPBERRY_PI_ZERO_2W_ID};

    #[test]
    fn binary_metadata_uses_canonical_raspberry_profile() {
        assert_eq!(BOARD_PROFILE_ID, RASPBERRY_PI_ZERO_2W_ID);
        assert_ne!(BOARD_PROFILE_ID, ORANGE_PI_ZERO_2W_ID);
        assert_eq!(BINARY_NAME, "octessera-pi");
    }
}
