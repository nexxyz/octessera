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
    assert_eq!(snapshot.path, "L3: Voice/Instruments/I1: synth direct");
    assert!(snapshot.lines.iter().any(|line| line == "> Type synth"));
    assert!(snapshot.lines.iter().any(|line| line == "  Synth"));
    assert!(!snapshot.lines.iter().any(|line| line == "> Sampler"));
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
    assert!(snapshot.lines.iter().any(|line| line == "  Name"));
}

#[test]
pub(crate) fn voice_menu_exposes_fx_bus_and_global_fx_groups() {
    let mut menu = NativeMenuModel::new(config());
    menu.turn(2);
    let _ = menu.press();
    let snapshot = menu.snapshot();
    assert_eq!(snapshot.path, "L3: Voice");
    assert!(snapshot.lines.iter().any(|line| line == "> Instruments"));
    assert!(snapshot.lines.iter().any(|line| line == "  FX Buses"));
    assert!(snapshot.lines.iter().any(|line| line == "  Global FX"));
    menu.turn(1);
    let _ = menu.press();
    let buses = menu.snapshot();
    assert_eq!(buses.path, "L3: Voice/FX Buses");
    assert!(buses.lines.iter().any(|line| line == "> B1: None"));
    let _ = menu.press();
    let bus = menu.snapshot();
    assert!(bus.lines.iter().any(|line| line == "  Name"));
}

#[test]
pub(crate) fn number_params_emit_bar_values_and_pan_uses_marker_format() {
    let mut menu = NativeMenuModel::new(config());
    menu.state.stack = vec![2, 0, 0, 3];
    menu.state.cursor = 2;
    let pan = menu.snapshot();
    assert!(pan.lines.iter().any(|line| line == "> Pan Pos C"));
    let pan_value_row = pan.selected_row.unwrap();
    assert!(
        matches!(pan.bar_values.get(pan_value_row), Some(Some(NativeMenuBarValue { style: Some(style), .. })) if style == "marker")
    );

    let mut menu = NativeMenuModel::new(config());
    menu.state.stack = vec![5, 3];
    menu.state.cursor = 0;
    let volume = menu.snapshot();
    assert!(volume.lines.iter().any(|line| line == "> Master Vol 100"));
    assert!(matches!(
        volume.bar_values.get(volume.selected_row.unwrap()),
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
pub(crate) fn fx_slot_groups_show_selected_effect_params() {
    let mut config = config();
    config.fx_buses[0].slot1_type = "delay".into();
    config.fx_buses[0].slot1_params =
        serde_json::json!({ "timeMs": 333, "feedback": 0.42, "mixPct": 44 });
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
        .any(|item| item.label == "Feedback"
            && matches!(item.value, NativeMenuValue::Number { value: 42, .. })));
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
