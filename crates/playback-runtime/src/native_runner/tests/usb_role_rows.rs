use super::device_config_apply::{board_runner, commit_setting};
use super::*;

#[test]
fn usb_role_changes_its_rows_in_place_to_match_a_full_rebuild() {
    let mut runner = board_runner(true);
    runner.audio_outputs = AudioOutputSet::from_flags(true, true, false).unwrap();
    runner.usb_midi_out_enabled = true;
    runner.midi_outputs = vec![MidiPort {
        id: "name:Keys".into(),
        name: "Keys".into(),
    }];
    runner.menu.rebuild(runner.menu_config());
    let rebuilds = runner.menu.rebuild_count;

    for delta in [1, -1] {
        let _ = commit_setting(&mut runner, "usb.dataRole", delta);
        assert_eq!(runner.menu.rebuild_count, rebuilds);
        assert_eq!(
            runner.menu.root,
            crate::native_menu::NativeMenuModel::new(runner.menu_config()).root
        );
    }
    assert_eq!(runner.usb_data_role, UsbDataRole::Gadget);
    assert!(runner.menu.focus_item_key("audioOutputs.usb"));
}
