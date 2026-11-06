use super::*;

fn play_page<'a>(menu: &'a NativeMenuModel, label: &str) -> &'a NativeMenuItem {
    menu.root.children[3]
        .children
        .iter()
        .find(|item| item.label == label)
        .unwrap()
}

#[test]
pub(crate) fn play_spec_rows_show_only_selected_play_page_controls() {
    let menu = NativeMenuModel::new(config());
    let play = &menu.root.children[3];
    let labels = play
        .children
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        labels,
        vec![
            "Mix",
            "Pan",
            "Route",
            "Tempo",
            "LFO",
            "Faders",
            "FX",
            "XY",
            "Transpose",
            "Rate",
            "Trigger Gate"
        ]
    );
    let sense = &menu.root.children[1];
    assert_eq!(sense.children[0].label, "BPM");
    assert_eq!(sense.children[1].label, "Swing A");
    assert_eq!(sense.children[2].label, "Swing B");
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
        .find(|item| item.label == "Swing A")
        .unwrap();
    assert!(matches!(
        swing.value,
        NativeMenuValue::Number {
            value: 4,
            min: 1,
            max: 7,
            step: 1,
            ..
        }
    ));

    let mut fx_config = config();
    fx_config.play_mode = "fx".into();
    let fx_menu = NativeMenuModel::new(fx_config);
    let fx_labels = fx_menu.root.children[3]
        .children
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        fx_labels,
        vec![
            "Mix",
            "Pan",
            "Route",
            "Tempo",
            "LFO",
            "Faders",
            "FX",
            "XY",
            "Transpose",
            "Rate",
            "Trigger Gate"
        ]
    );
    let fx_labels = play_page(&fx_menu, "FX")
        .children
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();
    assert!(fx_labels.contains(&"FX Type"));
    assert!(fx_labels.contains(&"Target"));

    let mut trigger_gate_config = config();
    trigger_gate_config.play_mode = "trigger-gate".into();
    let trigger_gate_menu = NativeMenuModel::new(trigger_gate_config);
    let trigger_gate_labels = trigger_gate_menu.root.children[3]
        .children
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        trigger_gate_labels,
        vec![
            "Mix",
            "Pan",
            "Route",
            "Tempo",
            "LFO",
            "Faders",
            "FX",
            "XY",
            "Transpose",
            "Rate",
            "Trigger Gate"
        ]
    );
}

#[test]
pub(crate) fn xy_menu_orders_and_formats_smoothing() {
    let mut config = config();
    config.play_mode = "xy".into();
    let mut menu = NativeMenuModel::new(config);
    let xy = play_page(&menu, "XY");
    let labels = xy
        .children
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        labels,
        vec![
            "X Axis: (none)",
            "Y Axis: (none)",
            "Glide",
            "Invert X",
            "Invert Y",
            "Release",
        ]
    );
    let smoothing = menu.item_for_key("play.xy.smoothingMs").unwrap();
    assert!(matches!(
        smoothing.value,
        NativeMenuValue::Number {
            value: 80,
            min: 0,
            max: 5000,
            ..
        }
    ));
    assert!(menu.focus_item_key("play.xy.smoothingMs"));
    menu.set_number_value_for_key("play.xy.smoothingMs", 0);
    assert_eq!(
        menu.snapshot()
            .full_lines
            .iter()
            .flatten()
            .find(|line| line.contains("Glide"))
            .map(String::as_str),
        Some("  Glide Off")
    );
    menu.set_number_value_for_key("play.xy.smoothingMs", 120);
    assert_eq!(
        menu.snapshot()
            .full_lines
            .iter()
            .flatten()
            .find(|line| line.contains("Glide"))
            .map(String::as_str),
        Some("  Glide 120 ms")
    );
}

#[test]
pub(crate) fn play_fx_page_is_flat_and_shows_selected_type_params() {
    let mut config = config();
    config.play_mode = "fx".into();
    config.play_fx_type = "stutter".into();
    config
        .play_fx_params
        .insert("rateHz".into(), serde_json::json!(12));
    let mut menu = NativeMenuModel::new(config);
    menu.open_labels(&["Play", "FX"]);
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
pub(crate) fn pitch_fx_page_exposes_independent_slide_rows() {
    let mut config = config();
    config.play_mode = "fx".into();
    config.play_fx_type = "pitch_shift".into();
    let mut menu = NativeMenuModel::new(config);
    menu.open_labels(&["Play", "FX"]);
    let rows = menu.current_siblings();
    for (label, key, value) in [
        ("Slide In", "play.fx.params.slideInMs", 120),
        ("Slide Out", "play.fx.params.slideOutMs", 180),
    ] {
        let row = rows
            .iter()
            .find(|item| item.label == label)
            .unwrap_or_else(|| panic!("missing {label}"));
        assert_eq!(row.key.as_deref(), Some(key));
        assert!(matches!(
            row.value,
            NativeMenuValue::Number {
                value: actual,
                min: 10,
                max: 3000,
                step: 10, ..
            } if actual == value
        ));
    }
}

#[test]
pub(crate) fn play_aux_map_rows_show_the_mapped_controls() {
    let mut config = config();
    config.play_mode = "fx".into();
    config.play_fx_type = "stutter".into();
    let menu = NativeMenuModel::new(config);
    let aux_map = play_page(&menu, "FX")
        .children
        .iter()
        .find(|item| item.label == "Aux Map")
        .expect("aux map group");
    let labels = aux_map
        .children
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();
    assert!(labels.contains(&"Rate Hz"));
    assert!(labels.contains(&"Depth"));
    assert!(labels.contains(&"Map to Grid"));
}

#[test]
pub(crate) fn parameter_picker_exposes_binding_actions_for_aux_param_and_xy_targets() {
    let menu = NativeMenuModel::new(config());
    assert!(contains_set_binding(
        &menu.browsable_root(),
        "aux:0:turn",
        "sound.noteLengthMs"
    ));
    assert!(contains_set_binding(
        &menu.browsable_root(),
        "param:0:x:0",
        "instruments.0.mixer.volume"
    ));

    let mut xy_config = config();
    xy_config.play_mode = "xy".into();
    let xy_menu = NativeMenuModel::new(xy_config);
    assert!(contains_set_binding(
        &xy_menu.browsable_root(),
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
        &menu.browsable_root(),
        target,
        "instruments.0.mixer.volume"
    ));
    assert!(contains_set_binding(
        &menu.browsable_root(),
        target,
        "mixer.buses.0.volume"
    ));
    assert!(contains_set_binding(
        &menu.browsable_root(),
        target,
        "mixer.buses.0.slot1.params.feedback"
    ));
    assert!(!contains_set_binding(
        &menu.browsable_root(),
        target,
        "sound.noteLengthMs"
    ));
    assert!(!contains_set_binding(
        &menu.browsable_root(),
        target,
        "mixer.buses.0.slot1.params.timeMs"
    ));
    assert!(!contains_set_binding(
        &menu.browsable_root(),
        target,
        "instruments.0.synth.filter.cutoffHz"
    ));
}

#[test]
pub(crate) fn parameter_picker_includes_behavior_link_fx_and_global_fx_families() {
    let mut cfg = config();
    cfg.fx_buses[0].slot1_type = "delay".into();
    cfg.fx_buses[0].slot1_params =
        serde_json::json!({ "feedback": 0.35, "timeMs": 250, "mixPct": 35 });
    cfg.global_fx_slots[0] = "vinyl".into();
    cfg.global_fx_params[0] = serde_json::json!({ "cracklePct": 8, "saturationPct": 15, "warpDepthPct": 5, "mixPct": 100 });
    cfg.play_mode = "xy".into();
    cfg.play_fx_type = "stutter".into();
    let menu = NativeMenuModel::new(cfg);

    assert!(contains_set_binding(
        &menu.browsable_root(),
        "xy:x",
        "layers.0.build.behaviorConfig.randomTickInterval"
    ));
    assert!(contains_set_binding(
        &menu.browsable_root(),
        "xy:x",
        "layers.1.build.behaviorConfig.randomTickInterval"
    ));
    assert!(contains_set_binding(
        &menu.browsable_root(),
        "xy:x",
        "layers.0.link.scanMode"
    ));
    assert!(contains_set_binding(
        &menu.browsable_root(),
        "xy:x",
        "instruments.0.synth.filter.cutoffHz"
    ));
    assert!(contains_set_binding(
        &menu.browsable_root(),
        "xy:x",
        "mixer.buses.0.slot1.params.feedback"
    ));
    assert!(contains_set_binding(
        &menu.browsable_root(),
        "xy:x",
        "mixer.master.slots.0.params.cracklePct"
    ));
    assert!(contains_set_binding(
        &menu.browsable_root(),
        "xy:x",
        "play.fx.params.rateHz"
    ));
}

#[test]
pub(crate) fn aux_click_picker_exposes_assignable_actions() {
    let menu = NativeMenuModel::new(config());
    assert!(contains_aux_click_action(
        &menu.browsable_root(),
        0,
        "sample.assign:0:0"
    ));
    assert!(contains_aux_click_action(
        &menu.browsable_root(),
        0,
        "play.fx.map"
    ));
    assert!(contains_aux_click_reset(&menu.browsable_root(), 0));
}

#[test]
fn a_picker_entry_lookup_matches_the_entry_in_the_opened_picker() {
    let menu = NativeMenuModel::new(config());
    let root = menu.browsable_root();
    for target in ["aux:0:turn", "param:0:x:0", "linkLfos.0.target"] {
        let picker = find_item_by_key(&root, target).expect(target);
        let mut entries = Vec::new();
        collect_set_bindings(picker, &mut entries);
        assert!(entries.len() > 10, "{target}");
        for entry in entries {
            let key = entry.key.as_deref().expect("entry key");
            assert_eq!(menu.item_for_key(key).as_ref(), Some(entry), "{key}");
        }
    }
}

fn collect_set_bindings<'a>(node: &'a NativeMenuItem, entries: &mut Vec<&'a NativeMenuItem>) {
    if matches!(
        node.value,
        NativeMenuValue::Action(NativeMenuAction::SetParamBinding { .. })
    ) && node
        .key
        .as_deref()
        .is_some_and(|key| !key.ends_with(".current"))
    {
        entries.push(node);
    }
    for child in &node.children {
        collect_set_bindings(child, entries);
    }
}

#[test]
pub(crate) fn play_fx_rows_show_short_type_names_and_note_values() {
    use crate::native_menu::format_values::format_display_value;
    for (fx_type, shown) in [
        ("trance_gate", "gate"),
        ("dub_throw", "dub"),
        ("tape_stop", "tape_stop"),
        ("hp_riser", "hp_riser"),
    ] {
        assert_eq!(format_display_value(Some("play.fx.type"), fx_type), shown);
    }
    for (index, note) in [(0, "1/32T"), (3, "1/16"), (7, "1/4"), (9, "1/2")] {
        assert_eq!(
            format_display_value(Some("play.fx.params.rateNote"), index),
            note
        );
        assert_eq!(
            format_display_value(Some("play.fx.params.delayNote"), index),
            note
        );
    }
}
