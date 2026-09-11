use super::*;

#[test]
fn performance_mode_help_covers_both_values_on_pi() {
    let mut config = config();
    config.jack_audio_required = true;
    config.audio_optimization_capacity_available = true;
    let target = NativeMenuModel::new(config)
        .help_targets()
        .into_iter()
        .find(|target| target.key == "key:sound.optimizeFor")
        .expect("DSP mode help target");
    let entry = crate::native_help::resolve_native_help_entry(&target).expect("DSP mode help");
    let copy = format!("{} {}", entry.line1, entry.line2);
    assert_eq!(entry.title, "Perf. Mode");
    assert!(copy.contains("Latency"));
    assert!(copy.contains("Capacity"));
}

#[test]
fn board_device_help_targets_match_desktop_visibility_policy() {
    let desktop = NativeMenuModel::new(config());
    assert!(!desktop.help_targets().into_iter().any(|target| {
        target.key == "key:audioOutputs.dac"
            || target.key == "key:audioOutputs.usb"
            || target.key == "key:audioOutputs.hdmi"
            || target.key == "key:usb.midiOutEnabled"
            || target.key == "key:hdmi.mode"
    }));

    let mut pi_config = config();
    pi_config.jack_audio_required = true;
    let pi_targets = NativeMenuModel::new(pi_config);
    let usb_target = pi_targets
        .help_targets()
        .into_iter()
        .find(|target| target.key == "key:audioOutputs.usb")
        .expect("Pi USB Audio help target");
    assert_eq!(
        crate::native_help::resolve_native_help_entry(&usb_target)
            .expect("Pi USB Audio help entry")
            .title,
        "USB Audio"
    );
}
