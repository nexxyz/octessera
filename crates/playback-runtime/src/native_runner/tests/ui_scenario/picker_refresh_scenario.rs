//! The shared parameter picker after I2 becomes a MIDI Drum: every picker shows the new type's
//! targets and none of the old Synth's (an LFO, which takes only numeric targets, none), a pick
//! binds the new key, and a patch load while a picker is open refreshes it.
use super::device_driver::DeviceDriver;
use super::visible_menu_driver::VisibleMenuDriver;
use crate::protocol::RuntimeStoreResult;

const FADER_TARGET: &[&str] = &["Play", "Faders", "Fader 1", "Target"];
const CC_TARGET: &[&str] = &["Link", "MIDI CC In", "CC1", "Target"];
const LFO_TARGET: &[&str] = &["Link", "LFOs", "L1", "Target"];
const SYNTH_ROWS: [&str; 3] = ["Note Mode", "Synth", "Mixer"];

fn open_from_root(device: &mut DeviceDriver, path: &[&str]) {
    for _ in 0..10 {
        device.press_button("back");
    }
    let mut menu = VisibleMenuDriver::new(device);
    for label in path {
        menu.open_group(label);
    }
}

fn picker_instruments(device: &mut DeviceDriver, picker: &[&str]) -> Vec<String> {
    open_from_root(device, &[picker, &["Shape", "Instruments"]].concat());
    device.menu_labels()
}

/// Opens I2 in `picker` and checks it lists only the MIDI Drum's `Type`.
fn open_midi_drum_targets(device: &mut DeviceDriver, picker: &[&str]) {
    assert!(picker_instruments(device, picker).contains(&"I2: MIDI Drum".to_string()));
    open_from_root(device, &[picker, &["Shape", "Instruments", "I2"]].concat());
    let rows = device.menu_labels();
    assert_eq!(rows, vec!["Type"], "{picker:?} still offers {rows:?}");
}

pub(super) fn run() {
    let mut device = DeviceDriver::new();
    let synth_patch = device.patch_payload();
    open_from_root(&mut device, &["Shape", "Instruments", "I2"]);
    VisibleMenuDriver::new(&mut device).edit_enum_to("Type", "MIDI Drum");

    open_midi_drum_targets(&mut device, CC_TARGET);
    open_midi_drum_targets(&mut device, FADER_TARGET);
    VisibleMenuDriver::new(&mut device).activate_action("Type");
    assert_eq!(
        device.config_payload()["runtimeConfig"]["faders"]["sliders"][0]["target"]["key"],
        "instruments.1.type"
    );
    let lfo = picker_instruments(&mut device, LFO_TARGET);
    assert!(
        lfo.contains(&"I1: Synth".to_string()) && !lfo.iter().any(|row| row.starts_with("I2")),
        "a MIDI Drum has no numeric live target for an LFO: {lfo:?}"
    );

    open_from_root(
        &mut device,
        &[FADER_TARGET, &["Shape", "Instruments", "I2"]].concat(),
    );
    device.send_store_result(RuntimeStoreResult::LoadPresetResult {
        name: "Synth again".into(),
        payload: Some(synth_patch),
    });
    assert_eq!(
        device.snapshot()["display"]["title"],
        "../S/Instruments/I2: Synth"
    );
    let rows = device.menu_labels();
    assert!(
        SYNTH_ROWS.iter().all(|row| rows.contains(&row.to_string())),
        "the open picker refreshes to the loaded Synth: {rows:?}"
    );
}
