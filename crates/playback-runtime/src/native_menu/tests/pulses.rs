use super::*;

#[test]
pub(crate) fn pulses_spec_rows_include_probability_mapping_and_axis_controls() {
    let menu = NativeMenuModel::new(config());
    let layer = &menu.root.children[1].children[4];
    let trigger_prob = layer
        .children
        .iter()
        .find(|item| item.label == "Trigger Prob.")
        .expect("trigger probability group");
    assert!(trigger_prob
        .children
        .iter()
        .any(|item| item.label == "Map Prob Grid"));
    let events = layer
        .children
        .iter()
        .find(|item| item.label == "Events")
        .expect("events group");
    for label in [
        "On Delay",
        "On Retrig",
        "Hold Delay",
        "Hold Retrig",
        "Off Delay",
        "Off Retrig",
    ] {
        assert!(events.children.iter().any(|item| item.label == label));
    }
    let note_mapping = layer
        .children
        .iter()
        .find(|item| item.label == "Note Mapping")
        .expect("note mapping group");
    let scale = note_mapping
        .children
        .iter()
        .find(|item| item.label == "Scale")
        .expect("scale row");
    assert!(
        matches!(&scale.value, NativeMenuValue::Enum { options, .. } if options.contains(&"harmonic_minor".to_string()) && options.contains(&"major_pentatonic".to_string()))
    );
    assert!(note_mapping
        .children
        .iter()
        .any(|item| item.label == "Out of Range"));
    let x_axis = layer
        .children
        .iter()
        .find(|item| item.label == "X Axis")
        .expect("x axis group");
    let labels = x_axis
        .children
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        labels,
        vec![
            "Slot 1: (none)",
            "Slot 1 Invert",
            "Slot 2: (none)",
            "Slot 2 Invert",
            "Pitch Steps",
            "Velocity",
            "Filter Cutoff",
            "Filter Res"
        ]
    );
    assert!(contains_set_binding(
        x_axis,
        "param:0:x:0",
        "sound.noteLengthMs"
    ));

    let y_axis = layer
        .children
        .iter()
        .find(|item| item.label == "Y Axis")
        .expect("y axis group");
    assert!(contains_set_binding(
        y_axis,
        "param:0:y:0",
        "sound.noteLengthMs"
    ));
}

#[test]
pub(crate) fn conditional_rows_follow_scan_lane_and_sampler_state() {
    let menu = NativeMenuModel::new(config());
    let layer = &menu.root.children[1].children[4];
    let scanning = layer
        .children
        .iter()
        .find(|item| item.label == "Scanning")
        .expect("scanning group");
    assert_eq!(scanning.children.len(), 1);
    assert_eq!(scanning.children[0].label, "Scan Mode");
    let x_axis = layer
        .children
        .iter()
        .find(|item| item.label == "X Axis")
        .expect("x axis group");
    let velocity = x_axis
        .children
        .iter()
        .find(|item| item.label == "Velocity")
        .expect("velocity group");
    assert_eq!(velocity.children.len(), 1);
    assert_eq!(velocity.children[0].label, "Enabled");

    let mut cfg = config();
    cfg.pulses_layers[0].scan_mode = "scanning".into();
    cfg.pulses_layers[0].x_velocity.enabled = true;
    cfg.instrument_types[0] = "sampler".into();
    cfg.instrument_sample_velocity_levels_enabled[0] = false;
    let menu = NativeMenuModel::new(cfg);
    let layer = &menu.root.children[1].children[4];
    let scanning = layer
        .children
        .iter()
        .find(|item| item.label == "Scanning")
        .expect("scanning group");
    let scan_unit = scanning
        .children
        .iter()
        .find(|item| item.label == "Scan Unit")
        .expect("scan unit row");
    assert!(matches!(
        &scan_unit.value,
        NativeMenuValue::Enum { options, .. }
            if options.contains(&"1/32".to_string()) && options.contains(&"1/16T".to_string())
    ));
    assert!(scanning
        .children
        .iter()
        .any(|item| item.label == "Scan Axis"));
    let x_axis = layer
        .children
        .iter()
        .find(|item| item.label == "X Axis")
        .expect("x axis group");
    let velocity = x_axis
        .children
        .iter()
        .find(|item| item.label == "Velocity")
        .expect("velocity group");
    assert!(velocity.children.iter().any(|item| item.label == "Curve"));
    let sampler = menu.root.children[2].children[0].children[0]
        .children
        .iter()
        .find(|item| item.label == "Sampler")
        .expect("sampler group");
    assert!(!sampler
        .children
        .iter()
        .any(|item| item.label == "Velocity Levels"
            && !matches!(item.value, NativeMenuValue::Bool { .. })));
}

#[test]
pub(crate) fn scale_menu_uses_legacy_scale_ids_and_display_labels() {
    let mut config = config();
    config.pulses_layers[0].scale = "major_pentatonic".into();
    let mut menu = NativeMenuModel::new(config);
    menu.state.stack = vec![1, 4, 3];
    menu.state.cursor = 3;
    let scale = menu.current_siblings()[3].clone();
    let NativeMenuValue::Enum { options, .. } = scale.value else {
        panic!("scale should be enum");
    };
    assert!(options.contains(&"major_pentatonic".to_string()));
    assert!(options.contains(&"minor_pentatonic".to_string()));
    let snapshot = menu.snapshot();
    assert!(snapshot
        .lines
        .iter()
        .any(|line| line.contains("Maj Pentatonic")));
}

#[test]
pub(crate) fn pitch_note_params_use_legacy_note_name_display() {
    let mut menu = NativeMenuModel::new(config());
    menu.state.stack = vec![1, 4, 3];
    menu.state.cursor = 0;
    let lowest = menu.snapshot();
    assert!(lowest
        .lines
        .iter()
        .any(|line| line.starts_with("> ") && line.contains("C1 (24)")));
    menu.state.cursor = 1;
    let highest = menu.snapshot();
    assert!(highest
        .lines
        .iter()
        .any(|line| line.starts_with("> ") && line.contains("C6 (84)")));
    menu.state.cursor = 2;
    let starting = menu.snapshot();
    assert!(starting
        .lines
        .iter()
        .any(|line| line.starts_with("> ") && line.contains("C4 (60)")));
}
