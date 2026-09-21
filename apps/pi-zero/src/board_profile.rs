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
    feature = "routing-tree-benchmark",
    feature = "benchmark-voice-pools-128",
))]
#[path = "raspberry_live_benchmark_metadata.rs"]
mod raspberry_live_benchmark_metadata;

#[cfg(all(
    feature = "hardware-orange-pi-zero-2w",
    feature = "routing-tree-benchmark",
    feature = "benchmark-voice-pools-128"
))]
const ORANGE_METADATA_CARGO_FEATURE: &str =
    octessera_hal::orange_metadata::RUNTIME_BENCHMARK_DIAGNOSTIC_CARGO_FEATURE_ROUTING_128;

#[cfg(all(
    feature = "hardware-orange-pi-zero-2w",
    feature = "routing-tree-benchmark",
    feature = "benchmark-voice-pools-256"
))]
const ORANGE_METADATA_CARGO_FEATURE: &str =
    octessera_hal::orange_metadata::RUNTIME_BENCHMARK_DIAGNOSTIC_CARGO_FEATURE_ROUTING_256;

#[cfg(all(
    feature = "hardware-orange-pi-zero-2w",
    feature = "routing-tree-benchmark",
    not(any(
        feature = "benchmark-voice-pools-128",
        feature = "benchmark-voice-pools-256"
    ))
))]
const ORANGE_METADATA_CARGO_FEATURE: &str =
    octessera_hal::orange_metadata::RUNTIME_BENCHMARK_DIAGNOSTIC_CARGO_FEATURE_ROUTING;

#[cfg(all(
    feature = "hardware-orange-pi-zero-2w",
    not(feature = "routing-tree-benchmark"),
    feature = "benchmark-voice-pools-128"
))]
const ORANGE_METADATA_CARGO_FEATURE: &str =
    octessera_hal::orange_metadata::RUNTIME_BENCHMARK_DIAGNOSTIC_CARGO_FEATURE_128;

#[cfg(all(
    feature = "hardware-orange-pi-zero-2w",
    not(feature = "routing-tree-benchmark"),
    feature = "benchmark-voice-pools-256"
))]
const ORANGE_METADATA_CARGO_FEATURE: &str =
    octessera_hal::orange_metadata::RUNTIME_BENCHMARK_DIAGNOSTIC_CARGO_FEATURE_256;

#[cfg(all(
    feature = "hardware-orange-pi-zero-2w",
    any(
        feature = "routing-tree-benchmark",
        feature = "benchmark-voice-pools-128",
        feature = "benchmark-voice-pools-256"
    )
))]
const ORANGE_METADATA_CONTRACT_NAME: &str = "runtime benchmark diagnostic";

#[cfg(all(
    feature = "hardware-orange-pi-zero-2w",
    not(any(
        feature = "routing-tree-benchmark",
        feature = "benchmark-voice-pools-128",
        feature = "benchmark-voice-pools-256"
    ))
))]
const ORANGE_METADATA_CONTRACT_NAME: &str = "runtime-candidate";

#[cfg(feature = "hardware-raspberry-pi-zero-2w")]
pub const COMPILED_FAT_DIAGNOSTIC_PROFILE: Option<&str> = Some(RASPBERRY_PI_ZERO_2W_ID);

#[cfg(feature = "hardware-orange-pi-zero-2w")]
pub const COMPILED_FAT_DIAGNOSTIC_PROFILE: Option<&str> = Some(ORANGE_PI_ZERO_2W_ID);

#[cfg(not(any(
    feature = "hardware-raspberry-pi-zero-2w",
    feature = "hardware-orange-pi-zero-2w"
)))]
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
    print_orange_build_metadata();
    #[cfg(all(
        feature = "hardware-raspberry-pi-zero-2w",
        feature = "routing-tree-benchmark",
        feature = "benchmark-voice-pools-128",
    ))]
    raspberry_live_benchmark_metadata::print_build_metadata();
    #[cfg(not(any(
        feature = "hardware-orange-pi-zero-2w",
        all(
            feature = "hardware-raspberry-pi-zero-2w",
            feature = "routing-tree-benchmark",
            feature = "benchmark-voice-pools-128",
        )
    )))]
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

#[cfg(all(
    feature = "hardware-orange-pi-zero-2w",
    any(
        feature = "routing-tree-benchmark",
        feature = "benchmark-voice-pools-128",
        feature = "benchmark-voice-pools-256"
    )
))]
fn print_orange_build_metadata() {
    let result = octessera_hal::orange_metadata::print_runtime_benchmark_diagnostic_metadata(
        ORANGE_METADATA_CARGO_FEATURE,
    );
    if let Err(error) = result {
        eprintln!("Orange {ORANGE_METADATA_CONTRACT_NAME} metadata check failed: {error}");
        std::process::exit(1);
    }
}

#[cfg(all(
    feature = "hardware-orange-pi-zero-2w",
    not(any(
        feature = "routing-tree-benchmark",
        feature = "benchmark-voice-pools-128",
        feature = "benchmark-voice-pools-256"
    ))
))]
fn print_orange_build_metadata() {
    if let Err(error) = octessera_hal::orange_metadata::print_runtime_candidate_metadata() {
        eprintln!("Orange {ORANGE_METADATA_CONTRACT_NAME} metadata check failed: {error}");
        std::process::exit(1);
    }
}

pub fn metadata_requested() -> bool {
    std::env::args()
        .skip(1)
        .any(|arg| arg == "--print-build-metadata")
}

#[cfg(test)]
#[path = "board_profile_tests.rs"]
mod tests;
