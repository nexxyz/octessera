use octessera_hal::board_profiles::{ORANGE_PI_ZERO_2W_ID, RASPBERRY_PI_ZERO_2W_ID};

#[cfg(all(board_orange, board_raspberry))]
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

#[cfg(board_orange)]
pub const BOARD_COMPOSITION: BoardComposition = BoardComposition::OrangePiZero2w;

#[cfg(board_not_orange)]
pub const BOARD_COMPOSITION: BoardComposition = BoardComposition::RaspberryPiZero2w;

pub const BOARD_PROFILE_ID: &str = BOARD_COMPOSITION.profile_id();
#[cfg(board_not_orange)]
pub const BINARY_NAME: &str = "octessera-pi";

#[cfg(board_orange)]
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

#[cfg(board_orange)]
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

fn validate_runtime_profile_value(expected: Option<&str>) -> Result<(), String> {
    #[cfg(board_orange)]
    let expected = expected.ok_or_else(|| {
        format!("OCTESSERA_EXPECTED_BOARD_PROFILE must be set to {BOARD_PROFILE_ID}")
    })?;
    #[cfg(board_not_orange)]
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

#[cfg(board_not_orange)]
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

#[cfg(board_not_orange)]
pub fn metadata_requested() -> bool {
    std::env::args()
        .skip(1)
        .any(|arg| arg == "--print-build-metadata")
}

#[cfg(test)]
mod tests;
