use super::*;

fn selected_option(root: &NativeMenuItem, key: &str) -> String {
    let item = find_item_by_key(root, key).expect("timing menu item");
    let NativeMenuValue::Enum { options, selected } = &item.value else {
        panic!("timing menu item should be an enum");
    };
    options[*selected].clone()
}

fn item_with_label<'a>(item: &'a NativeMenuItem, label: &str) -> Option<&'a NativeMenuItem> {
    if item.label == label {
        return Some(item);
    }
    item.children
        .iter()
        .find_map(|child| item_with_label(child, label))
}

#[test]
pub(crate) fn invalid_timing_menu_labels_use_the_canonical_default() {
    let mut config = config();
    config.link_lfos[0].period = "invalid".into();
    config.pulses_layers[0].scan_mode = "scanning".into();
    config.pulses_layers[0].scan_unit = "invalid".into();
    config.fx_buses[0].slot1_type = "delay".into();
    config.fx_buses[0].slot1_params = serde_json::json!({
        "timeMs": 250,
        "timeNote": "invalid",
        "feedback": 0.35,
        "mixPct": 35
    });

    let root = build_root(config);

    assert_eq!(
        selected_option(&root, "linkLfos.0.period"),
        crate::timing_units::DEFAULT_NOTE_UNIT
    );
    assert_eq!(
        selected_option(&root, "layers.0.pulses.scanUnit"),
        crate::timing_units::DEFAULT_NOTE_UNIT
    );
    assert_eq!(
        selected_option(&root, "mixer.buses.0.slot1.params.timeNote"),
        crate::timing_units::DEFAULT_NOTE_UNIT
    );
}

#[test]
pub(crate) fn scan_unit_binding_keeps_the_canonical_note_option_order() {
    let root = build_root(config());
    let scan_unit = item_with_label(&root, "Scan Unit").expect("scan unit binding");
    let NativeMenuValue::Action(NativeMenuAction::SetParamBinding { binding, .. }) =
        &scan_unit.value
    else {
        panic!("scan unit binding should be an action");
    };
    assert_eq!(
        binding.options,
        crate::timing_units::NOTE_UNIT_OPTIONS
            .iter()
            .map(|option| (*option).to_string())
            .collect::<Vec<_>>()
    );
}
