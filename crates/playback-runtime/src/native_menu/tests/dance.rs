use super::*;

#[test]
fn l4_spec_rows_show_only_selected_dance_page_controls() {
    let menu = NativeMenuModel::new(config());
    let dance = &menu.root.children[3];
    let labels = dance
        .children
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();
    assert_eq!(labels, vec!["Dance Page", "BPM"]);
    let bpm = dance
        .children
        .iter()
        .find(|item| item.label == "BPM")
        .unwrap();
    assert!(matches!(
        bpm.value,
        NativeMenuValue::Number {
            min: 40,
            max: 240,
            step: 1,
            ..
        }
    ));

    let mut fx_config = config();
    fx_config.dance_mode = "fx".into();
    let fx_menu = NativeMenuModel::new(fx_config);
    let fx_labels = fx_menu.root.children[3]
        .children
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();
    assert!(fx_labels.contains(&"FX Type"));
    assert!(fx_labels.contains(&"Target"));
    assert!(!fx_labels.contains(&"Trigger Gate"));
}

#[test]
fn dance_fx_page_is_flat_and_shows_selected_type_params() {
    let mut config = config();
    config.dance_mode = "fx".into();
    config.dance_fx_type = "stutter".into();
    config
        .dance_fx_params
        .insert("rateHz".into(), serde_json::json!(12));
    let mut menu = NativeMenuModel::new(config);
    menu.state.stack = vec![3];
    let _snapshot = menu.snapshot();
    assert!(menu
        .current_siblings()
        .iter()
        .any(|item| item.label == "FX Type"));
    assert!(menu
        .current_siblings()
        .iter()
        .any(|item| item.label == "Rate Hz"));
    assert!(menu
        .current_siblings()
        .iter()
        .any(|item| item.label == "Rate Hz"
            && matches!(item.value, NativeMenuValue::Number { value: 12, .. })));
    assert!(menu
        .current_siblings()
        .iter()
        .any(|item| item.label == "Depth"));
    assert!(!menu
        .current_siblings()
        .iter()
        .any(|item| item.label == "Stutter"));
}

#[test]
fn parameter_picker_exposes_binding_actions_for_aux_param_and_xy_targets() {
    let menu = NativeMenuModel::new(config());
    assert!(contains_set_binding(
        &menu.root,
        "aux:0:turn",
        "sound.noteLengthMs"
    ));
    assert!(contains_set_binding(
        &menu.root,
        "param:0:x:0",
        "instruments.0.mixer.volume"
    ));

    let mut xy_config = config();
    xy_config.dance_mode = "xy".into();
    let xy_menu = NativeMenuModel::new(xy_config);
    assert!(contains_set_binding(
        &xy_menu.root,
        "xy:x",
        "sound.velocityScalePct"
    ));
}

#[test]
fn aux_click_picker_exposes_assignable_actions() {
    let menu = NativeMenuModel::new(config());
    assert!(contains_aux_click_action(
        &menu.root,
        0,
        "sample.assign:0:0"
    ));
    assert!(contains_aux_click_action(&menu.root, 0, "dance.fx.map"));
    assert!(contains_aux_click_reset(&menu.root, 0));
}
