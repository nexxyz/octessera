use super::*;

#[test]
pub(crate) fn voice_instrument_rows_expose_configuration_groups() {
    let mut menu = NativeMenuModel::new(config());
    menu.turn(1);
    menu.turn(1);
    let _ = menu.press();
    let _ = menu.press();
    let _ = menu.press();
    let snapshot = menu.snapshot();
    assert_eq!(snapshot.path, ".../I1: synth direct");
    assert!(snapshot.lines.iter().any(|line| line == "> Type synth"));
    assert!(snapshot.lines.iter().any(|line| line == "  Synth >"));
    assert!(!snapshot.lines.iter().any(|line| line == "> Sampler >"));
    assert!(menu
        .current_siblings()
        .iter()
        .any(|item| item.label == "Slot Actions"));
    assert!(menu
        .current_siblings()
        .iter()
        .find(|item| item.label == "Slot Actions")
        .is_some_and(|item| item.children.iter().any(|child| child.label == "Clone")));
    assert!(menu
        .current_siblings()
        .iter()
        .find(|item| item.label == "Slot Actions")
        .is_some_and(|item| item.children.iter().any(|child| child.label == "Reset")));
    assert!(menu
        .current_siblings()
        .iter()
        .any(|item| item.label == "Name"));
}

#[test]
pub(crate) fn voice_menu_exposes_fx_bus_and_global_fx_groups() {
    let mut menu = NativeMenuModel::new(config());
    menu.turn(2);
    let _ = menu.press();
    let snapshot = menu.snapshot();
    assert_eq!(snapshot.path, "/Shape");
    assert!(snapshot.lines.iter().any(|line| line == "> Instruments >"));
    assert!(snapshot.lines.iter().any(|line| line == "  FX Buses >"));
    assert!(snapshot.lines.iter().any(|line| line == "  Global FX >"));
    menu.turn(1);
    let _ = menu.press();
    let buses = menu.snapshot();
    assert_eq!(buses.path, "/S/FX Buses");
    assert!(buses.lines.iter().any(|line| line == "> B1: None >"));
    let _ = menu.press();
    let _bus = menu.snapshot();
    assert!(menu
        .current_siblings()
        .iter()
        .any(|item| item.label == "Volume"));
    assert!(menu
        .current_siblings()
        .iter()
        .any(|item| item.label == "Name"));
}

#[test]
pub(crate) fn number_params_emit_bar_values_and_pan_uses_marker_format() {
    let mut menu = NativeMenuModel::new(config());
    menu.state.stack = vec![2, 0, 0, 3];
    menu.state.cursor = 2;
    let pan = menu.snapshot();
    assert!(pan.lines.iter().any(|line| line == "> Pan Pos C"));
    assert!(pan.lines.iter().any(|line| line == "  Volume 100"));
    let pan_value_row = pan.selected_row.unwrap();
    assert!(
        matches!(pan.bar_values.get(pan_value_row + 1), Some(Some(NativeMenuBarValue { style: Some(style), .. })) if style == "marker")
    );

    let mut menu = NativeMenuModel::new(config());
    menu.state.stack = vec![5, 3];
    menu.state.cursor = 0;
    let volume = menu.snapshot();
    assert!(volume.lines.iter().any(|line| line == "> Master Vol 100"));
    assert!(matches!(
        volume.bar_values.get(volume.selected_row.unwrap() + 1),
        Some(Some(NativeMenuBarValue {
            frac_pct: 100,
            style: None,
            ..
        }))
    ));

    let mut numbers_config = config();
    numbers_config.numeric_display_mode = "numbers".into();
    let mut menu = NativeMenuModel::new(numbers_config);
    menu.state.stack = vec![5, 3];
    menu.state.cursor = 0;
    let numbers = menu.snapshot();
    assert!(numbers.bar_values.iter().all(Option::is_none));
}

#[test]
pub(crate) fn mixer_menu_uses_current_volume_and_pan_values() {
    let mut config = config();
    config.instrument_volumes = vec![70];
    config.instrument_pan_positions = vec![10];
    let mut menu = NativeMenuModel::new(config);
    menu.state.stack = vec![2, 0, 0, 3];
    menu.state.cursor = 1;
    let _snapshot = menu.snapshot();
    let NativeMenuValue::Number { value, .. } = menu.current_siblings()[1].value else {
        panic!("volume should be number");
    };
    assert_eq!(value, 70);
    menu.state.cursor = 2;
    let pan = menu.snapshot();
    assert!(pan.lines.iter().any(|line| line == "> Pan Pos L6"));
}

#[test]
pub(crate) fn mixer_pan_is_non_editable_when_routed_to_fx_bus() {
    let mut config = config();
    config.instrument_routes = vec!["fx_bus_1".into()];
    let mut menu = NativeMenuModel::new(config);
    menu.state.stack = vec![2, 0, 0, 3];
    menu.state.cursor = 2;
    let snapshot = menu.snapshot();
    assert!(snapshot
        .lines
        .iter()
        .any(|line| line == "> Pan Pos -- (bus)"));
    assert_eq!(menu.current_siblings()[2].key, None);
    assert!(matches!(
        menu.current_siblings()[2].value,
        NativeMenuValue::Group
    ));
    assert_eq!(menu.press(), None);
    assert_eq!(menu.state.stack, vec![2, 0, 0, 3]);
    assert_eq!(menu.state.cursor, 2);
    assert!(!menu.state.editing);
}

#[test]
pub(crate) fn fx_slot_groups_show_selected_effect_params() {
    let mut config = config();
    config.fx_buses[0].slot1_type = "delay".into();
    config.fx_buses[0].slot1_params =
        serde_json::json!({ "timeMs": 333, "feedback": 0.42, "mixPct": 44, "spreadPct": 12 });
    config.global_fx_slots[0] = "vinyl".into();
    config.global_fx_params[0] =
        serde_json::json!({ "cracklePct": 9, "warpDepthPct": 6, "mixPct": 80 });
    let mut menu = NativeMenuModel::new(config);
    menu.state.stack = vec![2, 1, 0];
    assert!(menu
        .current_siblings()
        .iter()
        .any(|item| item.label == "Slot 1: Delay"));
    menu.state.stack = vec![2, 1, 0, 0];
    assert!(menu
        .current_siblings()
        .iter()
        .any(|item| item.label == "Time Note"
            && matches!(item.value, NativeMenuValue::Enum { ref options, selected } if options[selected] == "1/4T")));
    assert!(menu
        .current_siblings()
        .iter()
        .any(|item| item.label == "Feedback"
            && matches!(item.value, NativeMenuValue::Number { value: 42, .. })));
    assert!(menu
        .current_siblings()
        .iter()
        .any(|item| item.label == "Spread %"
            && matches!(
                item.value,
                NativeMenuValue::Number {
                    value: 12,
                    min: 0,
                    max: 100,
                    ..
                }
            )));
    menu.state.stack = vec![2, 2];
    assert!(menu
        .current_siblings()
        .iter()
        .any(|item| item.label == "Slot 1: Vinyl"));
    menu.state.stack = vec![2, 2, 0];
    assert!(menu
        .current_siblings()
        .iter()
        .any(|item| item.label == "Crackle %"
            && matches!(item.value, NativeMenuValue::Number { value: 9, .. })));
}

#[test]
pub(crate) fn binding_target_tree_excludes_delay_time_note_shortcut() {
    let mut config = config();
    config.fx_buses[0].slot1_type = "delay".into();
    config.fx_buses[0].slot1_params =
        serde_json::json!({ "timeMs": 333, "feedback": 0.42, "mixPct": 44, "spreadPct": 12 });

    let picker = parameter_picker_group("Bind".into(), "aux.turn.0".into(), None, &config);
    let keys = binding_action_keys(&picker);

    assert!(keys.iter().any(|key| key.ends_with(".params.timeMs")));
    assert!(!keys.iter().any(|key| key.ends_with(".params.timeMode")));
    assert!(!keys.iter().any(|key| key.ends_with(".params.timeNote")));
}

fn binding_action_keys(item: &NativeMenuItem) -> Vec<String> {
    let mut keys = Vec::new();
    if let NativeMenuValue::Action(NativeMenuAction::SetParamBinding { binding, .. }) = &item.value
    {
        keys.push(binding.key.clone());
    }
    for child in &item.children {
        keys.extend(binding_action_keys(child));
    }
    keys
}

#[test]
pub(crate) fn duck_source_param_displays_source_text() {
    let mut config = config();
    config.fx_buses[0].slot1_type = "duck".into();
    config.fx_buses[0].slot1_params = serde_json::json!({ "source": "B2" });
    let mut menu = NativeMenuModel::new(config);
    menu.state.stack = vec![2, 1, 0, 0];
    menu.state.cursor = 1;
    let snapshot = menu.snapshot();
    assert!(snapshot.lines.iter().any(|line| line == "> Source B2"));
    assert!(!snapshot.lines.iter().any(|line| line.contains("Source 0")));
}
