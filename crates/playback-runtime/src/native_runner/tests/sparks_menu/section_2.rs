use super::*;

#[test]
pub(crate) fn entering_worlds_or_pulses_clears_active_sparks_overlay_but_keeps_selected_page() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.sparks_mode = "pan".into();
    runner.active_sparks_mode = "pan".into();
    runner.menu.rebuild(runner.menu_config());

    let worlds_response = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(runner.active_sparks_mode, "none");
    assert_eq!(runner.sparks_mode, "pan");
    assert_eq!(
        snapshot_from(&worlds_response)["display"]["title"],
        "/Build"
    );

    runner.active_sparks_mode = "pan".into();
    runner.menu.state.stack.clear();
    runner.menu.state.cursor = 1;

    let pulses_response = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(runner.active_sparks_mode, "none");
    assert_eq!(runner.sparks_mode, "pan");
    assert_eq!(snapshot_from(&pulses_response)["display"]["title"], "/Link");
}

#[test]
pub(crate) fn trigger_gate_page_edits_only_selected_layer_row() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.active_sparks_mode = "trigger-gate".into();
    runner.trigger_gate_modes = vec!["full".into(); GRID_HEIGHT];

    let changed = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 0, "y": 1 }),
            request_snapshot: None,
        })
        .unwrap();

    assert_eq!(runner.trigger_gate_modes[0], "full");
    assert_eq!(runner.trigger_gate_modes[1], "zero");
    let changed_snapshot = snapshot_from(&changed);
    let cells = led_cells(&changed_snapshot);
    let row1_zero = cells[display_index(0, 1)].as_object().unwrap();
    assert!(row1_zero["r"].as_i64().unwrap() > 0);
    assert!(row1_zero["r"].as_i64().unwrap() >= row1_zero["g"].as_i64().unwrap());
}

#[test]
pub(crate) fn trigger_gate_all_layers_button_edits_all_rows() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.active_sparks_mode = "trigger-gate".into();
    runner.trigger_gate_modes = vec!["full".into(); GRID_HEIGHT];

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 6, "y": 0 }),
            request_snapshot: None,
        })
        .unwrap();

    assert!(runner
        .trigger_gate_modes
        .iter()
        .all(|mode| mode == "custom"));
}

#[test]
pub(crate) fn fn_play_toggles_active_layer_trigger_mode_to_zero_and_restores_it() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.trigger_gate_modes[0] = "custom".into();
    runner.pulses_layers[0].trigger_probability_mode = "custom".into();

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_fn", "pressed": true }),
            request_snapshot: None,
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_s", "pressed": true }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(runner.trigger_gate_modes[0], "zero");
    assert_eq!(runner.pulses_layers[0].trigger_probability_mode, "zero");
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["layers"][0]["pulses"]["triggerProbabilityMode"],
        "zero"
    );

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_s", "pressed": true }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(runner.trigger_gate_modes[0], "custom");
    assert_eq!(runner.pulses_layers[0].trigger_probability_mode, "custom");

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_fn", "pressed": false }),
            request_snapshot: None,
        })
        .unwrap();
}

#[test]
pub(crate) fn fn_play_toggles_selected_active_layer_trigger_mode() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.active_layer_index = 2;
    runner.trigger_gate_modes = vec!["full".into(); GRID_HEIGHT];
    runner.trigger_gate_modes[2] = "custom".into();
    runner.pulses_layers[2].trigger_probability_mode = "custom".into();

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_fn", "pressed": true }),
            request_snapshot: None,
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_s", "pressed": true }),
            request_snapshot: None,
        })
        .unwrap();

    assert_eq!(runner.trigger_gate_modes[0], "full");
    assert_eq!(runner.trigger_gate_modes[2], "zero");
    assert_eq!(runner.pulses_layers[0].trigger_probability_mode, "full");
    assert_eq!(runner.pulses_layers[2].trigger_probability_mode, "zero");
}

#[test]
pub(crate) fn fn_rightmost_grid_column_selects_sparks_pages() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_fn", "pressed": true }),
            request_snapshot: None,
        })
        .unwrap();

    let mix = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 7, "y": 0 }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(runner.active_sparks_mode, "mix");
    assert_eq!(snapshot_from(&mix)["display"]["title"], "/Play");

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 7, "y": 1 }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(runner.active_sparks_mode, "pan");

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 7, "y": 3 }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(runner.active_sparks_mode, "trigger-gate");

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 7, "y": 4 }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(runner.active_sparks_mode, "transpose");

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 7, "y": 5 }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(runner.active_sparks_mode, "xy");

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 7, "y": 7 }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(runner.active_sparks_mode, "xy");
}

#[test]
pub(crate) fn sparks_transpose_layer_selection_controls_routing() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.active_sparks_mode = "transpose".into();
    runner.sparks_transpose_offsets[0] = 7;
    assert_eq!(runner.sparks_transpose_offsets_for_routing()[0], 7);

    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 0, "y": 0 }),
            request_snapshot: None,
        })
        .unwrap();

    assert!(!runner.sparks_transpose_selected[0]);
    assert_eq!(runner.sparks_transpose_offsets_for_routing()[0], 0);
}

#[test]
pub(crate) fn fn_leftmost_grid_column_switches_active_layer() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.layer_behavior_ids[2] = "sequencer".into();

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_fn", "pressed": true }),
            request_snapshot: None,
        })
        .unwrap();
    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 0, "y": 2 }),
            request_snapshot: None,
        })
        .unwrap();

    assert_eq!(runner.active_layer_index, 2);
    assert_eq!(runner.behavior.id(), "sequencer");
    assert_eq!(snapshot_from(&messages)["activeBehavior"], "sequencer");
}

#[test]
pub(crate) fn sparks_trigger_gate_leds_show_layer_modes_and_all_layers_actions() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.active_sparks_mode = "trigger-gate".into();
    runner.trigger_gate_modes[0] = "custom".into();
    runner.trigger_gate_modes[1] = "full".into();

    let snapshot = runner.snapshot().unwrap();
    let cells = led_cells(&snapshot);
    let part0_zero = cells[display_index(0, 0)].as_object().unwrap();
    let part0_custom = cells[display_index(1, 0)].as_object().unwrap();
    let part1_full = cells[display_index(2, 1)].as_object().unwrap();
    let all_custom = cells[display_index(6, 0)].as_object().unwrap();

    assert!(part0_custom["r"].as_i64().unwrap() > 100 && part0_custom["g"].as_i64().unwrap() > 80);
    assert!(
        part0_zero["r"].as_i64().unwrap() > 0
            && part0_zero["r"].as_i64().unwrap() >= part0_zero["g"].as_i64().unwrap()
    );
    assert!(part1_full["g"].as_i64().unwrap() > part1_full["r"].as_i64().unwrap());
    assert!(all_custom["r"].as_i64().unwrap() > 100 && all_custom["g"].as_i64().unwrap() > 80);
}

#[test]
pub(crate) fn back_exits_active_sparks_overlay_and_menu_context() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.active_sparks_mode = "fx".into();
    runner.menu.state.stack = vec![3];
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_a", "pressed": true }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(runner.active_sparks_mode, "none");
    assert!(runner.menu.state.stack.is_empty());
}
