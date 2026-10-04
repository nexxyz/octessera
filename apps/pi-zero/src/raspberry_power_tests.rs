use super::*;

#[test]
fn power_command_attempts_match_shutdown_sudoers_shape() {
    let shutdown = power_command_attempts(PowerRequest::Shutdown);
    assert!(shutdown.contains(&("/usr/bin/systemctl", &["poweroff"][..])));
    assert!(shutdown.contains(&("sudo", &["-n", "/usr/bin/systemctl", "poweroff"][..])));
    assert!(!shutdown
        .iter()
        .any(|(_, args)| args.contains(&"--no-block")));
    let reboot = power_command_attempts(PowerRequest::Reboot);
    assert!(reboot.contains(&("/usr/bin/systemctl", &["reboot"][..])));
    assert!(reboot.contains(&("sudo", &["-n", "/usr/bin/systemctl", "reboot"][..])));
    assert!(!reboot.iter().any(|(_, args)| args.contains(&"--no-block")));
}
