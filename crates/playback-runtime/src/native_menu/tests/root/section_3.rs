use super::*;

#[test]
pub(crate) fn unsupported_host_keeps_gadget_controls_without_usb_role_row() {
    let mut config = config();
    config.jack_audio_required = true;
    config.audio_optimization_capacity_available = true;
    config.usb_data_role = crate::native_runner::UsbDataRole::Host;

    assert!(!super::section_1::system_labels(&config)
        .iter()
        .any(|label| label == "USB Role"));
    assert_eq!(
        super::section_1::group_labels(&config, "Audio"),
        vec![
            "USB Audio",
            "HDMI Audio",
            "Perf. Mode",
            "Polyphony",
            "Engine"
        ]
    );
    assert_eq!(
        super::section_1::group_labels(&config, "MIDI"),
        vec!["Enabled", "USB MIDI", "MIDI Out", "MIDI In", "Sync / Clock"]
    );
    assert_eq!(
        super::section_1::group_labels(&config, "SD Card 2"),
        vec!["Start Transfer", "Stop Transfer"]
    );
}
