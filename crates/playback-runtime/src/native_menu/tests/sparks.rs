use super::*;

#[test]
pub(crate) fn sparks_spec_rows_show_only_selected_sparks_page_controls() {
    let menu = NativeMenuModel::new(config());
    let sparks = &menu.root.children[3];
    let labels = sparks
        .children
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        labels,
        vec!["Mix", "Pan", "FX", "Trigger Gate", "Transpose", "XY"]
    );
    let sense = &menu.root.children[1];
    assert_eq!(sense.children[0].label, "BPM");
    assert_eq!(sense.children[1].label, "Swing");
    let bpm = sense
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
    let swing = sense
        .children
        .iter()
        .find(|item| item.label == "Swing")
        .unwrap();
    assert!(matches!(
        swing.value,
        NativeMenuValue::Number {
            min: 0,
            max: 75,
            step: 1,
            ..
        }
    ));

    let mut fx_config = config();
    fx_config.sparks_mode = "fx".into();
    let fx_menu = NativeMenuModel::new(fx_config);
    let fx_labels = fx_menu.root.children[3]
        .children
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        fx_labels,
        vec!["Mix", "Pan", "FX", "Trigger Gate", "Transpose", "XY"]
    );
    let fx_labels = fx_menu.root.children[3].children[2]
        .children
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();
    assert!(fx_labels.contains(&"FX Type"));
    assert!(fx_labels.contains(&"Target"));

    let mut trigger_gate_config = config();
    trigger_gate_config.sparks_mode = "trigger-gate".into();
    let trigger_gate_menu = NativeMenuModel::new(trigger_gate_config);
    let trigger_gate_labels = trigger_gate_menu.root.children[3]
        .children
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        trigger_gate_labels,
        vec!["Mix", "Pan", "FX", "Trigger Gate", "Transpose", "XY"]
    );
}

#[test]
pub(crate) fn sparks_fx_page_is_flat_and_shows_selected_type_params() {
    let mut config = config();
    config.sparks_mode = "fx".into();
    config.sparks_fx_type = "stutter".into();
    config
        .sparks_fx_params
        .insert("rateHz".into(), serde_json::json!(12));
    let mut menu = NativeMenuModel::new(config);
    menu.state.stack = vec![3, 2];
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
pub(crate) fn sparks_aux_map_rows_show_mapped_paths() {
    let mut config = config();
    config.sparks_mode = "fx".into();
    config.sparks_fx_type = "stutter".into();
    let menu = NativeMenuModel::new(config);
    let aux_map = menu.root.children[3].children[2]
        .children
        .iter()
        .find(|item| item.label == "Aux Map")
        .expect("aux map group");
    let labels = aux_map
        .children
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();
    assert!(labels.contains(&"Rate Hz: sparks.fx.params.rateHz"));
    assert!(labels.contains(&"Depth: sparks.fx.params.depthPct"));
    assert!(labels.contains(&"Map to Grid: sparks.fx.map"));
}

#[test]
pub(crate) fn parameter_picker_exposes_binding_actions_for_aux_param_and_xy_targets() {
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
    xy_config.sparks_mode = "xy".into();
    let xy_menu = NativeMenuModel::new(xy_config);
    assert!(contains_set_binding(
        &xy_menu.root,
        "xy:x",
        "sound.velocityScalePct"
    ));
}

#[test]
pub(crate) fn link_lfo_picker_only_exposes_live_safe_targets() {
    let mut cfg = config();
    cfg.fx_buses[0].slot1_type = "delay".into();
    cfg.fx_buses[0].slot1_params =
        serde_json::json!({ "feedback": 0.35, "timeMs": 250, "mixPct": 35 });
    let menu = NativeMenuModel::new(cfg);
    let target = "linkLfos.0.target";

    assert!(contains_set_binding(
        &menu.root,
        target,
        "instruments.0.mixer.volume"
    ));
    assert!(contains_set_binding(
        &menu.root,
        target,
        "mixer.buses.0.volume"
    ));
    assert!(contains_set_binding(
        &menu.root,
        target,
        "mixer.buses.0.slot1.params.feedback"
    ));
    assert!(!contains_set_binding(
        &menu.root,
        target,
        "sound.noteLengthMs"
    ));
    assert!(!contains_set_binding(
        &menu.root,
        target,
        "mixer.buses.0.slot1.params.timeMs"
    ));
    assert!(!contains_set_binding(
        &menu.root,
        target,
        "instruments.0.synth.filter.cutoffHz"
    ));
}

#[test]
pub(crate) fn parameter_picker_includes_behavior_pulses_fx_and_global_fx_families() {
    let mut cfg = config();
    cfg.fx_buses[0].slot1_type = "delay".into();
    cfg.fx_buses[0].slot1_params =
        serde_json::json!({ "feedback": 0.35, "timeMs": 250, "mixPct": 35 });
    cfg.global_fx_slots[0] = "vinyl".into();
    cfg.global_fx_params[0] = serde_json::json!({ "cracklePct": 8, "saturationPct": 15, "warpDepthPct": 5, "mixPct": 100 });
    cfg.sparks_mode = "xy".into();
    cfg.sparks_fx_type = "stutter".into();
    let menu = NativeMenuModel::new(cfg);

    assert!(contains_set_binding(
        &menu.root,
        "xy:x",
        "layers.0.worlds.behaviorConfig.randomTickInterval"
    ));
    assert!(contains_set_binding(
        &menu.root,
        "xy:x",
        "layers.1.worlds.behaviorConfig.randomTickInterval"
    ));
    assert!(contains_set_binding(
        &menu.root,
        "xy:x",
        "layers.0.pulses.scanMode"
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
        "sparks.fx.params.rateHz"
    ));
}

#[test]
pub(crate) fn aux_click_picker_exposes_assignable_actions() {
    let menu = NativeMenuModel::new(config());
    assert!(contains_aux_click_action(
        &menu.root,
        0,
        "sample.assign:0:0"
    ));
    assert!(contains_aux_click_action(&menu.root, 0, "sparks.fx.map"));
    assert!(contains_aux_click_reset(&menu.root, 0));
}
