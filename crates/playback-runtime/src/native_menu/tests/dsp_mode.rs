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
            || target.key == "key:usb.dataRole"
            || target.key == "key:hdmi.mode"
    }));

    let mut pi_config = config();
    pi_config.jack_audio_required = true;
    pi_config.usb_data_role_available = true;
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
    let role_target = pi_targets
        .help_targets()
        .into_iter()
        .find(|target| target.key == "key:usb.dataRole")
        .expect("Raspberry USB Role help target");
    let role_entry = crate::native_help::resolve_native_help_entry(&role_target)
        .expect("Raspberry USB Role help entry");
    assert_eq!(role_entry.path, "System > Setup > USB Role");
    assert_eq!(role_entry.title, "USB Role");
    assert_eq!(
        role_entry.line1,
        "Selects Raspberry USB role. Gadget allows USB Audio, USB MIDI, and SD2 transfer; Host disables all three."
    );
    assert_eq!(
        role_entry.line2,
        "Unplug computer USB before rebooting to Host."
    );
}

#[test]
fn midi_and_sd2_help_describes_board_hierarchy_and_roles() {
    let mut config = config();
    config.jack_audio_required = true;
    let menu = NativeMenuModel::new(config);

    let enabled = menu
        .help_targets()
        .into_iter()
        .find(|target| target.key == "key:midiEnabled")
        .expect("global MIDI enabled help target");
    assert_eq!(enabled.path, "Menu > System > MIDI > MIDI Active");
    let enabled_entry = crate::native_help::resolve_native_help_entry(&enabled)
        .expect("global MIDI enabled help entry");
    let enabled_copy = format!("{} {}", enabled_entry.line1, enabled_entry.line2).to_lowercase();
    assert!(enabled_copy.contains("global runtime gate"));
    assert!(enabled_copy.contains("does not select a port or device"));

    let usb_midi = menu
        .help_targets()
        .into_iter()
        .find(|target| target.key == "key:usb.midiOutEnabled")
        .expect("USB MIDI help target");
    assert_eq!(
        usb_midi.path,
        "Menu > System > MIDI > USB Device > USB MIDI"
    );
    let usb_midi_entry =
        crate::native_help::resolve_native_help_entry(&usb_midi).expect("USB MIDI help entry");
    let usb_midi_copy = format!("{} {}", usb_midi_entry.line1, usb_midi_entry.line2).to_lowercase();
    for phrase in [
        "bidirectional usb midi",
        "after restart",
        "owns both input and output",
        "routing takes precedence",
        "host selections are retained",
    ] {
        assert!(
            usb_midi_copy.contains(phrase),
            "USB MIDI help omitted {phrase}"
        );
    }

    let midi_host = menu
        .help_targets()
        .into_iter()
        .find(|target| target.path == "Menu > System > MIDI > MIDI Host")
        .expect("MIDI Host help target");
    let midi_host_entry =
        crate::native_help::resolve_native_help_entry(&midi_host).expect("MIDI Host help entry");
    let midi_host_copy = format!("{} {}", midi_host_entry.line1, midi_host_entry.line2);
    assert!(midi_host_copy.contains("MIDI Out"));
    assert!(midi_host_copy.contains("MIDI In"));

    let usb_device = menu
        .help_targets()
        .into_iter()
        .find(|target| target.path == "Menu > System > MIDI > USB Device")
        .expect("USB Device help target");
    let usb_device_entry =
        crate::native_help::resolve_native_help_entry(&usb_device).expect("USB Device help entry");
    let usb_device_copy = format!("{} {}", usb_device_entry.line1, usb_device_entry.line2);
    assert!(usb_device_copy.contains("computer-facing gadget interface"));

    let sd2 = menu
        .help_targets()
        .into_iter()
        .find(|target| target.path == "Menu > System > SD Card 2")
        .expect("SD Card 2 help target");
    let sd2_entry =
        crate::native_help::resolve_native_help_entry(&sd2).expect("SD Card 2 help entry");
    let sd2_copy = format!("{} {}", sd2_entry.line1, sd2_entry.line2).to_lowercase();
    for phrase in [
        "second card",
        "usb host",
        "usb audio",
        "midi",
        "recording",
        "inactive",
    ] {
        assert!(sd2_copy.contains(phrase), "SD Card 2 help omitted {phrase}");
    }
}
