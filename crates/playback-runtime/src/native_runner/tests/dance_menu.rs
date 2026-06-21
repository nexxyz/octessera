use super::*;

#[test]
fn dance_page_menu_edits_selected_and_active_mode() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();

    for _ in 0..3 {
        let _ = runner.send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
        });
    }
    let _ = runner.send(HostMessage::DeviceInput {
        input: json!({ "type": "encoder_press", "id": "main" }),
    });
    let _ = runner.send(HostMessage::DeviceInput {
        input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
    });
    let edit = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    assert_eq!(snapshot_from(&edit)["display"]["title"], "L4: Dance");
    assert_eq!(snapshot_from(&edit)["display"]["editing"], true);

    for mode in ["pan", "fx", "trigger-gate", "xy", "xy"] {
        let changed = runner
            .send(HostMessage::DeviceInput {
                input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
            })
            .unwrap();
        assert_eq!(snapshot_from(&changed)["danceMode"], mode);
        assert_eq!(snapshot_from(&changed)["activeDanceMode"], mode);
    }

    for mode in ["trigger-gate", "fx", "pan", "mix", "mix"] {
        let changed = runner
            .send(HostMessage::DeviceInput {
                input: json!({ "type": "encoder_turn", "delta": -1, "id": "main" }),
            })
            .unwrap();
        assert_eq!(snapshot_from(&changed)["danceMode"], mode);
        assert_eq!(snapshot_from(&changed)["activeDanceMode"], mode);
    }
}

#[test]
fn changing_dance_page_rebuilds_visible_menu_rows_for_selected_page() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();

    for _ in 0..3 {
        let _ = runner.send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
        });
    }
    let _ = runner.send(HostMessage::DeviceInput {
        input: json!({ "type": "encoder_press", "id": "main" }),
    });
    let _ = runner.send(HostMessage::DeviceInput {
        input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
    });
    let _ = runner.send(HostMessage::DeviceInput {
        input: json!({ "type": "encoder_press", "id": "main" }),
    });

    let fx = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 2, "id": "main" }),
        })
        .unwrap();
    let fx_snapshot = snapshot_from(&fx);
    let fx_lines = fx_snapshot["display"]["lines"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|line| line.as_str())
        .collect::<Vec<_>>();
    assert!(fx_lines.iter().any(|line| line.contains("FX Type")));
    assert!(fx_lines.iter().any(|line| line.contains("Target")));

    let xy = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 2, "id": "main" }),
        })
        .unwrap();
    let xy_snapshot = snapshot_from(&xy);
    let xy_lines = xy_snapshot["display"]["lines"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|line| line.as_str())
        .collect::<Vec<_>>();
    assert!(!xy_lines.iter().any(|line| line.contains("FX Type")));
    assert!(xy_lines.iter().any(|line| line.contains("X Axis")));
}

#[test]
fn entering_dance_menu_activates_selected_page_and_overlay() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.dance_mode = "pan".into();
    runner.active_dance_mode = "none".into();
    runner.menu.rebuild(runner.menu_config());

    for _ in 0..3 {
        let _ = runner.send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
        });
    }
    let entered = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();

    assert_eq!(runner.active_dance_mode, "pan");
    let snapshot = snapshot_from(&entered);
    assert_eq!(snapshot["display"]["title"], "L4: Dance");
    assert_eq!(snapshot["danceMode"], "pan");
    assert_eq!(snapshot["activeDanceMode"], "pan");

    let cells = snapshot["leds"]["cells"].as_array().unwrap();
    let left = cells[3].as_object().unwrap();
    let right = cells[4].as_object().unwrap();
    assert!(left["r"].as_i64().unwrap() > 100 && left["g"].as_i64().unwrap() > 100);
    assert!(right["r"].as_i64().unwrap() > 100 && right["g"].as_i64().unwrap() > 100);
}

#[test]
fn entering_l1_or_l2_clears_active_dance_overlay_but_keeps_selected_page() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.dance_mode = "pan".into();
    runner.active_dance_mode = "pan".into();
    runner.menu.rebuild(runner.menu_config());

    let l1 = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
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
        })
        .unwrap();
    assert_eq!(runner.active_dance_mode, "none");
    assert_eq!(runner.dance_mode, "pan");
    assert_eq!(snapshot_from(&l2)["display"]["title"], "L2: Sense");
}

#[test]
fn trigger_gate_page_edits_only_selected_part_row() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.active_dance_mode = "trigger-gate".into();
    runner.trigger_gate_modes = vec!["full".into(); GRID_HEIGHT];

    let changed = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 0, "y": 1 }),
        })
        .unwrap();

    assert_eq!(runner.trigger_gate_modes[0], "full");
    assert_eq!(runner.trigger_gate_modes[1], "zero");
    let cells = snapshot_from(&changed)["leds"]["cells"]
        .as_array()
        .unwrap()
        .clone();
    let row1_zero = cells[display_index(0, 1)].as_object().unwrap();
    assert!(row1_zero["r"].as_i64().unwrap() > 0);
    assert!(row1_zero["r"].as_i64().unwrap() >= row1_zero["g"].as_i64().unwrap());
}

#[test]
fn trigger_gate_all_parts_button_edits_all_rows() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.active_dance_mode = "trigger-gate".into();
    runner.trigger_gate_modes = vec!["full".into(); GRID_HEIGHT];

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 6, "y": 0 }),
        })
        .unwrap();

    assert!(runner
        .trigger_gate_modes
        .iter()
        .all(|mode| mode == "custom"));
}

#[test]
fn fn_play_toggles_active_part_trigger_mode_to_zero_and_restores_it() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.trigger_gate_modes[0] = "custom".into();
    runner.sense_parts[0].trigger_probability_mode = "custom".into();

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_fn", "pressed": true }),
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_s", "pressed": true }),
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
        })
        .unwrap();
    assert_eq!(runner.trigger_gate_modes[0], "custom");
    assert_eq!(runner.sense_parts[0].trigger_probability_mode, "custom");

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_fn", "pressed": false }),
        })
        .unwrap();
}

#[test]
fn fn_play_toggles_selected_active_part_trigger_mode() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.active_part_index = 2;
    runner.trigger_gate_modes = vec!["full".into(); GRID_HEIGHT];
    runner.trigger_gate_modes[2] = "custom".into();
    runner.sense_parts[2].trigger_probability_mode = "custom".into();

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_fn", "pressed": true }),
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_s", "pressed": true }),
        })
        .unwrap();

    assert_eq!(runner.trigger_gate_modes[0], "full");
    assert_eq!(runner.trigger_gate_modes[2], "zero");
    assert_eq!(runner.sense_parts[0].trigger_probability_mode, "full");
    assert_eq!(runner.sense_parts[2].trigger_probability_mode, "zero");
}

#[test]
fn fn_rightmost_grid_column_selects_dance_pages() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_fn", "pressed": true }),
        })
        .unwrap();

    let mix = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 7, "y": 0 }),
        })
        .unwrap();
    assert_eq!(runner.active_dance_mode, "mix");
    assert_eq!(snapshot_from(&mix)["display"]["title"], "L4: Dance");

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 7, "y": 1 }),
        })
        .unwrap();
    assert_eq!(runner.active_dance_mode, "pan");

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 7, "y": 3 }),
        })
        .unwrap();
    assert_eq!(runner.active_dance_mode, "trigger-gate");

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 7, "y": 7 }),
        })
        .unwrap();
    assert_eq!(runner.active_dance_mode, "trigger-gate");
}

#[test]
fn fn_leftmost_grid_column_switches_active_part() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.part_behavior_ids[2] = "sequencer".into();

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_fn", "pressed": true }),
        })
        .unwrap();
    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 0, "y": 2 }),
        })
        .unwrap();

    assert_eq!(runner.active_part_index, 2);
    assert_eq!(runner.behavior.id(), "sequencer");
    assert_eq!(snapshot_from(&messages)["activeBehavior"], "sequencer");
}

#[test]
fn fn_overlay_shows_active_parts_and_dance_page_options() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.active_dance_mode = "pan".into();
    runner.dance_mode = "pan".into();
    runner.part_behavior_ids[1] = "none".into();
    runner.part_behavior_ids[2] = "life".into();
    runner.ui.fn_held = true;

    let snapshot = runner.snapshot().unwrap();
    let cells = snapshot["leds"]["cells"].as_array().unwrap();
    let active_part = cells[display_index(0, 0)].as_object().unwrap();
    let none_part = cells[display_index(0, 1)].as_object().unwrap();
    let configured_part = cells[display_index(0, 2)].as_object().unwrap();
    let selected_page = cells[display_index(GRID_WIDTH - 1, 1)].as_object().unwrap();
    let middle_cell = cells[display_index(3, 3)].as_object().unwrap();

    assert_eq!(active_part["r"].as_i64().unwrap(), 0);
    assert_eq!(active_part["g"].as_i64().unwrap(), 120);
    assert_eq!(active_part["b"].as_i64().unwrap(), 0);
    assert_eq!(none_part["r"].as_i64().unwrap(), 0);
    assert_eq!(none_part["g"].as_i64().unwrap(), 48);
    assert_eq!(none_part["b"].as_i64().unwrap(), 23);
    assert_eq!(configured_part, active_part);
    assert!(selected_page["g"].as_i64().unwrap() > 0 || selected_page["b"].as_i64().unwrap() > 0);
    assert!(middle_cell["r"].as_i64().unwrap() < 70);
    assert!(middle_cell["g"].as_i64().unwrap() < 70);
    assert!(middle_cell["b"].as_i64().unwrap() < 70);
}

#[test]
fn fn_overlay_highlights_active_part_when_not_in_dance_mode() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.active_dance_mode = "none".into();
    runner.dance_mode = "mix".into();
    runner.part_behavior_ids[1] = "none".into();
    runner.part_behavior_ids[2] = "life".into();
    runner.ui.fn_held = true;

    let snapshot = runner.snapshot().unwrap();
    let cells = snapshot["leds"]["cells"].as_array().unwrap();
    let active_part = cells[display_index(0, 0)].as_object().unwrap();
    let none_part = cells[display_index(0, 1)].as_object().unwrap();
    let configured_part = cells[display_index(0, 2)].as_object().unwrap();
    let dance_page = cells[display_index(GRID_WIDTH - 1, 0)].as_object().unwrap();

    assert!(active_part["g"].as_i64().unwrap() > 0);
    assert!(active_part["b"].as_i64().unwrap() > 0);
    assert_eq!(active_part["g"], active_part["b"]);
    assert!(active_part["r"].as_i64().unwrap() < active_part["g"].as_i64().unwrap());
    assert_eq!(none_part["r"].as_i64().unwrap(), 0);
    assert_eq!(none_part["g"].as_i64().unwrap(), 48);
    assert_eq!(none_part["b"].as_i64().unwrap(), 23);
    assert!(configured_part["g"].as_i64().unwrap() > 0);
    assert!(configured_part["g"].as_i64().unwrap() < active_part["g"].as_i64().unwrap());
    assert_eq!(dance_page["r"].as_i64().unwrap(), 0);
    assert_eq!(dance_page["g"].as_i64().unwrap(), 60);
    assert_eq!(dance_page["b"].as_i64().unwrap(), 60);
}

#[test]
fn fn_overlay_dims_fx_grid_cells_when_dance_mode_is_fx() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.ui.fn_held = true;
    runner.active_dance_mode = "fx".into();
    runner.dance_mode = "fx".into();

    let snapshot = runner.snapshot().unwrap();
    let cells = snapshot["leds"]["cells"].as_array().unwrap();
    let mid_cell = cells[display_index(2, 2)].as_object().unwrap();
    let fx_page = cells[display_index(GRID_WIDTH - 1, 2)].as_object().unwrap();
    let part_cell = cells[display_index(0, 0)].as_object().unwrap();

    assert!(mid_cell["r"].as_i64().unwrap() < 20);
    assert!(mid_cell["g"].as_i64().unwrap() < 20);
    assert!(mid_cell["b"].as_i64().unwrap() < 20);
    assert!(fx_page["g"].as_i64().unwrap() > 100 && fx_page["g"].as_i64().unwrap() < 200);
    assert!(fx_page["g"].as_i64().unwrap() > 0 || fx_page["b"].as_i64().unwrap() > 0);
    assert!(part_cell["g"].as_i64().unwrap() > 0);
}

#[test]
fn dance_trigger_gate_leds_show_part_modes_and_all_parts_actions() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.active_dance_mode = "trigger-gate".into();
    runner.trigger_gate_modes[0] = "custom".into();
    runner.trigger_gate_modes[1] = "full".into();

    let snapshot = runner.snapshot().unwrap();
    let cells = snapshot["leds"]["cells"].as_array().unwrap();
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
fn back_exits_active_dance_overlay_and_menu_context() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.active_dance_mode = "fx".into();
    runner.menu.state.stack = vec![3];
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_a", "pressed": true }),
        })
        .unwrap();
    assert_eq!(runner.active_dance_mode, "none");
    assert!(runner.menu.state.stack.is_empty());
}

#[test]
fn fn_left_column_selects_parts_while_in_dance_overlay_and_exits_overlay() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.active_dance_mode = "fx".into();
    runner.ui.fn_held = true;

    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 0, "y": 1 }),
        })
        .unwrap();

    assert_eq!(runner.active_part_index, 1);
    assert_eq!(runner.active_dance_mode, "none");
}
