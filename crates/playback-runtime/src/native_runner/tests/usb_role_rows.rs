use super::audio_restart_persistence::{back, press, turn};
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

#[test]
fn reentering_audio_after_a_usb_role_flip_starts_at_its_first_row() {
    let mut runner = board_runner(true);
    runner.audio_outputs = AudioOutputSet::from_flags(true, true, false).unwrap();
    runner.menu.rebuild(runner.menu_config());
    assert!(runner.menu.focus_item_key("audioOutputs.hdmi"));
    back(&mut runner);

    let _ = commit_setting(&mut runner, "usb.dataRole", 1);
    assert_eq!(runner.usb_data_role, UsbDataRole::Host);
    while runner.menu.current_group_path() != "Menu > System" {
        back(&mut runner);
    }
    let selected = |runner: &NativeRunner| {
        runner
            .menu
            .snapshot()
            .lines
            .into_iter()
            .find(|line| line.starts_with('>'))
            .unwrap_or_default()
    };
    for _ in 0..16 {
        if selected(&runner)
            .trim_start_matches(['>', ' '])
            .starts_with("Audio")
        {
            break;
        }
        turn(&mut runner, -1);
    }
    press(&mut runner);

    assert_eq!(runner.menu.current_group_path(), "Menu > System > Audio");
    assert_eq!(runner.menu.state.cursor, 0);
}
