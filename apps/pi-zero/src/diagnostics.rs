pub(crate) fn run_pre_hardware_diagnostics() -> Result<bool, String> {
    println!(
        "WARN legacy diagnostic mode is deprecated; use --fat-diagnostic --board-profile <profile>"
    );
    crate::fat_diagnostic::run_legacy_raspberry()
}
