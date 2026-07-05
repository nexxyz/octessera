use super::*;

#[test]
pub(crate) fn entering_l1_or_l2_clears_active_dance_overlay_but_keeps_selected_page() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.dance_mode = "pan".into();
    runner.active_dance_mode = "pan".into();
    runner.menu.rebuild(runner.menu_config());

    let l1 = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(runner.active_dance_mode, "none");
    assert_eq!(runner.dance_mode, "pan");
    assert_eq!(snapshot_from(&l1)["display"]["title"], "L1: Life");

    runner.active_dance_mode = "pan".into();
    runner.menu.state.stack.clear();
    runner.menu.state.cursor = 1;

    let l2 = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(runner.active_dance_mode, "none");
    assert_eq!(runner.dance_mode, "pan");
    assert_eq!(snapshot_from(&l2)["display"]["title"], "L2: Sense");
}

#[test]
pub(crate) fn trigger_gate_page_edits_only_selected_part_row() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.active_dance_mode = "trigger-gate".into();
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
pub(crate) fn trigger_gate_all_parts_button_edits_all_rows() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.active_dance_mode = "trigger-gate".into();
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
pub(crate) fn fn_play_toggles_active_part_trigger_mode_to_zero_and_restores_it() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.trigger_gate_modes[0] = "custom".into();
    runner.sense_parts[0].trigger_probability_mode = "custom".into();

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
    assert_eq!(runner.sense_parts[0].trigger_probability_mode, "zero");
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["parts"][0]["l2"]["triggerProbabilityMode"],
        "zero"
    );

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_s", "pressed": true }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(runner.trigger_gate_modes[0], "custom");
    assert_eq!(runner.sense_parts[0].trigger_probability_mode, "custom");

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_fn", "pressed": false }),
            request_snapshot: None,
        })
        .unwrap();
}

#[test]
pub(crate) fn fn_play_toggles_selected_active_part_trigger_mode() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.active_part_index = 2;
    runner.trigger_gate_modes = vec!["full".into(); GRID_HEIGHT];
    runner.trigger_gate_modes[2] = "custom".into();
    runner.sense_parts[2].trigger_probability_mode = "custom".into();

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
    assert_eq!(runner.sense_parts[0].trigger_probability_mode, "full");
    assert_eq!(runner.sense_parts[2].trigger_probability_mode, "zero");
}

#[test]
pub(crate) fn fn_rightmost_grid_column_selects_dance_pages() {
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
    assert_eq!(runner.active_dance_mode, "mix");
    assert_eq!(snapshot_from(&mix)["display"]["title"], "L4: Dance");

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 7, "y": 1 }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(runner.active_dance_mode, "pan");

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 7, "y": 3 }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(runner.active_dance_mode, "trigger-gate");

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 7, "y": 7 }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(runner.active_dance_mode, "trigger-gate");
}

#[test]
pub(crate) fn fn_leftmost_grid_column_switches_active_part() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.part_behavior_ids[2] = "sequencer".into();

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

    assert_eq!(runner.active_part_index, 2);
    assert_eq!(runner.behavior.id(), "sequencer");
    assert_eq!(snapshot_from(&messages)["activeBehavior"], "sequencer");
}

#[test]
pub(crate) fn dance_trigger_gate_leds_show_part_modes_and_all_parts_actions() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.active_dance_mode = "trigger-gate".into();
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
pub(crate) fn back_exits_active_dance_overlay_and_menu_context() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.active_dance_mode = "fx".into();
    runner.menu.state.stack = vec![3];
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_a", "pressed": true }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(runner.active_dance_mode, "none");
    assert!(runner.menu.state.stack.is_empty());
}
