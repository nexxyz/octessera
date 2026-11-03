use super::*;

#[test]
pub(crate) fn pan_page_grid_edit_sends_live_mixer_commands() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let _ = runner.messages_with_snapshot().unwrap();
    runner.active_play_mode = "pan".into();

    let direct = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 6, "y": 0 }),
            request_snapshot: None,
        })
        .unwrap();

    assert_eq!(runner.instruments[0].pan_pos, 27);
    assert!(direct.iter().any(|message| matches!(
        message,
        RunnerMessage::AudioCommands { commands }
            if commands.iter().any(|command| matches!(
                command,
                RuntimeAudioCommand::SetInstrumentMixer {
                    instrument_slot: 0,
                    volume_pct: None,
                    pan_pos: Some(27),
                    ..
                }
            ))
    )));

    runner.instruments[0].route = "fx_bus_1".into();
    let bus = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 1, "y": 0 }),
            request_snapshot: None,
        })
        .unwrap();

    assert_eq!(runner.instruments[0].pan_pos, 5);
    assert_eq!(runner.fx_buses[0].pan_pos, 5);
    assert!(bus.iter().any(|message| matches!(
        message,
        RunnerMessage::AudioCommands { commands }
            if commands.iter().any(|command| matches!(
                command,
                RuntimeAudioCommand::SetInstrumentMixer {
                    instrument_slot: 0,
                    volume_pct: None,
                    pan_pos: Some(5),
                    ..
                }
            )) && commands.iter().any(|command| matches!(
                command,
                RuntimeAudioCommand::SetFxBusMixer {
                    bus_index: 0,
                    pan_pos: Some(5),
                    ..
                }
            ))
    )));
}

#[test]
pub(crate) fn entering_build_or_link_clears_active_play_overlay_but_keeps_selected_page() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.play_mode = "pan".into();
    runner.active_play_mode = "pan".into();
    runner.menu.rebuild(runner.menu_config());

    let build_response = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(runner.active_play_mode, "none");
    assert_eq!(runner.play_mode, "pan");
    assert_eq!(snapshot_from(&build_response)["display"]["title"], "/Build");

    runner.active_play_mode = "pan".into();
    runner.menu.state.stack.clear();
    runner.menu.select_label("Link");

    let link_response = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(runner.active_play_mode, "none");
    assert_eq!(runner.play_mode, "pan");
    assert_eq!(snapshot_from(&link_response)["display"]["title"], "/Link");
}

#[test]
pub(crate) fn trigger_gate_page_edits_only_selected_layer_row() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.active_play_mode = "trigger-gate".into();
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
    let tiles = led_tiles(&changed_snapshot);
    let row1_zero = tiles[display_index(0, 1)].as_object().unwrap();
    assert!(row1_zero["r"].as_i64().unwrap() > 0);
    assert!(row1_zero["r"].as_i64().unwrap() >= row1_zero["g"].as_i64().unwrap());
}

#[test]
pub(crate) fn trigger_gate_all_layers_button_edits_all_rows() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.active_play_mode = "trigger-gate".into();
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
pub(crate) fn combined_modifier_left_column_toggles_layer_trigger_mode_to_zero_and_restores_it() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.trigger_gate_modes[0] = "custom".into();
    runner.link_layers[0].trigger_probability_mode = "custom".into();

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_combined_modifier", "pressed": true }),
            request_snapshot: None,
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 0, "y": 0 }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(runner.trigger_gate_modes[0], "zero");
    assert_eq!(runner.link_layers[0].trigger_probability_mode, "zero");
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["layers"][0]["link"]["triggerProbabilityMode"],
        "zero"
    );

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 0, "y": 0 }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(runner.trigger_gate_modes[0], "custom");
    assert_eq!(runner.link_layers[0].trigger_probability_mode, "custom");

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_combined_modifier", "pressed": false }),
            request_snapshot: None,
        })
        .unwrap();
}

#[test]
pub(crate) fn combined_modifier_left_column_toggles_selected_layer_trigger_mode() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.active_layer_index = 2;
    runner.trigger_gate_modes = vec!["full".into(); GRID_HEIGHT];
    runner.trigger_gate_modes[2] = "custom".into();
    runner.link_layers[2].trigger_probability_mode = "custom".into();

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_combined_modifier", "pressed": true }),
            request_snapshot: None,
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 0, "y": 2 }),
            request_snapshot: None,
        })
        .unwrap();

    assert_eq!(runner.trigger_gate_modes[0], "full");
    assert_eq!(runner.trigger_gate_modes[2], "zero");
    assert_eq!(runner.link_layers[0].trigger_probability_mode, "full");
    assert_eq!(runner.link_layers[2].trigger_probability_mode, "zero");
}

#[test]
pub(crate) fn fn_rightmost_grid_column_selects_technical_play_pages() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_fn", "pressed": true }),
            request_snapshot: None,
        })
        .unwrap();

    let mix = press_grid(&mut runner, 7, 0);
    assert_eq!(runner.active_play_mode, "mix");
    assert_eq!(snapshot_from(&mix)["display"]["title"], "/Play");
    assert_eq!(runner.menu.current_key(), Some("play.page.mix"));

    let _ = press_grid(&mut runner, 7, 1);
    assert_eq!(runner.active_play_mode, "pan");
    assert_eq!(runner.menu.current_key(), Some("play.page.pan"));

    let _ = press_grid(&mut runner, 7, 3);
    assert_eq!(runner.active_play_mode, "tempo");
    assert_eq!(runner.menu.current_key(), Some("play.page.tempo"));

    for free_row in [2, 4, 5, 6, 7] {
        let _ = press_grid(&mut runner, 7, free_row);
        assert_eq!(runner.active_play_mode, "tempo", "row {free_row}");
    }
}

#[test]
pub(crate) fn shift_fn_rightmost_grid_column_selects_musical_play_pages() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    for button in ["button_shift", "button_fn"] {
        let _ = runner
            .send(HostMessage::DeviceInput {
                input: json!({ "type": button, "pressed": true }),
                request_snapshot: None,
            })
            .unwrap();
    }
    assert!(runner.display.ui.combined_modifier_held);

    for (row, mode, toast) in [
        (0, "fx", "Play: fx"),
        (1, "xy", "Play: xy"),
        (2, "transpose", "Play: transpose"),
        (3, "rate", "Play: rate"),
        (4, "trigger-gate", "Play: trig"),
    ] {
        let messages = press_grid(&mut runner, 7, row);
        assert_eq!(runner.active_play_mode, mode);
        assert_eq!(snapshot_from(&messages)["display"]["toast"], toast);
        assert_eq!(
            runner.menu.current_key(),
            Some(format!("play.page.{mode}").as_str())
        );
    }
    for free_row in [5, 6, 7] {
        let _ = press_grid(&mut runner, 7, free_row);
        assert_eq!(runner.active_play_mode, "trigger-gate", "row {free_row}");
    }
}

fn press_grid(runner: &mut NativeRunner, x: usize, y: usize) -> Vec<RunnerMessage> {
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": x, "y": y }),
            request_snapshot: None,
        })
        .unwrap()
}

#[test]
pub(crate) fn play_transpose_layer_selection_controls_routing() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.active_play_mode = "transpose".into();
    runner.play_transpose_offsets[0] = 7;
    assert_eq!(runner.play_transpose_offsets_for_routing()[0], 7);

    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 0, "y": 0 }),
            request_snapshot: None,
        })
        .unwrap();

    assert!(!runner.play_transpose_selected[0]);
    assert_eq!(runner.play_transpose_offsets_for_routing()[0], 0);
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
pub(crate) fn play_trigger_gate_leds_show_layer_modes_and_all_layers_actions() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.active_play_mode = "trigger-gate".into();
    runner.trigger_gate_modes[0] = "custom".into();
    runner.trigger_gate_modes[1] = "full".into();

    let snapshot = runner.snapshot().unwrap();
    let tiles = led_tiles(&snapshot);
    let part0_zero = tiles[display_index(0, 0)].as_object().unwrap();
    let part0_custom = tiles[display_index(1, 0)].as_object().unwrap();
    let part1_full = tiles[display_index(2, 1)].as_object().unwrap();
    let all_custom = tiles[display_index(6, 0)].as_object().unwrap();

    assert!(part0_custom["r"].as_i64().unwrap() > 100 && part0_custom["g"].as_i64().unwrap() > 80);
    assert!(
        part0_zero["r"].as_i64().unwrap() > 0
            && part0_zero["r"].as_i64().unwrap() >= part0_zero["g"].as_i64().unwrap()
    );
    assert!(part1_full["g"].as_i64().unwrap() > part1_full["r"].as_i64().unwrap());
    assert!(all_custom["r"].as_i64().unwrap() > 100 && all_custom["g"].as_i64().unwrap() > 80);
}

#[test]
pub(crate) fn back_exits_active_play_overlay_and_menu_context() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.active_play_mode = "fx".into();
    runner.menu.open_labels(&["Play"]);
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_a", "pressed": true }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(runner.active_play_mode, "none");
    assert!(runner.menu.state.stack.is_empty());
}
