pub(super) fn print_build_metadata() {
    println!(
        "{}",
        serde_json::json!({
            "schema_version": 1,
            "board_profile": super::BOARD_PROFILE_ID,
            "binary": super::BINARY_NAME,
            "arch": std::env::consts::ARCH,
            "package_version": env!("CARGO_PKG_VERSION"),
            "artifact_kind": "diagnostic-only",
            "profile": "release",
            "cargo_feature": "hardware-raspberry-pi-zero-2w routing-tree-benchmark benchmark-voice-pools-128",
        })
    );
}
