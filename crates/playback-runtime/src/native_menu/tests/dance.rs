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
    assert_eq!(labels, vec!["BPM", "Dance Page"]);
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

    let mut trigger_gate_config = config();
    trigger_gate_config.dance_mode = "trigger-gate".into();
    let trigger_gate_menu = NativeMenuModel::new(trigger_gate_config);
    let trigger_gate_labels = trigger_gate_menu.root.children[3]
        .children
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();
    assert_eq!(trigger_gate_labels, vec!["BPM", "Dance Page"]);
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
fn parameter_picker_includes_behavior_sense_fx_and_global_fx_families() {
    let mut cfg = config();
    cfg.fx_buses[0].slot1_type = "delay".into();
    cfg.fx_buses[0].slot1_params =
        serde_json::json!({ "feedback": 0.35, "timeMs": 250, "mixPct": 35 });
    cfg.global_fx_slots[0] = "vinyl".into();
    cfg.global_fx_params[0] = serde_json::json!({ "cracklePct": 8, "saturationPct": 15, "warpDepthPct": 5, "mixPct": 100 });
    cfg.dance_mode = "xy".into();
    cfg.dance_fx_type = "stutter".into();
    let menu = NativeMenuModel::new(cfg);

    assert!(contains_set_binding(
        &menu.root,
        "xy:x",
        "parts.0.l1.behaviorConfig.randomTickInterval"
    ));
    assert!(contains_set_binding(
        &menu.root,
        "xy:x",
        "parts.1.l1.behaviorConfig.randomTickInterval"
    ));
    assert!(contains_set_binding(
        &menu.root,
        "xy:x",
        "parts.0.l2.scanMode"
    ));
    assert!(contains_set_binding(
        &menu.root,
        "xy:x",
        "instruments.0.synth.filter.cutoffHz"
    ));
    assert!(contains_set_binding(
        &menu.root,
        "xy:x",
        "mixer.buses.0.slot1.params.feedback"
    ));
    assert!(contains_set_binding(
        &menu.root,
        "xy:x",
        "mixer.master.slots.0.params.cracklePct"
    ));
    assert!(contains_set_binding(
        &menu.root,
        "xy:x",
        "dance.fx.params.rateHz"
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
