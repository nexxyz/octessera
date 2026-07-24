use std::process::Command;

#[test]
fn print_build_metadata_exits_before_runtime_startup() {
    let output = Command::new(env!("CARGO_BIN_EXE_octessera-pi"))
        .arg("--print-build-metadata")
        .output()
        .expect("metadata command should start");

    assert!(
        output.status.success(),
        "metadata command failed: {output:?}"
    );
    let metadata: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("metadata output should be JSON");

    assert_eq!(metadata["binary"], "octessera-pi");
    assert_eq!(metadata["board_profile"], "raspberry-pi-zero-2w");
    assert_eq!(metadata["package_version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(metadata["schema_version"], 1);
}
