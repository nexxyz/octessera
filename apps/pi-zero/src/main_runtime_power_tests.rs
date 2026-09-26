use super::*;

#[test]
fn power_command_attempts_match_shutdown_sudoers_shape() {
    let shutdown = power_command_attempts(PiPowerRequest::Shutdown);
    assert!(shutdown
        .iter()
        .any(|attempt| *attempt == ("/usr/bin/systemctl", &["poweroff"])));
    assert!(shutdown
        .iter()
        .any(|attempt| *attempt == ("sudo", &["-n", "/usr/bin/systemctl", "poweroff"])));
    assert!(!shutdown
        .iter()
        .any(|(_, args)| args.contains(&"--no-block")));
    let reboot = power_command_attempts(PiPowerRequest::Reboot);
    assert!(reboot
        .iter()
        .any(|attempt| *attempt == ("/usr/bin/systemctl", &["reboot"])));
    assert!(reboot
        .iter()
        .any(|attempt| *attempt == ("sudo", &["-n", "/usr/bin/systemctl", "reboot"])));
    assert!(!reboot.iter().any(|(_, args)| args.contains(&"--no-block")));
}
