use super::*;

#[test]
pub(crate) fn orange_shows_both_midi_host_and_usb_device_groups() {
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
            "Master Vol",
            "Perf. Mode",
            "Polyphony",
            "Engine"
        ]
    );
    assert_eq!(
        super::section_1::group_labels(&config, "MIDI"),
        vec!["MIDI Active", "MIDI Host", "USB Device", "Sync / Clock"]
    );
    assert_eq!(
        super::section_1::group_labels(&config, "MIDI > MIDI Host"),
        vec!["MIDI Out", "MIDI In"]
    );
    assert_eq!(
        super::section_1::group_labels(&config, "MIDI > USB Device"),
        vec!["USB MIDI"]
    );
    assert_eq!(
        super::section_1::group_labels(&config, "SD Card 2"),
        vec!["Start Transfer", "Stop Transfer"]
    );
}
