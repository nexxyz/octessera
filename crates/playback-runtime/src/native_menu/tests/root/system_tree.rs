use super::section_1::{group_labels, system_labels};
use super::*;

#[test]
fn desktop_system_tree_has_exact_order() {
    let desktop = config();
    assert_eq!(
        system_labels(&desktop),
        vec![
            "Load Preset",
            "Recording",
            "Notes",
            "MIDI",
            "Audio",
            "UI",
            "Saves",
            "Setup",
            "Reset",
            "Panic",
            "Sys. Info",
            "Basic Help",
            "Reboot",
            "Shutdown",
        ]
    );
    assert_eq!(
        group_labels(&desktop, "Recording"),
        vec!["Max Time", "Start Audio", "St. Audio+OLED", "Stop"]
    );
    assert_eq!(group_labels(&desktop, "Load Preset"), vec!["(none)"]);
    assert_eq!(
        group_labels(&desktop, "Notes"),
        vec!["Note Length", "Vel Scale", "Vel Curve"]
    );
    assert_eq!(
        group_labels(&desktop, "MIDI"),
        vec!["MIDI Active", "MIDI Host", "Sync / Clock"]
    );
    assert_eq!(
        group_labels(&desktop, "MIDI > MIDI Host"),
        vec!["MIDI Out", "MIDI In"]
    );
    assert_eq!(
        group_labels(&desktop, "Audio"),
        vec!["Master Vol", "Polyphony", "Engine"]
    );
    assert_eq!(
        group_labels(&desktop, "Audio > Engine"),
        vec!["CPU Warn %", "Bus Idle", "Buf Frames"]
    );
    assert_eq!(
        group_labels(&desktop, "Setup"),
        vec![
            "Updates",
            "Configure WiFi",
            "Backup / Restore",
            "Hardware Test"
        ]
    );
    assert_eq!(
        group_labels(&desktop, "Saves > Library"),
        vec![
            "Save As",
            "Load",
            "Rename",
            "Delete",
            "Save Current",
            "Refresh List"
        ]
    );
    assert_eq!(
        group_labels(&desktop, "Saves > Default"),
        vec!["Auto Save", "Backups", "Save Default", "Load Default"]
    );
    assert!(NativeMenuModel::new(desktop)
        .item_for_key("usb.midiOutEnabled")
        .is_none());
}

#[test]
fn orange_system_tree_has_exact_order() {
    let mut orange = config();
    orange.jack_audio_required = true;
    orange.audio_optimization_capacity_available = true;
    assert_eq!(
        system_labels(&orange),
        vec![
            "Load Preset",
            "Recording",
            "Notes",
            "MIDI",
            "Audio",
            "UI",
            "SD Card 2",
            "HDMI Video",
            "Saves",
            "Setup",
            "Reset",
            "Panic",
            "Sys. Info",
            "Basic Help",
            "Reboot",
            "Shutdown",
        ]
    );
    assert_eq!(
        group_labels(&orange, "Audio"),
        vec![
            "USB Audio",
            "HDMI Audio",
            "Master Vol",
            "Perf. Mode",
            "Polyphony",
            "Engine",
        ]
    );
    assert_eq!(
        group_labels(&orange, "MIDI"),
        vec!["MIDI Active", "MIDI Host", "USB Device", "Sync / Clock"]
    );
    assert_eq!(
        group_labels(&orange, "MIDI > MIDI Host"),
        vec!["MIDI Out", "MIDI In"]
    );
    assert_eq!(group_labels(&orange, "MIDI > USB Device"), vec!["USB MIDI"]);
    assert_eq!(
        group_labels(&orange, "SD Card 2"),
        vec!["Start Transfer", "Stop Transfer"]
    );
    assert_eq!(
        group_labels(&orange, "HDMI Video"),
        vec!["Mode", "Grid Lines"]
    );
    assert_eq!(
        group_labels(&orange, "Setup"),
        vec![
            "Updates",
            "Configure WiFi",
            "Backup / Restore",
            "Hardware Test"
        ]
    );
}

#[test]
fn raspberry_gadget_system_tree_has_exact_order() {
    let mut raspberry_gadget = config();
    raspberry_gadget.jack_audio_required = true;
    raspberry_gadget.audio_optimization_capacity_available = true;
    raspberry_gadget.usb_data_role_available = true;
    raspberry_gadget.usb_data_role = crate::native_runner::UsbDataRole::Gadget;
    assert_eq!(
        system_labels(&raspberry_gadget),
        vec![
            "Load Preset",
            "Recording",
            "Notes",
            "MIDI",
            "Audio",
            "UI",
            "SD Card 2",
            "HDMI Video",
            "Saves",
            "Setup",
            "Reset",
            "Panic",
            "Sys. Info",
            "Basic Help",
            "Reboot",
            "Shutdown",
        ]
    );
    assert_eq!(
        group_labels(&raspberry_gadget, "MIDI"),
        vec!["MIDI Active", "USB Device", "Sync / Clock"]
    );
    assert_eq!(
        group_labels(&raspberry_gadget, "Audio"),
        vec![
            "USB Audio",
            "HDMI Audio",
            "Master Vol",
            "Perf. Mode",
            "Polyphony",
            "Engine",
        ]
    );
    assert_eq!(
        group_labels(&raspberry_gadget, "Setup"),
        vec![
            "USB Role",
            "Updates",
            "Configure WiFi",
            "Backup / Restore",
            "Hardware Test",
        ]
    );
    assert!(NativeMenuModel::new(raspberry_gadget.clone())
        .item_for_key("midi.output.none")
        .is_none());
    assert_eq!(
        group_labels(&raspberry_gadget, "SD Card 2"),
        vec!["Start Transfer", "Stop Transfer"]
    );
}

#[test]
fn raspberry_host_system_tree_has_exact_order() {
    let mut raspberry_host = config();
    raspberry_host.jack_audio_required = true;
    raspberry_host.audio_optimization_capacity_available = true;
    raspberry_host.usb_data_role_available = true;
    raspberry_host.usb_data_role = crate::native_runner::UsbDataRole::Host;
    assert_eq!(
        system_labels(&raspberry_host),
        vec![
            "Load Preset",
            "Recording",
            "Notes",
            "MIDI",
            "Audio",
            "UI",
            "SD Card 2",
            "HDMI Video",
            "Saves",
            "Setup",
            "Reset",
            "Panic",
            "Sys. Info",
            "Basic Help",
            "Reboot",
            "Shutdown",
        ]
    );
    assert_eq!(
        group_labels(&raspberry_host, "MIDI"),
        vec!["MIDI Active", "MIDI Host", "Sync / Clock"]
    );
    assert_eq!(
        group_labels(&raspberry_host, "MIDI > MIDI Host"),
        vec!["MIDI Out", "MIDI In"]
    );
    assert_eq!(
        group_labels(&raspberry_host, "Audio"),
        vec![
            "HDMI Audio",
            "Master Vol",
            "Perf. Mode",
            "Polyphony",
            "Engine"
        ]
    );
    assert_eq!(
        group_labels(&raspberry_host, "SD Card 2"),
        vec!["Stop Transfer"]
    );
    assert!(NativeMenuModel::new(raspberry_host.clone())
        .item_for_key("usb.midiOutEnabled")
        .is_none());
    let role = NativeMenuModel::new(raspberry_host)
        .item_for_key("usb.dataRole")
        .expect("USB Role row");
    assert_eq!(
        role.value,
        NativeMenuValue::Enum {
            options: vec!["Gadget".into(), "Host".into()],
            selected: 1,
        }
    );
}
