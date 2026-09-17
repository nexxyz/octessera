use super::section_1::group_labels;
use super::*;

#[test]
fn system_actions_preserve_direct_effects_and_named_presets() {
    let menu = NativeMenuModel::new(config());
    assert!(matches!(
        menu.item_for_key("preset.load").map(|item| item.value),
        Some(NativeMenuValue::Group)
    ));
    assert!(matches!(
        menu.item_for_key("preset.load.none").map(|item| item.value),
        Some(NativeMenuValue::Action(NativeMenuAction::PlatformEffect(effect)))
            if effect == "preset.refresh"
    ));
    for key in [
        "midi.panic",
        "system.info",
        "system.controlsHelp",
        "system.reboot",
        "system.shutdown",
    ] {
        assert!(
            matches!(
                menu.item_for_key(key).map(|item| item.value),
                Some(NativeMenuValue::Action(NativeMenuAction::PlatformEffect(_)))
            ),
            "{key} should be a direct action"
        );
    }

    let mut named = config();
    named.preset_names = vec!["Alpha".into(), "Beta".into()];
    assert_eq!(group_labels(&named, "Load Preset"), vec!["Alpha", "Beta"]);
    let named_menu = NativeMenuModel::new(named);
    assert!(matches!(
        named_menu
            .item_for_key("preset.load.Alpha")
            .map(|item| item.value),
        Some(NativeMenuValue::Action(NativeMenuAction::PlatformEffect(effect)))
            if effect == "preset.load:Alpha"
    ));
}

#[test]
fn midi_host_output_help_describes_orange_route_retention() {
    let entries = crate::native_help::native_help_entries_for_tests();
    let find = |key: &str, path: &str| {
        entries
            .iter()
            .find(|entry| entry.key == key && entry.path == path)
            .unwrap_or_else(|| panic!("missing MIDI help entry {key} {path}"))
    };
    let disconnect = find(
        "action:midi_select_output:null",
        "System > MIDI > MIDI Host > MIDI Out > Disconnect",
    );
    assert!(disconnect.line1.contains("clears the retained Host"));
    assert!(disconnect.line2.contains("USB Device MIDI stays active"));

    let output = find(
        "action:midi_select_output:*",
        "System > MIDI > MIDI Host > MIDI Out > *",
    );
    assert!(output.line2.contains("retained for later"));

    let group = find("", "Menu > System > MIDI > MIDI Host > MIDI Out");
    assert!(group.line2.contains("Disconnect clears it"));
    assert!(group.line2.contains("without disabling USB Device MIDI"));
}
