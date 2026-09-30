use super::*;

fn layer_item(menu: &NativeMenuModel, index: usize) -> &NativeMenuItem {
    let prefix = format!("L{}:", index + 1);
    menu.root.children[1]
        .children
        .iter()
        .find(|item| item.label.starts_with(&prefix))
        .expect("layer group")
}

#[test]
pub(crate) fn link_spec_rows_include_probability_mapping_and_axis_controls() {
    let menu = NativeMenuModel::new(config());
    let layer = layer_item(&menu, 0);
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
        .find(|item| item.label == "Set")
        .expect("note set row");
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
    let layer = layer_item(&menu, 0);
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
    cfg.link_layers[0].scan_mode = "scanning".into();
    cfg.link_layers[0].x_velocity.enabled = true;
    cfg.instrument_types[0] = "sampler".into();
    cfg.instrument_sample_velocity_levels_enabled[0] = false;
    let menu = NativeMenuModel::new(cfg);
    let layer = layer_item(&menu, 0);
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
pub(crate) fn note_set_menu_uses_canonical_scale_ids_and_compact_display_labels() {
    let mut config = config();
    config.link_layers[0].scale = "major_pentatonic".into();
    let mut menu = NativeMenuModel::new(config);
    assert!(menu.focus_item_key("layers.0.link.pitch.scale"));
    let scale = menu.current_item().clone();
    let NativeMenuValue::Enum { options, .. } = scale.value else {
        panic!("scale should be enum");
    };
    assert!(options.contains(&"major_pentatonic".to_string()));
    assert!(options.contains(&"minor_pentatonic".to_string()));
    let snapshot = menu.snapshot();
    assert!(snapshot.lines.iter().any(|line| line.contains("Maj Pent")));
}

#[test]
pub(crate) fn pitch_note_params_use_canonical_note_name_display() {
    let mut menu = NativeMenuModel::new(config());
    assert!(menu.focus_item_key("layers.0.link.pitch.lowestNote"));
    let lowest = menu.snapshot();
    assert!(lowest
        .lines
        .iter()
        .any(|line| line.starts_with("> ") && line.contains("C1 (24)")));
    assert!(menu.focus_item_key("layers.0.link.pitch.highestNote"));
    let highest = menu.snapshot();
    assert!(highest
        .lines
        .iter()
        .any(|line| line.starts_with("> ") && line.contains("C6 (84)")));
    assert!(menu.focus_item_key("layers.0.link.pitch.startingNote"));
    let starting = menu.snapshot();
    assert!(starting
        .lines
        .iter()
        .any(|line| line.starts_with("> ") && line.contains("C4 (60)")));
}

#[test]
pub(crate) fn link_instrument_targets_use_two_row_selected_presentation() {
    let mut config = config();
    config.link_layers[0].scan_mode = "scanning".into();
    config.instrument_labels[0] = "I1: An Instrument Name Longer Than One OLED Line".into();
    let mut menu = NativeMenuModel::new(config);
    let key = "layers.0.link.mapping.scanned.slot";
    assert!(menu.focus_item_key(key));

    let selected = menu.snapshot();
    let detail_row = selected.selected_row.expect("selected detail row");
    assert_eq!(selected.line_keys[detail_row].as_deref(), Some(key));
    assert_eq!(selected.lines[detail_row - 1], "  Instrument:");
    assert_eq!(
        selected.lines[detail_row],
        "> I1: An Instrument Name Longer Than One OLED Line"
    );
    assert_eq!(
        selected.full_lines[detail_row].as_deref(),
        Some("> I1: An Instrument Name Longer Than One OLED Line")
    );
    assert!(selected.lines.len() <= 7);
    let scroll = selected.scroll.expect("menu scroll metadata");
    assert!(scroll.total_rows > scroll.visible_rows);

    menu.state.editing = true;
    let editing = menu.snapshot();
    let detail_row = editing.selected_row.expect("selected edit detail row");
    assert_eq!(editing.line_keys[detail_row].as_deref(), Some(key));
    assert_eq!(editing.lines.len(), selected.lines.len());
    assert_eq!(editing.lines[detail_row - 1], "  Instrument:");
    assert_eq!(
        editing.lines[detail_row],
        ">* I1: An Instrument Name Longer Than One OLED Line"
    );
    assert_eq!(
        editing.full_lines[detail_row].as_deref(),
        Some(">* I1: An Instrument Name Longer Than One OLED Line")
    );

    let item = menu.current_item();
    assert!(matches!(
        &item.value,
        NativeMenuValue::Enum { options, .. }
            if options.contains(&"I1: An Instrument Name Longer Than One OLED Line".to_string())
    ));
    assert_eq!(
        super::format::format_item_lines(item, false, false, "bar")[0],
        "  Instrument I1"
    );
}

#[test]
pub(crate) fn link_named_selectors_outside_instrument_targets_keep_their_display() {
    let mut menu = NativeMenuModel::new(config());
    assert!(menu.focus_item_key("layers.0.link.pitch.scale"));
    assert_eq!(
        super::format::formatted_item_row_count(menu.current_item(), true, false, "bar"),
        1
    );
}

#[test]
pub(crate) fn link_event_instrument_targets_keep_item_specific_headings() {
    let mut menu = NativeMenuModel::new(config());
    for (key, heading, detail) in [
        (
            "layers.0.link.mapping.activate.slot",
            "On Inst",
            "> I1: synth",
        ),
        (
            "layers.0.link.mapping.stable.slot",
            "Hold Inst",
            "> I1: synth",
        ),
        (
            "layers.0.link.mapping.deactivate.slot",
            "Off Inst",
            "> I1: synth",
        ),
    ] {
        assert!(menu.focus_item_key(key));
        let snapshot = menu.snapshot();
        let detail_row = snapshot.selected_row.expect("selected detail row");
        assert_eq!(snapshot.lines[detail_row - 1], format!("  {heading}:"));
        assert_eq!(snapshot.lines[detail_row], detail);
    }
}

#[test]
pub(crate) fn scanned_empty_instrument_target_keeps_its_heading() {
    let mut config = config();
    config.link_layers[0].scan_mode = "scanning".into();
    let mut menu = NativeMenuModel::new(config);
    assert!(menu.focus_item_key("layers.0.link.mapping.scanned_empty.slot"));
    let snapshot = menu.snapshot();
    let detail_row = snapshot.selected_row.expect("selected detail row");
    assert_eq!(snapshot.lines[detail_row - 1], "  Empty Inst:");
    assert_eq!(snapshot.lines[detail_row], "> none");
}

#[test]
pub(crate) fn link_instrument_target_row_budgets_match_formatted_rows() {
    let mut config = config();
    config.link_layers[0].scan_mode = "scanning".into();
    let mut menu = NativeMenuModel::new(config);
    let keys = [
        "layers.0.link.mapping.scanned.slot",
        "layers.0.link.mapping.scanned_empty.slot",
        "layers.0.link.mapping.activate.slot",
        "layers.0.link.mapping.stable.slot",
        "layers.0.link.mapping.deactivate.slot",
    ];
    for key in keys {
        assert!(menu.focus_item_key(key));
        let item = menu.current_item();
        for (selected, editing) in [(false, false), (true, false), (true, true)] {
            assert_eq!(
                super::format::formatted_item_row_count(item, selected, editing, "bar"),
                super::format::format_item_lines(item, selected, editing, "bar").len(),
                "row budget for {key}, selected={selected}, editing={editing}"
            );
        }
    }
}

#[test]
pub(crate) fn event_section_rows_and_boundary_selection_use_exact_budget() {
    let mut menu = NativeMenuModel::new(config());
    assert!(menu.focus_item_key("layers.0.link.eventEnabled"));
    let events = menu.snapshot();
    let scroll = events.scroll.expect("Events scroll metadata");
    assert_eq!(scroll.total_rows, 14);
    assert_eq!(scroll.visible_rows, 7);
    assert_eq!(events.lines.len(), 7);

    assert!(menu.focus_item_key("layers.0.link.mapping.deactivate.slot"));
    let selected = menu.snapshot();
    let detail_row = selected.selected_row.expect("selected detail row");
    let scroll = selected.scroll.expect("selected Events scroll metadata");
    assert_eq!(scroll.total_rows, 15);
    assert_eq!(scroll.visible_rows, 7);
    assert!((3..=4).contains(&detail_row));
    assert_eq!(selected.lines[detail_row - 1], "  Off Inst:");
    assert_eq!(selected.lines[detail_row], "> I1: synth");
}
