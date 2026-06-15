use super::*;

fn snapshot_from(messages: &[RunnerMessage]) -> Value {
    messages
        .iter()
        .find_map(|message| match message {
            RunnerMessage::Snapshot { snapshot } => Some(snapshot.clone()),
            _ => None,
        })
        .expect("snapshot message")
}

fn confirm_current_dialog(runner: &mut NativeRunner) -> Vec<RunnerMessage> {
    runner.confirm_dialog.as_mut().unwrap().cursor = 1;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap()
}

#[test]
fn unsupported_behavior_errors() {
    let error = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "unsupported".into(),
        ..NativeRunnerConfig::default()
    })
    .err()
    .unwrap();
    assert!(error.contains("unsupported native behavior `unsupported`"));
}

#[test]
fn sequencer_behavior_is_native_and_paintable() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "sequencer".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 2, "y": 3 }),
        })
        .unwrap();

    let model = runner.engine.model().unwrap();
    assert_eq!(runner.behavior.id(), "sequencer");
    assert_eq!(model.name, "sequencer");
    assert_eq!(model.status_line, "Manual");
    assert!(model.cells[platform_core::grid_index(2, 3)]);
}

#[test]
fn keys_behavior_reports_momentary_grid_interaction() {
    let runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "keys".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();

    assert_eq!(runner.snapshot().unwrap()["gridInteraction"], "momentary");
}

#[test]
fn fresh_native_runner_uses_old_initial_sense_defaults() {
    let runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();

    assert_eq!(runner.sense_parts[0].scan_mode, "immediate");
    assert_eq!(runner.sense_parts[0].scan_axis, "columns");
    assert_eq!(runner.sense_parts[0].scan_unit, "1/16");
    assert!(runner.sense_parts[0].event_enabled);
    assert!(!runner.sense_parts[1].event_enabled);
    assert_eq!(runner.sense_parts[0].lowest_note, 36);
    assert_eq!(runner.sense_parts[0].starting_note, 60);
    assert_eq!(runner.sense_parts[0].highest_note, 74);
    assert_eq!(runner.sense_parts[0].scale, "major_pentatonic");
    assert_eq!(runner.sense_parts[0].root, "D");
    assert_eq!(runner.sense_parts[0].out_of_range, "clamp");
    assert_eq!(runner.sense_parts[0].x_pitch_steps, 0);
    assert_eq!(runner.sense_parts[0].y_pitch_steps, 1);
    assert_eq!(runner.ui.master_volume, 73);
    assert_eq!(runner.global_sound.note_length_ms, 120);
    assert!(!runner.auto_save_default);
    assert!(runner.trigger_probability_maps[0]
        .iter()
        .all(|cell| cell == "full"));
}

#[test]
fn behavior_menu_actions_dispatch_selected_action_type() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "ant".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    let action_cursor = runner.menu.root.children[0].children[0]
        .children
        .iter()
        .position(|item| {
            matches!(
                &item.value,
                crate::native_menu::NativeMenuValue::Action(
                    NativeMenuAction::BehaviorAction(action_type)
                ) if action_type == "spawnAnt"
            )
        })
        .expect("spawnAnt action row");
    runner.menu.state.stack = vec![0, 0];
    runner.menu.state.cursor = action_cursor;

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();

    let model = runner.engine.model().unwrap();
    assert_eq!(model.name, "ant");
    assert!(model.cells.iter().any(|cell| *cell));
}

#[test]
fn transport_tick_returns_status_and_snapshot() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.transport = RuntimeTransportState::Playing;
    let messages = runner
        .send(HostMessage::TransportPulseStep {
            pulses: 24,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(true),
        })
        .unwrap();
    assert!(matches!(
        messages.last(),
        Some(RunnerMessage::RuntimeStatus { .. })
    ));
    assert!(messages
        .iter()
        .any(|message| matches!(message, RunnerMessage::Snapshot { .. })));
}

#[test]
fn button_s_toggles_transport() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_s", "pressed": true }),
        })
        .unwrap();
    assert!(matches!(
        messages.last(),
        Some(RunnerMessage::RuntimeStatus { status }) if status.transport == RuntimeTransportState::Playing
    ));
}

#[test]
fn interpreting_behavior_grid_press_and_release_emit_musical_events() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "keys".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();

    let press = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 2, "y": 3 }),
        })
        .unwrap();
    let release = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_release", "x": 2, "y": 3 }),
        })
        .unwrap();

    assert!(musical_note_ons(&press).iter().any(|(_, note)| *note > 0));
    assert!(release.iter().any(|message| matches!(
        message,
        RunnerMessage::MusicalEvents { events }
            if events.iter().any(|event| matches!(event, platform_core::MusicalEvent::NoteOff { .. }))
    )));
}

#[test]
fn input_events_while_paused_false_suppresses_paused_grid_events() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "keys".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.input_events_while_paused = false;

    let paused_press = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 2, "y": 3 }),
        })
        .unwrap();
    runner.transport = RuntimeTransportState::Playing;
    let playing_press = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 3, "y": 3 }),
        })
        .unwrap();

    assert!(musical_note_ons(&paused_press).is_empty());
    assert!(musical_note_ons(&playing_press)
        .iter()
        .any(|(_, note)| *note > 0));
}

#[test]
fn trigger_probability_zero_suppresses_input_transition_events() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "keys".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.sense_parts[0].trigger_probability_mode = "zero".into();
    runner.refresh_active_interpretation_profile();

    let press = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 2, "y": 3 }),
        })
        .unwrap();

    assert!(musical_note_ons(&press).is_empty());
}

#[test]
fn event_enabled_false_suppresses_input_transition_events() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "keys".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.sense_parts[0].event_enabled = false;
    runner.refresh_active_interpretation_profile();
    runner
        .engine
        .set_interpretation_profile(runner.interpretation_profile.clone());

    let press = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 2, "y": 3 }),
        })
        .unwrap();

    assert!(musical_note_ons(&press).is_empty());
}

#[test]
fn trigger_probability_custom_zero_cell_suppresses_transport_events() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "sequencer".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.transport = RuntimeTransportState::Playing;
    runner.algorithm_step_pulses = 24;
    runner.sense_parts[0].scan_mode = "scanning".into();
    runner.sense_parts[0].scan_axis = "rows".into();
    runner.sense_parts[0].scan_unit = "1/4".into();
    runner.sense_parts[0].scanned_action = "note_on".into();
    runner.sense_parts[0].trigger_probability_mode = "custom".into();
    runner.trigger_probability_maps[0][2] = "zero".into();
    runner.refresh_active_mapping_config();
    runner.refresh_active_interpretation_profile();
    runner
        .engine
        .set_interpretation_profile(runner.interpretation_profile.clone());

    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 2, "y": 0 }),
        })
        .unwrap();
    let messages = runner
        .send(HostMessage::TransportPulseStep {
            pulses: 24,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();

    assert!(musical_note_ons(&messages).is_empty());
}

#[test]
fn non_interpreting_sequencer_grid_press_does_not_emit_input_event() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "sequencer".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 2, "y": 3 }),
        })
        .unwrap();

    assert!(!messages
        .iter()
        .any(|message| matches!(message, RunnerMessage::MusicalEvents { .. })));
}

#[test]
fn grid_state_edit_emits_deferred_auto_save_when_enabled() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "sequencer".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.auto_save_default = true;

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 2, "y": 3 }),
        })
        .unwrap();

    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if matches!(
                effects.as_slice(),
                [RuntimePlatformEffect::StoreSaveDefault { mode, .. }]
                    if mode.as_deref() == Some("deferred")
            )
    )));
}

#[test]
fn scan_progress_overlay_is_dim_white_and_preserves_live_cell_color() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "sequencer".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.sense_parts[0].scan_mode = "scanning".into();
    runner.sense_parts[0].scan_axis = "rows".into();
    runner.tick = 0;
    runner.refresh_active_interpretation_profile();
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 0, "y": 0 }),
        })
        .unwrap();
    runner.send(HostMessage::MidiRealtimeStart).unwrap();
    runner.send(HostMessage::MidiRealtimeStart).unwrap();

    let snapshot = runner.snapshot().unwrap();
    let cells = snapshot["leds"]["cells"].as_array().unwrap();
    let scanned_live = cells[display_index(0, 0)].as_object().unwrap();
    let scanned_empty = cells[display_index(1, 0)].as_object().unwrap();

    assert!(scanned_live["r"].as_i64().unwrap() > 0);
    assert!(scanned_live["g"].as_i64().unwrap() > 0);
    assert!(scanned_live["b"].as_i64().unwrap() > 0);
    let empty_r = scanned_empty["r"].as_i64().unwrap();
    let empty_g = scanned_empty["g"].as_i64().unwrap();
    let empty_b = scanned_empty["b"].as_i64().unwrap();
    assert!((empty_r - empty_b).abs() < 20);
    assert!((empty_g - empty_b).abs() < 20);
    assert!(empty_b < 80);
}

#[test]
fn switching_active_part_preserves_current_part_engine_state() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "sequencer".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.part_behavior_ids[1] = "sequencer".into();
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 2, "y": 3 }),
        })
        .unwrap();

    runner.select_active_part(1).unwrap();
    runner.select_active_part(0).unwrap();

    let model = runner.engine.model().unwrap();
    assert!(model.cells[platform_core::grid_index(2, 3)]);
}

#[test]
fn reverse_scan_direction_starts_from_last_lane() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "sequencer".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.sense_parts[0].scan_mode = "scanning".into();
    runner.sense_parts[0].scan_axis = "rows".into();
    runner.sense_parts[0].scan_direction = "reverse".into();
    runner.tick = 0;
    runner.refresh_active_interpretation_profile();

    let snapshot = runner.snapshot().unwrap();
    let cells = snapshot["leds"]["cells"].as_array().unwrap();
    let bottom_row = cells[display_index(0, 0)].as_object().unwrap();
    let top_row = cells[display_index(0, GRID_HEIGHT - 1)]
        .as_object()
        .unwrap();

    assert!(top_row["r"].as_i64().unwrap() > bottom_row["r"].as_i64().unwrap());
}

#[test]
fn scan_sections_limit_overlay_to_current_section_lane() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "sequencer".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.sense_parts[0].scan_mode = "scanning".into();
    runner.sense_parts[0].scan_axis = "rows".into();
    runner.sense_parts[0].scan_sections = 2;
    runner.tick = 0;
    runner.refresh_active_interpretation_profile();

    let snapshot = runner.snapshot().unwrap();
    let cells = snapshot["leds"]["cells"].as_array().unwrap();
    let in_section = cells[display_index(3, 0)].as_object().unwrap();
    let out_of_section = cells[display_index(4, 0)].as_object().unwrap();

    assert!(in_section["r"].as_i64().unwrap() > out_of_section["r"].as_i64().unwrap());
}

#[test]
fn sense_scan_menu_exposes_none_and_scanned_empty_targets() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.sense_parts[0].scan_mode = "scanning".into();
    runner.menu.rebuild(runner.menu_config());
    let scan_group = &runner.menu.root.children[1].children[1].children[0];
    let labels = scan_group
        .children
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();

    assert!(labels.contains(&"Empty Instrument"));
    assert!(labels.contains(&"Empty Action"));
    assert!(runner
        .menu
        .value_for_key("parts.0.l2.mapping.scanned_empty.slot")
        .is_some_and(|value| value == "none"));
}

#[test]
fn synth_preset_load_changes_full_synth_payload_and_filter_resonance_is_editable() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let initial = runner.config_payload()["runtimeConfig"]["instruments"][0]["synth"].clone();

    runner.load_synth_preset(0, "bright_pluck");
    let changed = runner.config_payload()["runtimeConfig"]["instruments"][0]["synth"].clone();

    assert_ne!(changed, initial);
    assert_eq!(changed["filter"]["resonance"], 34);
    assert_eq!(changed["filter"]["type"], "lowpass");
    assert_eq!(changed["filter"]["cutoffHz"], 7200);
    assert_eq!(changed["osc1"]["waveform"], "saw");
    assert_eq!(
        runner
            .menu
            .value_for_key("instruments.0.synth.osc1.waveform"),
        Some("saw".into())
    );
    assert_eq!(
        runner.menu.value_for_key("instruments.0.synth.filter.type"),
        Some("lowpass".into())
    );

    runner.menu.state.stack = vec![2, 0, 0, 2, 1, 0];
    runner.menu.state.cursor = 0;
    runner.menu.state.editing = true;
    runner.menu.turn(1);
    runner.apply_menu_state().unwrap();

    runner.menu.state.cursor = 1;
    runner.menu.turn(1);
    runner.apply_menu_state().unwrap();

    runner.menu.state.cursor = 2;
    runner.menu.turn(5);
    runner.apply_menu_state().unwrap();

    runner.menu.state.cursor = 3;
    runner.menu.turn(10);
    runner.apply_menu_state().unwrap();

    runner.menu.state.stack = vec![2, 0, 0, 2, 2];
    runner.menu.state.cursor = 0;
    runner.menu.state.editing = true;
    runner.menu.turn(1);
    runner.apply_menu_state().unwrap();

    runner.menu.state.stack = vec![2, 0, 0, 2, 2];
    runner.menu.state.cursor = 1;
    runner.menu.state.editing = true;
    runner.menu.turn(-2);
    runner.apply_menu_state().unwrap();

    runner.menu.state.stack = vec![2, 0, 0, 2, 2];
    runner.menu.state.cursor = 2;
    runner.menu.state.editing = true;
    runner.menu.turn(5);
    runner.apply_menu_state().unwrap();

    runner.menu.state.stack = vec![2, 0, 0, 2, 3];
    runner.menu.state.cursor = 1;
    runner.menu.state.editing = true;
    runner.menu.turn(-20);
    runner.apply_menu_state().unwrap();

    runner.menu.state.stack = vec![2, 0, 0, 2, 4];
    runner.menu.state.cursor = 0;
    runner.menu.state.editing = true;
    runner.menu.turn(5);
    runner.apply_menu_state().unwrap();

    assert_eq!(
        runner.config_payload()["runtimeConfig"]["instruments"][0]["synth"]["osc1"]["waveform"],
        "square"
    );
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["instruments"][0]["synth"]["osc1"]["octave"],
        1
    );
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["instruments"][0]["synth"]["osc1"]["levelPct"],
        91
    );
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["instruments"][0]["synth"]["osc1"]["detuneCents"],
        10
    );
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["instruments"][0]["synth"]["filter"]["type"],
        "highpass"
    );
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["instruments"][0]["synth"]["filter"]["cutoffHz"],
        6969
    );
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["instruments"][0]["synth"]["filter"]["resonance"],
        39
    );
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["instruments"][0]["synth"]["amp"]
            ["velocitySensitivityPct"],
        80
    );
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["instruments"][0]["synth"]["ampEnv"]["attackMs"],
        28
    );
}

#[test]
fn sample_slot_menu_is_one_based_but_payload_remains_zero_based_and_back_exits_assign() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].kind = "sampler".into();
    runner.menu.rebuild(runner.menu_config());

    assert_eq!(
        runner
            .menu
            .value_for_key("instruments.0.sample.selectedSlot")
            .unwrap(),
        "1"
    );
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["instruments"][0]["sample"]["selectedSlot"],
        0
    );

    runner.sample_assign = Some((0, 0));
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_a", "pressed": true }),
        })
        .unwrap();
    assert!(runner.sample_assign.is_none());
}

#[test]
fn legacy_bus_route_normalizes_on_config_load_and_round_trip() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["instruments"][0]["mixer"]["route"] = json!("bus_2");

    runner.apply_config_payload(payload).unwrap();

    assert_eq!(runner.instruments[0].route, "fx_bus_2");
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["instruments"][0]["mixer"]["route"],
        "fx_bus_2"
    );
}

#[test]
fn legacy_trigger_gates_migrate_to_probability_map() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["parts"][0]["l1"]["triggerGates"] = Value::Array(
        (0..GRID_WIDTH * GRID_HEIGHT)
            .map(|index| Value::Bool(index != 3))
            .collect(),
    );
    payload["runtimeConfig"]["parts"][0]["l2"]
        .as_object_mut()
        .unwrap()
        .remove("triggerProbabilityMap");

    runner.apply_config_payload(payload).unwrap();

    assert_eq!(runner.trigger_probability_maps[0][3], "zero");
    assert_eq!(runner.trigger_probability_maps[0][4], "full");
}

#[test]
fn legacy_eight_position_pan_payload_scales_to_native_pan_range() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["panPositions"] = json!(8);
    payload["runtimeConfig"]["instruments"][0]["mixer"]["panPos"] = json!(7);
    payload["runtimeConfig"]["instruments"][1]["mixer"]["panPos"] = json!(3);

    runner.apply_config_payload(payload).unwrap();

    assert_eq!(runner.instruments[0].pan_pos, PAN_POSITION_COUNT - 1);
    assert_eq!(runner.instruments[1].pan_pos, PAN_POSITION_COUNT / 2);
}

#[test]
fn sample_slots_and_assignments_are_sanitized_on_load() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["instruments"][0]["sample"]["selectedSlot"] = json!(99);
    payload["runtimeConfig"]["instruments"][0]["sample"]["slots"] = Value::Array(
        (0..12)
            .map(|index| json!({ "path": format!("s{index}.wav") }))
            .collect(),
    );
    payload["runtimeConfig"]["instruments"][0]["sample"]["assignments"] = json!([
        { "x": 99, "y": 99, "sampleSlot": 99, "level": "loud" }
    ]);

    runner.apply_config_payload(payload).unwrap();

    assert_eq!(runner.instruments[0].selected_sample_slot, 7);
    assert_eq!(
        runner.instruments[0].sample_paths[7].as_deref(),
        Some("s7.wav")
    );
    assert_eq!(runner.instruments[0].sample_assignments[0].x, 7);
    assert_eq!(runner.instruments[0].sample_assignments[0].y, 7);
    assert_eq!(runner.instruments[0].sample_assignments[0].sample_slot, 7);
    assert_eq!(runner.instruments[0].sample_assignments[0].level, None);
}

#[test]
fn dance_fx_payload_sanitizes_type_target_and_params() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.apply_touch_fx_payload(&json!({
        "selected": { "fxType": "stutter", "targetKey": "bad", "params": { "rateHz": 99, "depthPct": -5, "ignored": 42 } },
        "assignments": [
            { "x": 1, "y": 2, "config": { "fxType": "pitch_shift", "targetKey": "instrument_8", "params": { "semitones": 99, "cents": -200, "mixPct": 250 } } }
        ]
    }));

    assert_eq!(runner.dance_fx_selected["targetKey"], "master");
    assert_eq!(runner.dance_fx_selected["params"]["rateHz"], 32);
    assert_eq!(runner.dance_fx_selected["params"]["depthPct"], 0);
    assert!(runner.dance_fx_selected["params"].get("ignored").is_none());
    assert_eq!(
        runner.dance_fx_assignments[0].config["params"]["semitones"],
        24
    );
    assert_eq!(
        runner.dance_fx_assignments[0].config["params"]["cents"],
        -100
    );
    assert_eq!(
        runner.dance_fx_assignments[0].config["params"]["mixPct"],
        100
    );
}

#[test]
fn sample_assign_mode_supports_shift_row_and_fn_shift_column() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].kind = "sampler".into();
    runner.sample_assign = Some((0, 2));

    runner.ui.shift_held = true;
    runner.handle_sample_assignment_grid_press(1, 3);
    assert_eq!(runner.instruments[0].sample_assignments.len(), GRID_WIDTH);
    assert!(runner.instruments[0]
        .sample_assignments
        .iter()
        .all(|assignment| assignment.y == 3 && assignment.sample_slot == 2));

    runner.instruments[0].sample_assignments.clear();
    runner.ui.fn_held = true;
    runner.handle_sample_assignment_grid_press(4, 1);
    assert_eq!(runner.instruments[0].sample_assignments.len(), GRID_HEIGHT);
    assert!(runner.instruments[0]
        .sample_assignments
        .iter()
        .all(|assignment| assignment.x == 4 && assignment.sample_slot == 2));
}

#[test]
fn sample_assignment_cycles_velocity_levels_when_enabled() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].kind = "sampler".into();
    runner.instruments[0].sample_velocity_levels_enabled = true;
    runner.sample_assign = Some((0, 2));

    runner.handle_sample_assignment_grid_press(1, 3);
    assert_eq!(
        runner.instruments[0].sample_assignments[0].level.as_deref(),
        Some("high")
    );
    runner.handle_sample_assignment_grid_press(1, 3);
    assert_eq!(
        runner.instruments[0].sample_assignments[0].level.as_deref(),
        Some("medium")
    );
    runner.handle_sample_assignment_grid_press(1, 3);
    assert_eq!(
        runner.instruments[0].sample_assignments[0].level.as_deref(),
        Some("low")
    );
    runner.handle_sample_assignment_grid_press(1, 3);
    assert!(runner.instruments[0].sample_assignments.is_empty());
}

#[test]
fn sample_assignment_velocity_level_uses_configured_values() {
    let mut instrument = NativeInstrumentSlot::new(0);
    instrument.sample_base_velocity = 80;
    instrument.sample_velocity_high = 127;
    instrument.sample_velocity_medium = 64;
    instrument.sample_velocity_low = 32;
    let base = NativeSampleAssignment {
        x: 0,
        y: 0,
        sample_slot: 0,
        level: None,
    };
    let high = NativeSampleAssignment {
        x: 0,
        y: 0,
        sample_slot: 0,
        level: Some("high".into()),
    };
    let medium = NativeSampleAssignment {
        x: 0,
        y: 0,
        sample_slot: 0,
        level: Some("medium".into()),
    };
    let low = NativeSampleAssignment {
        x: 0,
        y: 0,
        sample_slot: 0,
        level: Some("low".into()),
    };

    assert_eq!(sampler_assignment_velocity(127, &base, &instrument), 80);
    assert_eq!(sampler_assignment_velocity(127, &high, &instrument), 127);
    assert_eq!(sampler_assignment_velocity(127, &medium, &instrument), 64);
    assert_eq!(sampler_assignment_velocity(127, &low, &instrument), 32);
}

#[test]
fn sense_velocity_and_filter_lanes_modulate_mapped_events() {
    let sense = NativeSensePart {
        x_velocity: NativeValueLane {
            enabled: true,
            from: 10,
            to: 110,
            grid_offset: 0,
            curve: "linear".into(),
        },
        y_filter_cutoff: NativeValueLane {
            enabled: true,
            from: 20,
            to: 120,
            grid_offset: 0,
            curve: "linear".into(),
        },
        ..NativeSensePart::default()
    };
    let events = vec![MusicalEvent::NoteOn {
        channel: 2,
        note: 60,
        velocity: 100,
        duration_ms: Some(150),
    }];
    let intents = vec![CellTriggerIntent {
        x: 7,
        y: 7,
        degree: 0,
        kind: platform_core::CellTriggerKind::Activate,
    }];

    let out = apply_sampler_assignments_for_instruments(events, &intents, 0, &[], Some(&sense));

    assert!(matches!(
        out.as_slice(),
        [
            MusicalEvent::Cc {
                channel: 2,
                controller: 74,
                value: 120
            },
            MusicalEvent::NoteOn { velocity: 110, .. }
        ]
    ));
}

#[test]
fn midi_instrument_channel_remaps_note_and_cc_events() {
    let mut instrument = NativeInstrumentSlot {
        kind: "midi".into(),
        midi_channel: 10,
        ..NativeInstrumentSlot::new(0)
    };
    instrument.midi_enabled = true;
    let sense = NativeSensePart {
        y_filter_cutoff: NativeValueLane {
            enabled: true,
            from: 20,
            to: 120,
            grid_offset: 0,
            curve: "linear".into(),
        },
        ..NativeSensePart::default()
    };
    let intent = CellTriggerIntent {
        x: 0,
        y: 7,
        degree: 0,
        kind: platform_core::CellTriggerKind::Activate,
    };

    let out = apply_sampler_assignments_for_instruments(
        vec![MusicalEvent::NoteOn {
            channel: 0,
            note: 60,
            velocity: 100,
            duration_ms: Some(120),
        }],
        &[intent],
        0,
        &[instrument],
        Some(&sense),
    );

    assert!(matches!(
        out.as_slice(),
        [
            MusicalEvent::Cc { channel: 9, .. },
            MusicalEvent::NoteOn { channel: 9, .. }
        ]
    ));
}

#[test]
fn sampler_assignment_remaps_note_off_and_suppresses_unmapped_notes() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].kind = "sampler".into();
    runner.instruments[0].sample_assignments = vec![NativeSampleAssignment {
        x: 2,
        y: 3,
        sample_slot: 4,
        level: None,
    }];
    let instruments = vec![runner.instruments[0].clone()];
    let assigned_intent = CellTriggerIntent {
        x: 2,
        y: 3,
        degree: 0,
        kind: platform_core::CellTriggerKind::Deactivate,
    };
    let unmapped_intent = CellTriggerIntent {
        x: 1,
        y: 1,
        degree: 0,
        kind: platform_core::CellTriggerKind::Activate,
    };

    let assigned = apply_sampler_assignments_for_instruments(
        vec![MusicalEvent::NoteOff {
            channel: 0,
            note: 60,
        }],
        &[assigned_intent],
        0,
        &instruments,
        None,
    );
    let unmapped = apply_sampler_assignments_for_instruments(
        vec![
            MusicalEvent::NoteOn {
                channel: 0,
                note: 60,
                velocity: 100,
                duration_ms: Some(120),
            },
            MusicalEvent::NoteOff {
                channel: 0,
                note: 60,
            },
        ],
        &[unmapped_intent.clone(), unmapped_intent],
        0,
        &instruments,
        None,
    );

    assert!(matches!(
        assigned.as_slice(),
        [MusicalEvent::NoteOff {
            channel: 0,
            note: 40
        }]
    ));
    assert!(unmapped.is_empty());
}

#[test]
fn sense_value_lanes_round_trip_in_payload() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["parts"][0]["l2"]["x"]["velocity"] = json!({
        "enabled": true,
        "from": 12,
        "to": 99,
        "gridOffset": 2,
        "curve": "curve"
    });
    payload["runtimeConfig"]["parts"][0]["l2"]["y"]["filterResonance"] = json!({
        "enabled": true,
        "from": 3,
        "to": 77,
        "gridOffset": -1,
        "curve": "linear"
    });

    runner.apply_config_payload(payload).unwrap();

    assert!(runner.sense_parts[0].x_velocity.enabled);
    assert_eq!(runner.sense_parts[0].x_velocity.from, 12);
    assert_eq!(runner.sense_parts[0].x_velocity.to, 99);
    assert_eq!(runner.sense_parts[0].x_velocity.grid_offset, 2);
    assert_eq!(runner.sense_parts[0].x_velocity.curve, "curve");
    assert!(runner.sense_parts[0].y_filter_resonance.enabled);
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["parts"][0]["l2"]["x"]["velocity"]["to"],
        99
    );
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["parts"][0]["l2"]["x"]["velocity"]["curve"],
        "curve"
    );

    runner.menu.rebuild(runner.menu_config());
    runner.menu.turn_key("parts.0.l2.x.velocity.curve", -1);
    runner.apply_menu_state().unwrap();
    assert_eq!(runner.sense_parts[0].x_velocity.curve, "linear");
}

#[test]
fn assignment_mode_wins_over_fn_part_navigation_and_autosaves() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.auto_save_default = true;
    runner.instruments[0].kind = "sampler".into();
    runner.sample_assign = Some((0, 1));
    runner.ui.fn_held = true;

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 0, "y": 3 }),
        })
        .unwrap();

    assert_eq!(runner.active_part_index, 0);
    assert!(runner.instruments[0]
        .sample_assignments
        .iter()
        .any(|assignment| assignment.x == 0 && assignment.y == 3 && assignment.sample_slot == 1));
    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if matches!(
                effects.as_slice(),
                [RuntimePlatformEffect::StoreSaveDefault { mode, .. }]
                    if mode.as_deref() == Some("deferred")
            )
    )));
}

#[test]
fn dance_mix_grid_edit_autosaves_persistent_volume_change() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.auto_save_default = true;
    runner.active_dance_mode = "mix".into();

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 0, "y": 1 }),
        })
        .unwrap();

    assert_eq!(runner.instruments[0].volume, 14);
    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if matches!(
                effects.as_slice(),
                [RuntimePlatformEffect::StoreSaveDefault { mode, .. }]
                    if mode.as_deref() == Some("deferred")
            )
    )));
}

#[test]
fn dance_xy_touch_persists_and_release_behavior_matches_config() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.active_dance_mode = "xy".into();
    runner.xy_invert_x = true;
    runner.xy_release = "reset-center".into();

    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 0, "y": 7 }),
        })
        .unwrap();
    assert_eq!(runner.xy_touch.x, 1.0);
    assert_eq!(runner.xy_touch.y, 1.0);
    assert!(runner.xy_touch.active);

    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_release", "x": 0, "y": 7 }),
        })
        .unwrap();
    assert_eq!(runner.xy_touch.x, 0.5);
    assert_eq!(runner.xy_touch.y, 0.5);
    assert!(!runner.xy_touch.active);

    let payload = runner.config_payload();
    assert_eq!(payload["runtimeConfig"]["xyRelease"], "reset-center");
    assert_eq!(payload["runtimeConfig"]["parts"][0]["xy"]["xInvert"], true);

    let mut restored = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    restored.apply_config_payload(payload).unwrap();
    assert_eq!(restored.xy_release, "reset-center");
    assert!(restored.xy_invert_x);
}

#[test]
fn param_mod_binding_updates_native_runtime_config() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.param_mods[0].x[0] = Some(NativeParamBinding {
        key: "instruments.0.mixer.volume".into(),
        label: Some("Volume".into()),
        kind: "number".into(),
        min: Some(0.0),
        max: Some(100.0),
        step: Some(1.0),
        options: vec![],
        invert: false,
    });
    let intents = vec![CellTriggerIntent {
        x: 7,
        y: 0,
        degree: 0,
        kind: platform_core::CellTriggerKind::Activate,
    }];

    runner.apply_runtime_modulation(&intents, 0);

    assert_eq!(runner.instruments[0].volume, 100);
    assert!(runner.config_dirty);
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["parts"][0]["paramMods"]["x"][0]["key"],
        "instruments.0.mixer.volume"
    );
}

#[test]
fn shift_grid_param_mod_mapping_cycles_slots() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let binding = NativeParamBinding {
        key: "instruments.0.mixer.volume".into(),
        label: Some("Volume".into()),
        kind: "number".into(),
        min: Some(0.0),
        max: Some(100.0),
        step: Some(1.0),
        options: vec![],
        invert: false,
    };

    assert!(runner.apply_param_mod_mapping(0, 0, binding.clone()));
    assert_eq!(runner.param_mods[0].x[0].as_ref().unwrap().key, binding.key);
    assert_eq!(runner.param_mods[0].y[0].as_ref().unwrap().key, binding.key);
    assert!(!runner.param_mods[0].x[0].as_ref().unwrap().invert);
    assert!(runner.config_dirty);

    assert!(runner.apply_param_mod_mapping(0, 0, binding.clone()));
    assert!(runner.param_mods[0].x[0].as_ref().unwrap().invert);
    assert!(runner.param_mods[0].y[0].as_ref().unwrap().invert);

    assert!(runner.apply_param_mod_mapping(0, 0, binding.clone()));
    assert!(runner.param_mods[0].x[0].is_none());
    assert!(runner.param_mods[0].y[0].is_none());

    assert!(runner.apply_param_mod_mapping(2, 1, binding.clone()));
    assert_eq!(runner.param_mods[0].x[1].as_ref().unwrap().key, binding.key);
    assert!(runner.param_mods[0].y[1].is_none());

    assert!(runner.apply_param_mod_mapping(1, 4, binding.clone()));
    assert_eq!(runner.param_mods[0].y[1].as_ref().unwrap().key, binding.key);

    assert!(runner.apply_param_mod_mapping(1, 1, binding));
    assert!(runner.param_mods[0].x[1].as_ref().unwrap().invert);
    assert!(runner.param_mods[0].y[1].as_ref().unwrap().invert);
}

#[test]
fn shift_grid_param_mod_overlay_marks_lanes_and_combined_cells() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let binding = NativeParamBinding {
        key: "instruments.0.mixer.volume".into(),
        label: Some("Volume".into()),
        kind: "number".into(),
        min: Some(0.0),
        max: Some(100.0),
        step: Some(1.0),
        options: vec![],
        invert: false,
    };
    let mut inverted = binding.clone();
    inverted.invert = true;
    runner.param_mods[0].x[0] = Some(binding);
    runner.param_mods[0].y[1] = Some(inverted);

    runner.menu.turn(2);
    runner.menu.press();
    runner.menu.press();
    runner.menu.press();
    runner.menu.turn(3);
    runner.menu.press();
    runner.menu.turn(1);
    runner.ui.shift_held = true;

    let snapshot = runner.snapshot().unwrap();
    let cells = snapshot["leds"]["cells"].as_array().unwrap();

    assert_eq!(
        cells[display_index(3, 0)],
        json!({ "r": 0, "g": 255, "b": 120 })
    );
    assert_eq!(
        cells[display_index(1, 3)],
        json!({ "r": 255, "g": 0, "b": 90 })
    );
    assert_eq!(
        cells[display_index(3, 1)],
        json!({ "r": 18, "g": 18, "b": 24 })
    );
    assert_eq!(
        cells[display_index(0, 0)],
        json!({ "r": 255, "g": 255, "b": 255 })
    );
    assert_eq!(
        cells[display_index(1, 1)],
        json!({ "r": 255, "g": 255, "b": 255 })
    );
}

#[test]
fn menu_binding_actions_update_param_xy_and_aux_targets() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let binding = NativeParamBindingSpec {
        key: "instruments.0.mixer.volume".into(),
        label: Some("Volume".into()),
        kind: "number".into(),
        min: Some(0),
        max: Some(100),
        step: Some(1),
        options: vec![],
        invert: false,
    };

    runner
        .execute_menu_action(NativeMenuAction::SetParamBinding {
            target: "param:0:x:0".into(),
            binding: binding.clone(),
        })
        .unwrap();
    runner
        .execute_menu_action(NativeMenuAction::SetParamBinding {
            target: "xy:x".into(),
            binding: binding.clone(),
        })
        .unwrap();
    runner
        .execute_menu_action(NativeMenuAction::SetParamBinding {
            target: "aux:0:turn".into(),
            binding,
        })
        .unwrap();

    assert_eq!(
        runner.param_mods[0].x[0].as_ref().unwrap().key,
        "instruments.0.mixer.volume"
    );
    assert_eq!(
        runner.xy_x_binding.as_ref().unwrap().key,
        "instruments.0.mixer.volume"
    );
    assert_eq!(
        runner.aux_bindings[0]
            .as_ref()
            .and_then(|binding| binding.turn_key.as_deref()),
        Some("instruments.0.mixer.volume")
    );

    runner
        .execute_menu_action(NativeMenuAction::SetAuxClick {
            index: 0,
            action: Some(Box::new(NativeMenuAction::PlatformEffect(
                "sample.assign:0:0".into(),
            ))),
        })
        .unwrap();
    assert!(matches!(
        runner.aux_bindings[0]
            .as_ref()
            .and_then(|binding| binding.press_action.as_ref()),
        Some(NativeMenuAction::PlatformEffect(action)) if action == "sample.assign:0:0"
    ));
}

#[test]
fn behavior_change_remaps_behavior_param_mods_and_aux_bindings() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.param_mods[0].x[0] = Some(NativeParamBinding {
        key: "parts.0.l1.behaviorConfig.randomTickInterval".into(),
        label: Some("Spawn Interval".into()),
        kind: "number".into(),
        min: Some(1.0),
        max: Some(20.0),
        step: Some(1.0),
        options: vec![],
        invert: false,
    });
    runner.param_mods[0].y[0] = Some(NativeParamBinding {
        key: "parts.0.l1.behaviorConfig.randomCellsPerTick".into(),
        label: Some("Spawn Count".into()),
        kind: "number".into(),
        min: Some(0.0),
        max: Some(20.0),
        step: Some(1.0),
        options: vec![],
        invert: true,
    });
    runner.aux_bindings[0] = Some(NativeAuxBinding {
        turn_key: Some("behaviorConfig.life.randomTickInterval".into()),
        press_action: Some(NativeMenuAction::BehaviorAction("spawnRandom".into())),
    });

    runner.remap_bindings_for_behavior_change("life", "brain", 0);

    let x_binding = runner.param_mods[0].x[0].as_ref().unwrap();
    assert_eq!(x_binding.key, "parts.0.l1.behaviorConfig.seedInterval");
    assert_eq!(x_binding.label.as_deref(), Some("Seed Interval"));
    assert!(!x_binding.invert);

    let y_binding = runner.param_mods[0].y[0].as_ref().unwrap();
    assert_eq!(y_binding.key, "parts.0.l1.behaviorConfig.randomSeedCells");
    assert_eq!(y_binding.label.as_deref(), Some("Spawn Count"));
    assert!(y_binding.invert);

    let aux = runner.aux_bindings[0].as_ref().unwrap();
    assert_eq!(
        aux.turn_key.as_deref(),
        Some("behaviorConfig.brain.seedInterval")
    );
    assert!(matches!(
        aux.press_action.as_ref(),
        Some(NativeMenuAction::BehaviorAction(action)) if action == "seedRandom"
    ));
}

#[test]
fn dance_xy_binding_updates_native_runtime_config() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.xy_touch = NativeXyTouch {
        x: 1.0,
        y: 0.5,
        active: true,
    };
    runner.xy_x_binding = Some(NativeParamBinding {
        key: "sound.velocityScalePct".into(),
        label: Some("Velocity Scale".into()),
        kind: "number".into(),
        min: Some(0.0),
        max: Some(200.0),
        step: Some(1.0),
        options: vec![],
        invert: true,
    });

    runner.apply_runtime_modulation(&[], 0);

    assert_eq!(runner.global_sound.velocity_scale_pct, 200);
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["parts"][0]["xy"]["x"]["key"],
        "sound.velocityScalePct"
    );
}

#[test]
fn invalid_aux_and_xy_bindings_are_dropped_on_load() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["auxBindings"] = json!({
        "aux1": { "turnKey": "../../bad", "pressAction": null },
        "aux2": { "turnKey": "sound.noteLengthMs", "pressAction": null }
    });
    payload["runtimeConfig"]["parts"][0]["xy"]["x"] = json!({
        "key": "unknown.path",
        "kind": "number"
    });
    payload["runtimeConfig"]["parts"][0]["xy"]["y"] = json!({
        "key": "instruments.0.mixer.volume",
        "kind": "number",
        "min": 0,
        "max": 100,
        "step": 1
    });

    runner.apply_config_payload(payload).unwrap();

    assert!(runner.aux_bindings[0].is_none());
    assert_eq!(
        runner.aux_bindings[1].as_ref().unwrap().turn_key.as_deref(),
        Some("sound.noteLengthMs")
    );
    assert!(runner.xy_x_binding.is_none());
    assert_eq!(
        runner.xy_y_binding.as_ref().unwrap().key,
        "instruments.0.mixer.volume"
    );
}

#[test]
fn config_payload_includes_complete_sample_and_fx_param_shapes() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut payload = runner.config_payload();

    assert_eq!(
        payload["runtimeConfig"]["instruments"][0]["sample"]["baseVelocity"],
        100
    );
    assert_eq!(
        payload["runtimeConfig"]["instruments"][0]["midiEngine"]["velocity"],
        100
    );
    assert_eq!(
        payload["runtimeConfig"]["instruments"][0]["midiEngine"]["channel"],
        1
    );
    assert!(payload["runtimeConfig"]["instruments"][0]["sample"]["ampEnv"].is_object());
    assert!(payload["runtimeConfig"]["instruments"][0]["sample"]["filter"].is_object());
    assert!(payload["runtimeConfig"]["instruments"][0]["sample"]["filterEnv"].is_object());
    assert!(payload["runtimeConfig"]["mixer"]["buses"][0]["slot1"]["params"].is_object());
    assert!(payload["runtimeConfig"]["mixer"]["master"]["slots"][0]["params"].is_object());

    payload["runtimeConfig"]["instruments"][0]["sample"]["baseVelocity"] = json!(72);
    payload["runtimeConfig"]["instruments"][0]["sample"]["ampEnv"] = json!({ "attackMs": 11 });
    payload["runtimeConfig"]["instruments"][0]["sample"]["filter"] =
        json!({ "type": "highpass", "cutoffHz": 1200 });
    payload["runtimeConfig"]["instruments"][0]["sample"]["filterEnv"] = json!({ "releaseMs": 222 });
    payload["runtimeConfig"]["instruments"][0]["midiEngine"] = json!({
        "channel": 7,
        "velocity": 66,
        "durationMs": 444
    });
    payload["runtimeConfig"]["mixer"]["buses"][0]["slot1"] =
        json!({ "type": "delay", "params": { "timeMs": 333, "feedback": 0.42, "mixPct": 44 } });
    payload["runtimeConfig"]["mixer"]["master"]["slots"][0] =
        json!({ "type": "distortion", "params": { "drive": 3.5, "clip": 0.75, "mixPct": 88 } });
    runner.apply_config_payload(payload).unwrap();
    assert_eq!(runner.instruments[0].sample_base_velocity, 72);
    assert_eq!(runner.instruments[0].sample_amp_env["attackMs"], 11);
    assert_eq!(runner.instruments[0].sample_filter["type"], "highpass");
    assert_eq!(runner.instruments[0].sample_filter_env["releaseMs"], 222);
    assert_eq!(runner.instruments[0].midi_velocity, 66);
    assert_eq!(runner.instruments[0].midi_channel, 7);
    assert_eq!(runner.instruments[0].midi_duration_ms, 444);
    assert_eq!(runner.fx_buses[0].slot1_params["feedback"], 0.42);
    assert_eq!(runner.global_fx_params[0]["drive"], 3.5);
    let round_trip = runner.config_payload();
    assert_eq!(
        round_trip["runtimeConfig"]["mixer"]["buses"][0]["slot1"]["params"]["timeMs"],
        333
    );
    assert_eq!(
        round_trip["runtimeConfig"]["mixer"]["master"]["slots"][0]["params"]["clip"],
        0.75
    );
}

#[test]
fn snapshot_settings_include_complete_audio_config_shapes() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].sample_base_velocity = 72;
    runner.instruments[0].sample_amp_env = json!({ "attackMs": 11 });
    runner.instruments[0].sample_filter = json!({ "type": "highpass", "cutoffHz": 1200 });
    runner.instruments[0].sample_filter_env = json!({ "releaseMs": 222 });
    runner.fx_buses[0].slot1_type = "delay".into();
    runner.fx_buses[0].slot1_params = json!({ "timeMs": 333, "feedback": 0.42, "mixPct": 44 });
    runner.global_fx_slots[0] = "distortion".into();
    runner.global_fx_params[0] = json!({ "drive": 3.5, "clip": 0.75, "mixPct": 88 });

    let snapshot = runner.snapshot().unwrap();

    assert_eq!(
        snapshot["settings"]["instruments"][0]["sample"]["baseVelocity"],
        72
    );
    assert_eq!(
        snapshot["settings"]["instruments"][0]["sample"]["ampEnv"]["attackMs"],
        11
    );
    assert_eq!(
        snapshot["settings"]["instruments"][0]["sample"]["filter"]["type"],
        "highpass"
    );
    assert_eq!(
        snapshot["settings"]["instruments"][0]["sample"]["filterEnv"]["releaseMs"],
        222
    );
    assert_eq!(
        snapshot["settings"]["mixer"]["buses"][0]["slot1"]["params"]["feedback"],
        0.42
    );
    assert_eq!(
        snapshot["settings"]["mixer"]["master"]["slots"][0]["params"]["clip"],
        0.75
    );
}

#[test]
fn unbound_aux_inputs_show_toast_without_navigating_menu() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let original_stack = runner.menu.state.stack.clone();
    let original_cursor = runner.menu.state.cursor;

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "aux1" }),
        })
        .unwrap();
    let snapshot = runner.snapshot().unwrap();

    assert_eq!(runner.menu.state.stack, original_stack);
    assert_eq!(runner.menu.state.cursor, original_cursor);
    assert!(snapshot["display"]["toast"]
        .as_str()
        .unwrap()
        .contains("Aux 1"));
}

#[test]
fn scanning_sequencer_pattern_emits_different_rows_over_scan_steps() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "sequencer".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.transport = RuntimeTransportState::Playing;
    runner.algorithm_step_pulses = 24;
    runner.sense_parts[0].scan_mode = "scanning".into();
    runner.sense_parts[0].scan_axis = "rows".into();
    runner.sense_parts[0].scan_unit = "1/4".into();
    runner.sense_parts[0].scanned_slot = 1;
    runner.refresh_active_mapping_config();
    runner.refresh_active_interpretation_profile();
    runner
        .engine
        .set_interpretation_profile(runner.interpretation_profile.clone());
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 0, "y": 0 }),
        })
        .unwrap();
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 0, "y": 1 }),
        })
        .unwrap();

    let first = runner
        .send(HostMessage::TransportPulseStep {
            pulses: 24,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();
    let second = runner
        .send(HostMessage::TransportPulseStep {
            pulses: 24,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();

    let first_notes = musical_note_ons(&first);
    let second_notes = musical_note_ons(&second);
    assert!(first_notes.iter().any(|(channel, _)| *channel == 1));
    assert!(second_notes.iter().any(|(channel, _)| *channel == 1));
    assert_ne!(first_notes, second_notes);
}

#[test]
fn transport_tick_advances_multiple_configured_parts() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "sequencer".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.transport = RuntimeTransportState::Playing;
    runner.algorithm_step_pulses = 24;
    runner.part_behavior_ids[1] = "sequencer".into();
    runner.sense_parts[0].scan_mode = "scanning".into();
    runner.sense_parts[0].scanned_slot = 0;
    runner.sense_parts[1].scan_mode = "scanning".into();
    runner.sense_parts[1].scanned_slot = 1;
    runner.refresh_active_mapping_config();
    runner.refresh_active_interpretation_profile();
    runner
        .engine
        .set_interpretation_profile(runner.interpretation_profile.clone());
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 0, "y": 0 }),
        })
        .unwrap();
    runner.select_active_part(1).unwrap();
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 0, "y": 0 }),
        })
        .unwrap();

    let messages = runner
        .send(HostMessage::TransportPulseStep {
            pulses: 24,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();
    let notes = musical_note_ons(&messages);

    assert!(notes.iter().any(|(channel, _)| *channel == 0));
    assert!(notes.iter().any(|(channel, _)| *channel == 1));
}

#[test]
fn inactive_part_transport_tick_applies_param_modulation() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "sequencer".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.transport = RuntimeTransportState::Playing;
    runner.part_behavior_ids[1] = "sequencer".into();
    runner.sense_parts[0].scan_mode = "scanning".into();
    runner.sense_parts[0].scan_axis = "rows".into();
    runner.sense_parts[0].scanned_slot = 0;
    runner.param_mods[0].x[0] = Some(NativeParamBinding {
        key: "instruments.0.mixer.volume".into(),
        label: Some("Volume".into()),
        kind: "number".into(),
        min: Some(0.0),
        max: Some(100.0),
        step: Some(1.0),
        options: vec![],
        invert: true,
    });
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 0, "y": 0 }),
        })
        .unwrap();
    runner.instruments[0].volume = 0;
    runner.select_active_part(1).unwrap();

    runner
        .send(HostMessage::TransportPulseStep {
            pulses: 24,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();

    assert_eq!(runner.active_part_index, 1);
    assert_eq!(runner.instruments[0].volume, 100);
}

#[test]
fn native_menu_edit_emits_deferred_auto_save_when_enabled() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.auto_save_default = true;
    runner.menu.state.stack = vec![3];
    runner.menu.state.cursor = 1;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    runner.transport = RuntimeTransportState::Playing;
    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
        })
        .unwrap();
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();

    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if matches!(
                effects.as_slice(),
                [RuntimePlatformEffect::StoreSaveDefault { mode, .. }]
                    if mode.as_deref() == Some("deferred")
            )
    )));
}

#[test]
fn native_snapshot_reserves_bottom_oled_row_for_status() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.state.stack = vec![2, 0, 0, 2, 1];
    runner.menu.state.cursor = 0;
    let snapshot = runner.snapshot().unwrap();

    assert!(snapshot["display"]["lines"].as_array().unwrap().len() <= 6);
}

#[test]
fn regular_menu_snapshot_keeps_seven_body_rows_above_reserved_status() {
    let runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let snapshot = runner.snapshot().unwrap();

    assert_eq!(snapshot["display"]["lines"].as_array().unwrap().len(), 6);
}

#[test]
fn scan_unit_advances_scanning_before_full_note_step_rate() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "sequencer".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.transport = RuntimeTransportState::Playing;
    runner.algorithm_step_pulses = 96;
    runner.sense_parts[0].scan_mode = "scanning".into();
    runner.sense_parts[0].scan_axis = "rows".into();
    runner.sense_parts[0].scan_unit = "1/4".into();
    runner.refresh_active_mapping_config();
    runner.refresh_active_interpretation_profile();
    runner
        .engine
        .set_interpretation_profile(runner.interpretation_profile.clone());
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 0, "y": 0 }),
        })
        .unwrap();
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 0, "y": 1 }),
        })
        .unwrap();

    let first = runner
        .send(HostMessage::TransportPulseStep {
            pulses: 24,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();
    let second = runner
        .send(HostMessage::TransportPulseStep {
            pulses: 24,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();

    assert_ne!(musical_note_ons(&first), musical_note_ons(&second));
}

#[test]
fn sequencer_grid_state_is_serialized_and_rehydrated_for_all_parts() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "sequencer".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 2, "y": 3 }),
        })
        .unwrap();
    runner.select_active_part(1).unwrap();
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 4, "y": 5 }),
        })
        .unwrap();
    let payload = runner.config_payload();

    let mut loaded = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    loaded.apply_config_payload(payload).unwrap();
    loaded.select_active_part(0).unwrap();
    assert!(loaded.engine.model().unwrap().cells[platform_core::grid_index(2, 3)]);
    loaded.select_active_part(1).unwrap();
    assert!(loaded.engine.model().unwrap().cells[platform_core::grid_index(4, 5)]);
}

#[test]
fn save_grid_state_controls_saved_state_payload_and_restore() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "sequencer".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 2, "y": 3 }),
        })
        .unwrap();
    let mut payload = runner.config_payload();

    assert_eq!(
        payload["runtimeConfig"]["parts"][0]["l1"]["saveGridState"],
        true
    );
    assert!(!payload["runtimeConfig"]["parts"][0]["l1"]["savedState"].is_null());
    assert!(payload["runtimeConfig"]["parts"][0]["l1"]["behaviorState"].is_null());

    let mut legacy_payload = payload.clone();
    let saved_state = legacy_payload["runtimeConfig"]["parts"][0]["l1"]["savedState"].clone();
    legacy_payload["runtimeConfig"]["parts"][0]["l1"]["savedState"] = Value::Null;
    legacy_payload["runtimeConfig"]["parts"][0]["l1"]["behaviorState"] = saved_state;
    let mut legacy_loaded = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    legacy_loaded.apply_config_payload(legacy_payload).unwrap();
    assert!(legacy_loaded.engine.model().unwrap().cells[platform_core::grid_index(2, 3)]);

    payload["runtimeConfig"]["parts"][0]["l1"]["saveGridState"] = json!(false);
    let mut loaded = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    loaded.apply_config_payload(payload).unwrap();

    assert!(!loaded.engine.model().unwrap().cells[platform_core::grid_index(2, 3)]);
    assert_eq!(
        loaded.config_payload()["runtimeConfig"]["parts"][0]["l1"]["saveGridState"],
        false
    );
    assert!(loaded.config_payload()["runtimeConfig"]["parts"][0]["l1"]["savedState"].is_null());
}

#[test]
fn save_default_result_lights_auto_save_indicator_and_toast_scrolls() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::SaveDefaultResult {
                ok: true,
                is_auto: Some(true),
            },
        })
        .unwrap();
    let snapshot = runner.snapshot().unwrap();
    assert_eq!(snapshot["settings"]["autoSaveFlash"], "flash");
    assert!(snapshot["display"]["toast"]
        .as_str()
        .unwrap()
        .contains("Saved"));

    runner.set_toast_for_test(
        "This is a very long toaster message that must scroll across the reserved status row",
    );
    let first = runner.snapshot().unwrap()["display"]["toast"].clone();
    runner.advance_toast_for_test();
    let second = runner.snapshot().unwrap()["display"]["toast"].clone();
    assert_ne!(first, second);
}

fn musical_note_ons(messages: &[RunnerMessage]) -> Vec<(u8, u8)> {
    messages
        .iter()
        .flat_map(|message| match message {
            RunnerMessage::MusicalEvents { events } => events.as_slice(),
            _ => &[],
        })
        .filter_map(|event| match event {
            platform_core::MusicalEvent::NoteOn { channel, note, .. } => Some((*channel, *note)),
            _ => None,
        })
        .collect()
}

#[test]
fn changing_behavior_keeps_menu_location() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();

    let level1 = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    assert_eq!(snapshot_from(&level1)["display"]["title"], "L1: Life");

    let part = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    assert_eq!(
        snapshot_from(&part)["display"]["title"],
        "L1: Life/P1: life"
    );

    let edit_behavior = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    assert_eq!(
        snapshot_from(&edit_behavior)["display"]["title"],
        "L1: Life/P1: life"
    );
    assert_eq!(snapshot_from(&edit_behavior)["display"]["editing"], true);

    let changed = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 2, "id": "main" }),
        })
        .unwrap();
    let snapshot = snapshot_from(&changed);
    assert_eq!(snapshot["display"]["title"], "L1: Life/P1: keys");
    assert_eq!(snapshot["display"]["editing"], true);
    assert_eq!(snapshot["activeBehavior"], "keys");
}

#[test]
fn behavior_config_number_param_edit_via_menu() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "life".into(),
        behavior_config: json!({ "randomCellsPerTick": 12, "randomTickInterval": 1 }),
        ..NativeRunnerConfig::default()
    })
    .unwrap();

    runner.menu.state.stack = vec![0, 0];
    runner.menu.state.cursor = 4;
    runner.menu.state.editing = true;
    let _ = runner.send(HostMessage::DeviceInput {
        input: json!({ "type": "encoder_turn", "delta": -1, "id": "main" }),
    });
    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();

    assert_eq!(runner.behavior_config["randomCellsPerTick"], 11);
    let snapshot = snapshot_from(&messages);
    assert_eq!(snapshot["display"]["editing"], false);
    assert_eq!(snapshot["display"]["title"], "L1: Life/P1: life");
}

#[test]
fn behavior_config_second_number_param_edit_via_menu() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "life".into(),
        behavior_config: json!({ "randomCellsPerTick": 0, "randomTickInterval": 1 }),
        ..NativeRunnerConfig::default()
    })
    .unwrap();

    runner.menu.state.stack = vec![0, 0];
    runner.menu.state.cursor = 5;
    runner.menu.state.editing = true;
    let _ = runner.send(HostMessage::DeviceInput {
        input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
    });

    assert_eq!(runner.behavior_config["randomTickInterval"], 2);
}

#[test]
fn behavior_config_enum_param_edits_via_menu() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "keys".into(),
        behavior_config: json!({ "quantize": "immediate" }),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.menu.state.stack = vec![0, 0];
    runner.menu.state.cursor = 4;
    runner.menu.state.editing = true;

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
        })
        .unwrap();

    assert_eq!(runner.behavior_config["quantize"], "step");
}

#[test]
fn bool_menu_items_edit_like_two_option_enums() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();

    for _ in 0..5 {
        let _ = runner.send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 2, "id": "main" }),
        });
    }
    let _ = runner.send(HostMessage::DeviceInput {
        input: json!({ "type": "encoder_press", "id": "main" }),
    });
    let _ = runner.send(HostMessage::DeviceInput {
        input: json!({ "type": "encoder_turn", "delta": 2, "id": "main" }),
    });
    let _ = runner.send(HostMessage::DeviceInput {
        input: json!({ "type": "encoder_press", "id": "main" }),
    });

    assert!(!runner.midi_enabled);

    let enter = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    assert_eq!(snapshot_from(&enter)["display"]["editing"], true);
    assert!(!runner.midi_enabled);

    let _ = runner.send(HostMessage::DeviceInput {
        input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
    });
    assert!(runner.midi_enabled);

    let _ = runner.send(HostMessage::DeviceInput {
        input: json!({ "type": "encoder_turn", "delta": -1, "id": "main" }),
    });
    assert!(!runner.midi_enabled);

    let exit = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    assert_eq!(snapshot_from(&exit)["display"]["editing"], false);
}

#[test]
fn system_sound_master_volume_edit_via_menu() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();

    for _ in 0..5 {
        let _ = runner.send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 2, "id": "main" }),
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

    let edit = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    assert_eq!(snapshot_from(&edit)["display"]["editing"], true);

    let _ = runner.send(HostMessage::DeviceInput {
        input: json!({ "type": "encoder_turn", "delta": 26, "id": "main" }),
    });
    assert_eq!(runner.ui.master_volume, 99);

    let exit = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    assert_eq!(snapshot_from(&exit)["display"]["editing"], false);
    assert_eq!(snapshot_from(&exit)["display"]["title"], "SYS/Sound");
}

#[test]
fn screen_sleep_splashes_then_turns_oled_off_and_input_wakes() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.ui.screen_sleep_seconds = 1;
    runner.last_interaction_at = Instant::now() - Duration::from_secs(2);

    let messages = runner.messages_with_snapshot().unwrap();
    let display = &snapshot_from(&messages)["display"];
    assert_eq!(display["splash"], "Going to sleep");
    assert_eq!(display["off"], false);

    runner.oled_splash_until = Some(Instant::now() - Duration::from_millis(1));
    let messages = runner.messages_with_snapshot().unwrap();
    let display = &snapshot_from(&messages)["display"];
    assert_eq!(display["off"], true);

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
        })
        .unwrap();
    let display = &snapshot_from(&messages)["display"];
    assert_eq!(display["off"], false);
    assert_eq!(display["splash"], "");
}

#[test]
fn fn_aux_binds_selected_param_and_aux_turn_edits_it() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.state.stack = vec![5, 1];
    runner.menu.state.cursor = 0;

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_fn", "pressed": true }),
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "aux1" }),
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_fn", "pressed": false }),
        })
        .unwrap();
    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "id": "aux1", "delta": -10 }),
        })
        .unwrap();

    assert_eq!(snapshot_from(&messages)["settings"]["masterVolume"], 63);
    assert!(snapshot_from(&messages)["display"]["toast"]
        .as_str()
        .unwrap_or("")
        .contains("Aux 1"));
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["auxBindings"]["aux1"]["turnKey"],
        "masterVolume"
    );
}

#[test]
fn fn_aux_binds_selected_action_and_aux_press_executes_it() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.state.stack = vec![5, 2];
    runner.menu.state.cursor = 1;

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_fn", "pressed": true }),
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "aux1" }),
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_fn", "pressed": false }),
        })
        .unwrap();
    let opened = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "aux1" }),
        })
        .unwrap();
    assert_eq!(snapshot_from(&opened)["display"]["title"], "Confirm MIDI");
    let messages = confirm_current_dialog(&mut runner);

    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects == &vec![RuntimePlatformEffect::MidiPanic]
    )));
}

#[test]
fn edit_marker_uses_compact_star_prefix() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();

    for _ in 0..5 {
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

    let edit = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    let snapshot = snapshot_from(&edit);
    let lines = snapshot["display"]["lines"].as_array().unwrap();
    assert!(lines.windows(2).any(|pair| {
        pair[0].as_str().unwrap_or("") == "  Master Vol:"
            && pair[1].as_str().unwrap_or("").starts_with(" *")
    }));
}

#[test]
fn midi_sync_mode_edits_through_menu() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();

    for _ in 0..5 {
        let _ = runner.send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 2, "id": "main" }),
        });
    }
    let _ = runner.send(HostMessage::DeviceInput {
        input: json!({ "type": "encoder_press", "id": "main" }),
    });
    let _ = runner.send(HostMessage::DeviceInput {
        input: json!({ "type": "encoder_turn", "delta": 2, "id": "main" }),
    });
    let _ = runner.send(HostMessage::DeviceInput {
        input: json!({ "type": "encoder_press", "id": "main" }),
    });
    let _ = runner.send(HostMessage::DeviceInput {
        input: json!({ "type": "encoder_turn", "delta": 4, "id": "main" }),
    });

    assert_eq!(runner.sync_source, SyncSource::Internal);

    let _ = runner.send(HostMessage::DeviceInput {
        input: json!({ "type": "encoder_press", "id": "main" }),
    });
    let _ = runner.send(HostMessage::DeviceInput {
        input: json!({ "type": "encoder_press", "id": "main" }),
    });
    let changed = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
        })
        .unwrap();

    assert_eq!(runner.sync_source, SyncSource::External);
    assert_eq!(
        snapshot_from(&changed)["settings"]["midi"]["syncMode"],
        "external"
    );
}

#[test]
fn system_menu_refresh_list_emits_store_list_effect() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.state.stack = vec![5, 0, 0];
    runner.menu.state.cursor = 5;

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();

    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects == &vec![RuntimePlatformEffect::StoreListPresets]
    )));
}

#[test]
fn system_menu_midi_panic_emits_panic_effect() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.state.stack = vec![5, 2];
    runner.menu.state.cursor = 1;

    let opened = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    assert_eq!(snapshot_from(&opened)["display"]["title"], "Confirm MIDI");
    let messages = confirm_current_dialog(&mut runner);

    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects == &vec![RuntimePlatformEffect::MidiPanic]
    )));
}

#[test]
fn system_menu_save_default_emits_native_config_payload() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.state.stack = vec![5, 0, 1];
    runner.menu.state.cursor = 0;

    let opened = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    assert_eq!(
        snapshot_from(&opened)["display"]["title"],
        "Confirm Default"
    );
    let messages = confirm_current_dialog(&mut runner);

    let payload = messages
        .iter()
        .find_map(|message| match message {
            RunnerMessage::PlatformEffects { effects } => {
                effects.iter().find_map(|effect| match effect {
                    RuntimePlatformEffect::StoreSaveDefault { payload, .. } => Some(payload),
                    _ => None,
                })
            }
            _ => None,
        })
        .expect("save default payload");
    assert_eq!(payload["activeBehavior"], "life");
    assert!(
        payload["runtimeConfig"]["instruments"]
            .as_array()
            .unwrap()
            .len()
            >= 8
    );
    assert_ne!(payload, &Value::Null);
}

#[test]
fn load_default_result_applies_native_config_payload() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let payload = json!({
        "activeBehavior": "sequencer",
        "runtimeConfig": {
            "activePartIndex": 1,
            "parts": [
                { "l1": { "behaviorId": "life" }, "name": "life" },
                { "l1": { "behaviorId": "sequencer" }, "name": "sequencer" }
            ],
            "instruments": [
                { "type": "sampler", "name": "sampler", "noteBehavior": "hold", "autoName": true, "mixer": { "volume": 70, "panPos": 10 } }
            ],
            "masterVolume": 88,
            "displayBrightness": 66,
            "buttonBrightness": 55,
            "danceMode": "pan",
            "midi": { "enabled": true, "syncMode": "external" }
        },
        "mappingConfig": platform_core::default_mapping_config()
    });

    let messages = runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::LoadDefaultResult {
                payload: Some(payload),
            },
        })
        .unwrap();

    assert_eq!(runner.active_part_index, 1);
    assert_eq!(runner.behavior.id(), "sequencer");
    assert_eq!(runner.instruments[0].kind, "sampler");
    assert_eq!(runner.instruments[0].note_behavior, "hold");
    assert_eq!(runner.note_behaviors[0], NoteBehavior::Hold);
    assert_eq!(runner.ui.master_volume, 88);
    assert_eq!(runner.sync_source, SyncSource::External);
    assert_eq!(snapshot_from(&messages)["activeBehavior"], "sequencer");
}

#[test]
fn midi_store_results_update_native_snapshot_state() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let _ = runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::MidiListOutputsResult {
                outputs: vec![MidiPort {
                    id: "out1".into(),
                    name: "Output 1".into(),
                }],
            },
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::MidiListInputsResult {
                inputs: vec![MidiPort {
                    id: "in1".into(),
                    name: "Input 1".into(),
                }],
            },
        })
        .unwrap();
    let messages = runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::MidiStatus {
                ok: false,
                message: Some("No device".into()),
                selected_out_id: Some("out1".into()),
                selected_in_id: Some("in1".into()),
            },
        })
        .unwrap();
    let midi = &snapshot_from(&messages)["settings"]["midi"];

    assert_eq!(midi["outputs"][0]["id"], "out1");
    assert_eq!(midi["inputs"][0]["id"], "in1");
    assert_eq!(midi["outId"], "out1");
    assert_eq!(midi["inId"], "in1");
    assert_eq!(midi["status"], "No device");
}

#[test]
fn midi_output_menu_selects_dynamic_port() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let _ = runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::MidiListOutputsResult {
                outputs: vec![MidiPort {
                    id: "out1".into(),
                    name: "Output 1".into(),
                }],
            },
        })
        .unwrap();
    runner.menu.state.stack = vec![5, 2, 2];
    runner.menu.state.cursor = 1;

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();

    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects == &vec![RuntimePlatformEffect::MidiSelectOutput { id: Some("out1".into()) }]
    )));
}

#[test]
fn entering_midi_port_groups_requests_port_lists() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.state.stack = vec![5, 2];
    runner.menu.state.cursor = 2;

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();

    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects == &vec![RuntimePlatformEffect::MidiListOutputsRequest]
    )));

    runner.menu.state.stack = vec![5, 2];
    runner.menu.state.cursor = 3;
    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();

    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects == &vec![RuntimePlatformEffect::MidiListInputsRequest]
    )));
}

#[test]
fn preset_load_menu_selects_dynamic_preset() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let _ = runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::ListPresetsResult {
                names: vec!["Alpha".into()],
            },
        })
        .unwrap();
    runner.menu.state.stack = vec![5, 0, 0, 2];
    runner.menu.state.cursor = 1;

    let opened = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    assert_eq!(snapshot_from(&opened)["display"]["title"], "Confirm Load");
    let messages = confirm_current_dialog(&mut runner);

    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects == &vec![RuntimePlatformEffect::StoreLoadPreset { name: "Alpha".into() }]
    )));
}

#[test]
fn save_current_uses_loaded_preset_name() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let _ = runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::LoadPresetResult {
                name: "Alpha".into(),
                payload: Some(json!({ "runtimeConfig": { "activeBehavior": "sequencer" } })),
            },
        })
        .unwrap();
    runner.menu.state.stack = vec![5, 0, 0];
    runner.menu.state.cursor = 1;

    let opened = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    assert_eq!(snapshot_from(&opened)["display"]["title"], "Confirm Save");
    let messages = confirm_current_dialog(&mut runner);

    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if matches!(
                effects.as_slice(),
                [RuntimePlatformEffect::StoreSavePreset { name, mode, .. }]
                    if name == "Alpha" && mode.as_deref() == Some("overwrite")
            )
    )));
}

#[test]
fn native_store_and_action_toasts_cover_common_confirmation_results() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();

    runner.menu.state.stack = vec![5, 0, 0];
    runner.menu.state.cursor = 1;
    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    assert!(!messages
        .iter()
        .any(|message| matches!(message, RunnerMessage::PlatformEffects { .. })));
    assert_eq!(runner.toast.as_ref().unwrap().message, "No preset loaded");

    let _ = runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::LoadPresetResult {
                name: "Alpha".into(),
                payload: Some(json!({ "runtimeConfig": { "activeBehavior": "sequencer" } })),
            },
        })
        .unwrap();
    assert_eq!(runner.toast.as_ref().unwrap().message, "Loaded Alpha");

    let _ = runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::SavePresetResult {
                name: "Alpha".into(),
                outcome: "overwritten".into(),
            },
        })
        .unwrap();
    assert_eq!(runner.toast.as_ref().unwrap().message, "Saved Alpha");

    let _ = runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::DeletePresetResult {
                name: "Alpha".into(),
                ok: true,
            },
        })
        .unwrap();
    assert_eq!(runner.toast.as_ref().unwrap().message, "Deleted Alpha");
}

#[test]
fn midi_panic_and_synth_preset_actions_show_toasts() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let effect = runner
        .execute_confirmed_action(NativeMenuAction::PlatformEffect("midi.panic".into()))
        .unwrap();
    assert_eq!(effect, Some(RuntimePlatformEffect::MidiPanic));
    assert_eq!(runner.toast.as_ref().unwrap().message, "MIDI panic sent");

    runner.load_synth_preset(0, "lead");
    assert_eq!(runner.toast.as_ref().unwrap().message, "Loaded synth lead");
}

#[test]
fn preset_save_as_uses_text_draft_name() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.preset_draft_name = "Jam A".into();
    runner.menu.rebuild(runner.menu_config());
    runner.menu.state.stack = vec![5, 0, 0, 0];
    runner.menu.state.cursor = 1;

    let opened = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    assert_eq!(snapshot_from(&opened)["display"]["title"], "Confirm Save");
    let messages = confirm_current_dialog(&mut runner);

    assert_eq!(runner.preset_draft_name, "Jam A");
    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if matches!(
                effects.as_slice(),
                [RuntimePlatformEffect::StoreSavePreset { name, mode, .. }]
                    if name == "Jam A" && mode.is_none()
            )
    )));
}

#[test]
fn preset_rename_pick_sets_new_name_and_apply_saves() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.preset_names = vec!["Alpha".into()];
    runner.menu.rebuild(runner.menu_config());
    runner.menu.state.stack = vec![5, 0, 0, 3];
    runner.menu.state.cursor = 0;

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();

    assert_eq!(runner.preset_rename_source.as_deref(), Some("Alpha"));
    assert_eq!(runner.preset_draft_name, "Alpha");
    runner.preset_draft_name = "Alpha A".into();
    runner.menu.rebuild(runner.menu_config());
    runner.menu.state.stack = vec![5, 0, 0, 3];
    runner.menu.state.cursor = 2;

    let opened = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    assert_eq!(snapshot_from(&opened)["display"]["title"], "Confirm Rename");
    let messages = confirm_current_dialog(&mut runner);

    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if matches!(
                effects.as_slice(),
                [RuntimePlatformEffect::StoreSavePreset { name, .. }]
                    if name == "Alpha A"
            )
    )));

    let messages = runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::SavePresetResult {
                name: "Alpha A".into(),
                outcome: "created".into(),
            },
        })
        .unwrap();

    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects == &vec![RuntimePlatformEffect::StoreDeletePreset { name: "Alpha".into() }]
    )));
}

#[test]
fn sample_browser_opens_lists_and_picks_sample() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].kind = "sampler".into();
    runner.instruments[0].name = "sampler".into();
    runner.menu.rebuild(runner.menu_config());
    runner.menu.state.stack = vec![2, 0, 0, 2];
    runner.menu.state.cursor = 1;

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();

    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects == &vec![RuntimePlatformEffect::SampleListRequest {
                instrument_slot: 0,
                sample_slot: 0,
                dir: "".into(),
            }]
    )));

    let _ = runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::SampleListResult {
                instrument_slot: 0,
                sample_slot: 0,
                dir: "".into(),
                entries: vec![SampleEntry {
                    name: "kick.wav".into(),
                    path: "Drums/kick.wav".into(),
                    is_dir: false,
                }],
            },
        })
        .unwrap();
    runner.menu.state.stack = vec![2, 0, 0, 2, 1];
    runner.menu.state.cursor = 1;

    let preview = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_s", "pressed": true }),
        })
        .unwrap();
    assert!(preview.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects == &vec![RuntimePlatformEffect::AudioCommand {
                command: RuntimeAudioCommand::SamplePreview {
                    instrument_slot: 0,
                    sample_slot: 0,
                    path: "Drums/kick.wav".into(),
                    velocity: 100,
                },
            }]
    )));

    runner.menu.state.stack = vec![2, 0, 0, 2, 1];
    runner.menu.state.cursor = 1;

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    let snapshot = snapshot_from(&messages);

    assert_eq!(
        snapshot["settings"]["instruments"][0]["sample"]["slots"][0]["path"],
        "Drums/kick.wav"
    );
}

#[test]
fn fn_shift_enter_opens_contextual_help_and_enter_closes_it() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.state.stack = vec![2];
    runner.menu.state.cursor = 0;

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_shift", "pressed": true }),
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_fn", "pressed": true }),
        })
        .unwrap();
    let opened = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    let snapshot = snapshot_from(&opened);
    assert_eq!(snapshot["display"]["title"], "Help: Instruments");
    let help_lines = snapshot["display"]["lines"].as_array().unwrap();
    assert!(help_lines
        .iter()
        .any(|line| line.as_str().unwrap_or("").contains("destination")));
    assert_eq!(help_lines.last().unwrap(), "> Close");

    let closed = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    assert_eq!(snapshot_from(&closed)["display"]["title"], "L3: Voice");
}

#[test]
fn submenu_snapshot_does_not_append_transport_status_line() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.state.stack = vec![1];
    let messages = runner.messages_with_snapshot().unwrap();
    let lines = snapshot_from(&messages)["display"]["lines"]
        .as_array()
        .unwrap()
        .iter()
        .map(|line| line.as_str().unwrap_or_default().to_string())
        .collect::<Vec<_>>();
    assert!(lines
        .iter()
        .all(|line| !line.contains("Stopped") && !line.contains("BPM")));
}

#[test]
fn instrument_pan_menu_edit_moves_monotonically_from_current_value() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].pan_pos = 10;
    runner.menu.rebuild(runner.menu_config());
    runner.menu.state.stack = vec![2, 0, 0, 3];
    runner.menu.state.cursor = 2;
    runner.menu.state.editing = true;

    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
        })
        .unwrap();
    assert_eq!(runner.instruments[0].pan_pos, 11);

    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
        })
        .unwrap();
    assert_eq!(runner.instruments[0].pan_pos, 12);
}

#[test]
fn active_part_trigger_toggle_suppresses_and_restores_with_toast() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.ui.fn_held = true;
    let off = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_s", "pressed": true }),
        })
        .unwrap();
    assert_eq!(runner.trigger_gate_modes[0], "zero");
    assert_eq!(snapshot_from(&off)["display"]["toast"], "P1 triggers off");

    let on = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_s", "pressed": true }),
        })
        .unwrap();
    assert_eq!(runner.trigger_gate_modes[0], "full");
    assert_eq!(snapshot_from(&on)["display"]["toast"], "P1 triggers full");
}

#[test]
fn repeated_autosaves_increment_flash_serial() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.auto_save_default = true;
    let first = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 0, "y": 0 }),
        })
        .unwrap();
    let first_serial = snapshot_from(&first)["settings"]["autoSaveFlashSerial"]
        .as_u64()
        .unwrap();

    let second = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 1, "y": 0 }),
        })
        .unwrap();
    let second_serial = snapshot_from(&second)["settings"]["autoSaveFlashSerial"]
        .as_u64()
        .unwrap();
    assert!(second_serial > first_serial);
}

#[test]
fn config_load_queues_midi_port_selection_effects() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();

    runner
        .apply_config_payload(json!({
            "runtimeConfig": {
                "midi": {
                    "enabled": true,
                    "outId": "out1",
                    "inId": "in1"
                }
            }
        }))
        .unwrap();

    let messages = runner.messages_with_snapshot().unwrap();
    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects == &vec![
                RuntimePlatformEffect::MidiSelectOutput { id: Some("out1".into()) },
                RuntimePlatformEffect::MidiSelectInput { id: Some("in1".into()) },
            ]
    )));
}

#[test]
fn contextual_help_includes_midi_output_guidance() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.state.stack = vec![5, 2];
    runner.menu.state.cursor = 2;
    runner.ui.fn_held = true;
    runner.ui.shift_held = true;

    let opened = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    let lines = snapshot_from(&opened)["display"]["lines"]
        .as_array()
        .unwrap()
        .iter()
        .map(|line| line.as_str().unwrap_or_default())
        .collect::<Vec<_>>()
        .join(" ");

    assert!(lines.contains("output"));
}

#[test]
fn contextual_help_resolves_life_params_and_scrolls_to_bottom() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "life".into(),
        behavior_config: json!({ "randomCellsPerTick": 12, "randomTickInterval": 1 }),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.menu.state.stack = vec![0, 0];
    runner.menu.state.cursor = 4;
    runner.ui.fn_held = true;
    runner.ui.shift_held = true;

    let opened = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    let lines = snapshot_from(&opened)["display"]["lines"]
        .as_array()
        .unwrap()
        .iter()
        .map(|line| line.as_str().unwrap_or_default())
        .collect::<Vec<_>>()
        .join(" ");

    assert!(lines.contains("random cells"));
    runner.ui.fn_held = false;
    runner.ui.shift_held = false;

    runner.turn_help_popup(2);
    let help = runner.help_popup.as_ref().unwrap();
    assert_eq!(
        help.scroll,
        help.lines.len().saturating_sub(OLED_BODY_ROWS - 1)
    );
}

#[test]
fn contextual_help_scrolls_and_back_closes() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.state.cursor = 5;
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_shift", "pressed": true }),
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_fn", "pressed": true }),
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    if let Some(help) = &mut runner.help_popup {
        help.lines = vec!["l1", "l2", "l3", "l4", "l5", "l6", "l7", "l8"]
            .into_iter()
            .map(String::from)
            .collect();
    }

    let scrolled = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 2, "id": "main" }),
        })
        .unwrap();
    assert_eq!(runner.help_popup.as_ref().unwrap().scroll, 2);
    assert!(snapshot_from(&scrolled)["display"]["lines"]
        .as_array()
        .unwrap()[0]
        .as_str()
        .unwrap()
        .contains("l3"));

    let closed = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_a", "pressed": true }),
        })
        .unwrap();
    assert!(runner.help_popup.is_none());
    assert_eq!(snapshot_from(&closed)["display"]["title"], "MENU");
}

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
    let edit = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    assert_eq!(snapshot_from(&edit)["display"]["title"], "L4: Dance");
    assert_eq!(snapshot_from(&edit)["display"]["editing"], true);

    for mode in ["mix", "pan", "fx", "trigger-gate", "xy"] {
        let changed = runner
            .send(HostMessage::DeviceInput {
                input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
            })
            .unwrap();
        assert_eq!(snapshot_from(&changed)["danceMode"], mode);
        assert_eq!(snapshot_from(&changed)["activeDanceMode"], mode);
    }

    for mode in ["trigger-gate", "fx", "pan", "mix", "none"] {
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
fn entering_l1_selects_active_part_row() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.active_part_index = 2;
    runner.menu.rebuild(runner.menu_config());

    let entered = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    let snapshot = snapshot_from(&entered);
    assert_eq!(snapshot["display"]["title"], "L1: Life");
    assert_eq!(snapshot["selectedRow"], 2);
}

#[test]
fn entering_l2_selects_active_part_row_after_event_group() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.active_part_index = 2;
    runner.menu.rebuild(runner.menu_config());

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
        })
        .unwrap();
    let entered = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    let snapshot = snapshot_from(&entered);
    assert_eq!(snapshot["display"]["title"], "L2: Sense");
    assert_eq!(snapshot["selectedRow"], 3);
}

#[test]
fn l2_sense_exposes_aux_mappings_and_enterable_part_rows() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
        })
        .unwrap();
    let entered_l2 = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    let lines = snapshot_from(&entered_l2)["display"]["lines"]
        .as_array()
        .unwrap()
        .clone();
    assert!(lines
        .iter()
        .any(|line| line.as_str().unwrap_or("") == "> Aux Mappings"));
    assert!(lines
        .iter()
        .any(|line| line.as_str().unwrap_or("").contains("P1:")));

    let part = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    let snapshot = snapshot_from(&part);
    assert_eq!(snapshot["display"]["title"], "L2: Sense/P1: life");
    let part_lines = snapshot["display"]["lines"].as_array().unwrap().clone();
    assert!(part_lines
        .iter()
        .any(|line| line.as_str().unwrap_or("") == "> Scanning"));
    assert!(part_lines
        .iter()
        .any(|line| line.as_str().unwrap_or("") == "> Events"));
}

#[test]
fn l2_sense_scan_mode_edits_into_config_payload() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.state.stack = vec![1, 1, 0];
    runner.menu.state.cursor = 0;

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();

    assert_eq!(
        runner.config_payload()["runtimeConfig"]["parts"][0]["l2"]["scanMode"],
        "scanning"
    );
}

#[test]
fn entering_part_row_updates_active_part_index() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 2, "id": "main" }),
        })
        .unwrap();
    let entered = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();

    assert_eq!(runner.active_part_index, 2);
    assert_eq!(
        snapshot_from(&entered)["display"]["title"],
        "L1: Life/P3: life"
    );
}

#[test]
fn instrument_list_shows_compact_name_labels() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[1].kind = "sampler".into();
    runner.instruments[1].name = "sampler".into();
    runner.menu.rebuild(runner.menu_config());

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 2, "id": "main" }),
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    let entered = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();

    let lines = snapshot_from(&entered)["display"]["lines"]
        .as_array()
        .unwrap()
        .clone();
    assert!(lines
        .iter()
        .any(|line| line.as_str().unwrap_or("").contains("I1: synth")));
}

#[test]
fn snapshot_exposes_platform_sized_instrument_slots() {
    let runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let snapshot = runner.snapshot().unwrap();
    let instruments = snapshot["settings"]["instruments"].as_array().unwrap();

    assert_eq!(instruments.len(), INSTRUMENT_COUNT);
    assert!(instruments
        .iter()
        .all(|instrument| instrument["type"] == "synth"));
}

#[test]
fn voice_menu_visibility_follows_instrument_type() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].kind = "sampler".into();
    runner.instruments[0].name = "sampler".into();
    runner.menu.rebuild(runner.menu_config());

    let _ = runner.send(HostMessage::DeviceInput {
        input: json!({ "type": "encoder_turn", "delta": 2, "id": "main" }),
    });
    let _ = runner.send(HostMessage::DeviceInput {
        input: json!({ "type": "encoder_press", "id": "main" }),
    });
    let _ = runner.send(HostMessage::DeviceInput {
        input: json!({ "type": "encoder_press", "id": "main" }),
    });
    let sampler = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    let sampler_lines = snapshot_from(&sampler)["display"]["lines"]
        .as_array()
        .unwrap()
        .clone();
    assert!(sampler_lines
        .iter()
        .any(|line| line.as_str().unwrap_or("") == "> Sampler"));
    assert!(!sampler_lines
        .iter()
        .any(|line| line.as_str().unwrap_or("") == "> Synth"));

    runner.instruments[0].kind = "midi".into();
    runner.instruments[0].name = "midi".into();
    runner.menu.rebuild(runner.menu_config());
    let midi = runner.snapshot().unwrap();
    let midi_lines = midi["display"]["lines"].as_array().unwrap();
    assert!(midi_lines
        .iter()
        .any(|line| line.as_str().unwrap_or("") == "> MIDI"));
    assert!(!midi_lines
        .iter()
        .any(|line| line.as_str().unwrap_or("") == "> Mixer"));
}

#[test]
fn midi_instrument_params_edit_through_menu() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].kind = "midi".into();
    runner.instruments[0].name = "midi".into();
    runner.menu.rebuild(runner.menu_config());
    runner.menu.state.stack = vec![2, 0, 0, 2];
    runner.menu.state.cursor = 1;

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 4, "id": "main" }),
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();

    runner.menu.state.cursor = 2;

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": -10, "id": "main" }),
        })
        .unwrap();
    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();

    assert_eq!(
        snapshot_from(&messages)["settings"]["instruments"][0]["midi"]["channel"],
        5
    );
    assert_eq!(
        snapshot_from(&messages)["settings"]["instruments"][0]["midi"]["velocity"],
        90
    );
}

#[test]
fn instrument_clone_and_reset_actions_update_slots() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].kind = "sampler".into();
    runner.instruments[0].name = "kit".into();
    runner.instruments[0].auto_name = false;

    runner
        .execute_menu_action(NativeMenuAction::ResetInstrument { index: 1 })
        .unwrap();

    assert_eq!(runner.instruments[1].kind, "none");
    assert_eq!(runner.instruments[1].name, "none");
    assert_eq!(runner.instruments[1].midi_channel, 2);

    runner
        .execute_menu_action(NativeMenuAction::CloneInstrument { index: 0 })
        .unwrap();

    assert_eq!(runner.instruments[1].kind, "sampler");
    assert_eq!(runner.instruments[1].name, "sampler");
    assert!(runner.instruments[1].auto_name);
    assert!(!runner.instruments[1].midi_enabled);
    assert_eq!(runner.instruments[1].midi_channel, 2);
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["instruments"][1]["type"],
        "sampler"
    );
}

#[test]
fn synth_gain_edits_into_config_payload() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.state.stack = vec![2, 0, 0, 2, 3];
    runner.menu.state.cursor = 0;

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": -10, "id": "main" }),
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();

    assert_eq!(
        runner.config_payload()["runtimeConfig"]["instruments"][0]["synth"]["amp"]["gainPct"],
        70
    );
}

#[test]
fn sampler_tune_edits_into_config_payload() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].kind = "sampler".into();
    runner.instruments[0].name = "sampler".into();
    runner.menu.rebuild(runner.menu_config());
    runner.menu.state.stack = vec![2, 0, 0, 2];
    runner.menu.state.cursor = 3;

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 7, "id": "main" }),
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();

    assert_eq!(
        runner.config_payload()["runtimeConfig"]["instruments"][0]["sample"]["tuneSemis"],
        7
    );
}

#[test]
fn sampler_extended_params_edit_into_config_payload() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].kind = "sampler".into();
    runner.instruments[0].name = "sampler".into();
    runner.menu.rebuild(runner.menu_config());

    runner.menu.state.stack = vec![2, 0, 0, 2];
    runner.menu.state.cursor = 5;
    runner.menu.state.editing = true;
    runner.menu.turn(-20);
    runner.apply_menu_state().unwrap();

    runner.menu.state.cursor = 6;
    runner.menu.turn(1);
    runner.apply_menu_state().unwrap();

    runner.menu.state.stack = vec![2, 0, 0, 2, 7];
    runner.menu.state.cursor = 0;
    runner.menu.state.editing = true;
    runner.menu.turn(-10);
    runner.apply_menu_state().unwrap();

    runner.menu.state.stack = vec![2, 0, 0, 2, 8];
    runner.menu.state.cursor = 0;
    runner.menu.state.editing = true;
    runner.menu.turn(1);
    runner.apply_menu_state().unwrap();
    runner.menu.state.cursor = 1;
    runner.menu.turn(-10);
    runner.apply_menu_state().unwrap();

    runner.menu.state.stack = vec![2, 0, 0, 2];
    runner.menu.state.cursor = 9;
    runner.menu.state.editing = true;
    runner.menu.turn(-25);
    runner.apply_menu_state().unwrap();

    runner.menu.state.stack = vec![2, 0, 0, 2, 10];
    runner.menu.state.cursor = 0;
    runner.menu.state.editing = true;
    runner.menu.turn(4);
    runner.apply_menu_state().unwrap();

    let sample = &runner.config_payload()["runtimeConfig"]["instruments"][0]["sample"];
    assert_eq!(sample["baseVelocity"], 80);
    assert_eq!(sample["velocityLevelsEnabled"], true);
    assert_eq!(sample["velocityLevels"]["high"], 110);
    assert_eq!(sample["filter"]["type"], "highpass");
    assert_eq!(sample["filter"]["cutoffHz"], 6548);
    assert_eq!(sample["amp"]["velocitySensitivityPct"], 75);
    assert_eq!(sample["ampEnv"]["attackMs"], 25);
}

#[test]
fn fx_bus_slot_type_edits_into_config_payload() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.turn_key("mixer.buses.0.slot1.type", 1);
    runner.apply_menu_state().unwrap();

    assert_eq!(
        runner.config_payload()["runtimeConfig"]["mixer"]["buses"][0]["slot1"]["type"],
        "tremolo"
    );
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["mixer"]["buses"][0]["slot1"]["params"]["rateHz"],
        4.0
    );
}

#[test]
fn global_fx_slot_type_edits_into_config_payload() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.turn_key("mixer.master.slots.0.type", 1);
    runner.apply_menu_state().unwrap();

    assert_eq!(
        runner.config_payload()["runtimeConfig"]["mixer"]["master"]["slots"][0]["type"],
        "vinyl"
    );
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["mixer"]["master"]["slots"][0]["params"]
            ["cracklePct"],
        8
    );
}

#[test]
fn fx_params_edit_into_config_payload() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner
        .apply_config_payload(json!({
            "runtimeConfig": {
                "mixer": {
                    "buses": [{ "slot1": { "type": "delay", "params": { "timeMs": 250, "feedback": 0.35, "mixPct": 35 } } }],
                    "master": { "slots": [{ "type": "distortion", "params": { "drive": 2.5, "clip": 0.6, "mixPct": 100 } }] }
                }
            }
        }))
        .unwrap();
    runner.menu.rebuild(runner.menu_config());

    runner
        .menu
        .turn_key("mixer.buses.0.slot1.params.feedback", 1);
    runner.menu.turn_key("mixer.master.slots.0.params.clip", 1);
    runner.apply_menu_state().unwrap();

    let payload = runner.config_payload();
    assert_eq!(
        payload["runtimeConfig"]["mixer"]["buses"][0]["slot1"]["params"]["feedback"],
        0.36
    );
    assert_eq!(
        payload["runtimeConfig"]["mixer"]["master"]["slots"][0]["params"]["clip"],
        0.65
    );
}

#[test]
fn l1_part_config_always_exposes_auto_name() {
    for behavior_id in ["life", "none", "glider"] {
        let mut runner = NativeRunner::new(NativeRunnerConfig {
            behavior_id: behavior_id.into(),
            ..NativeRunnerConfig::default()
        })
        .unwrap();

        let _ = runner
            .send(HostMessage::DeviceInput {
                input: json!({ "type": "encoder_press", "id": "main" }),
            })
            .unwrap();
        let entered = runner
            .send(HostMessage::DeviceInput {
                input: json!({ "type": "encoder_press", "id": "main" }),
            })
            .unwrap();

        let lines = snapshot_from(&entered)["display"]["lines"]
            .as_array()
            .unwrap()
            .clone();
        assert!(
            lines
                .iter()
                .any(|line| line.as_str().unwrap_or("").contains("Part Name")),
            "{behavior_id} should show Part Name"
        );
        assert!(
            lines
                .iter()
                .any(|line| line.as_str().unwrap_or("").contains("Auto Name")),
            "{behavior_id} should show Auto Name"
        );
    }
}

#[test]
fn behavior_change_updates_active_part_auto_name_label() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 2, "id": "main" }),
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();

    assert_eq!(runner.part_behavior_ids[0], "keys");
    runner.menu.back();
    runner.menu.rebuild(runner.menu_config());
    let snapshot = runner.snapshot().unwrap();
    let lines = snapshot["display"]["lines"].as_array().unwrap();
    assert!(lines
        .iter()
        .any(|line| line.as_str().unwrap_or("") == "> P1: keys"));
}

#[test]
fn part_and_bus_names_round_trip_with_auto_name_flags() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.part_names[0] = "lead".into();
    runner.part_auto_names[0] = false;
    runner.fx_buses[0].name = "space".into();
    runner.fx_buses[0].auto_name = false;
    runner.fx_buses[0].slot1_type = "delay".into();
    let payload = runner.config_payload();

    let mut restored = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    restored.apply_config_payload(payload).unwrap();

    assert_eq!(restored.part_names[0], "lead");
    assert!(!restored.part_auto_names[0]);
    assert_eq!(restored.fx_buses[0].name, "space");
    assert!(!restored.fx_buses[0].auto_name);

    restored
        .apply_config_payload(json!({
            "runtimeConfig": {
                "mixer": {
                    "buses": [{ "slot1": { "type": "delay" }, "slot2": { "type": "duck" }, "autoName": true }]
                }
            }
        }))
        .unwrap();
    assert_eq!(restored.fx_buses[0].name, "delay+duck");
}

#[test]
fn native_text_row_edits_part_name_and_clears_auto_name() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.state.stack = vec![0, 0];
    runner.menu.state.cursor = 2;

    runner.menu.press();
    runner.menu.turn(1);
    let snapshot = runner.menu.snapshot();
    runner.apply_menu_state().unwrap();

    assert!(snapshot.lines.iter().any(|line| line == " *lifeA"));
    assert!(snapshot.lines.iter().all(|line| !line.contains('@')));
    assert_eq!(runner.part_names[0], "lifeA");
    assert!(!runner.part_auto_names[0]);
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["parts"][0]["name"],
        "lifeA"
    );
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["parts"][0]["autoName"],
        false
    );
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

    assert!(active_part["g"].as_i64().unwrap() > 0);
    assert_eq!(none_part["r"].as_i64().unwrap(), 2);
    assert_eq!(none_part["g"].as_i64().unwrap(), 2);
    assert_eq!(none_part["b"].as_i64().unwrap(), 3);
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
    runner.dance_mode = "none".into();
    runner.part_behavior_ids[1] = "none".into();
    runner.part_behavior_ids[2] = "life".into();
    runner.ui.fn_held = true;

    let snapshot = runner.snapshot().unwrap();
    let cells = snapshot["leds"]["cells"].as_array().unwrap();
    let active_part = cells[display_index(0, 0)].as_object().unwrap();
    let none_part = cells[display_index(0, 1)].as_object().unwrap();
    let configured_part = cells[display_index(0, 2)].as_object().unwrap();

    assert!(active_part["g"].as_i64().unwrap() > 0);
    assert!(active_part["b"].as_i64().unwrap() > 0);
    assert_eq!(active_part["g"], active_part["b"]);
    assert!(active_part["r"].as_i64().unwrap() < active_part["g"].as_i64().unwrap());
    assert_eq!(none_part["r"].as_i64().unwrap(), 0);
    assert_eq!(none_part["g"].as_i64().unwrap(), 48);
    assert_eq!(none_part["b"].as_i64().unwrap(), 23);
    assert!(configured_part["g"].as_i64().unwrap() > 0);
    assert!(configured_part["g"].as_i64().unwrap() < active_part["g"].as_i64().unwrap());
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
fn factory_load_applies_native_factory_without_loading_user_default() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let effect = runner
        .execute_confirmed_action(NativeMenuAction::PlatformEffect("factory.load".into()))
        .unwrap();

    assert!(effect.is_none());
    assert_eq!(runner.part_behavior_ids[0], "life");
    assert_eq!(runner.part_behavior_ids[1], "sequencer");
    assert_eq!(runner.part_behavior_ids[2], "none");
    assert_eq!(runner.part_algorithm_step_pulses[1], 24);
    assert_eq!(runner.instruments[0].route, "fx_bus_1");
    assert_eq!(runner.instruments[1].name, "drums");
    assert_eq!(runner.toast.as_ref().unwrap().message, "Factory loaded");
}

#[test]
fn transport_and_event_indicators_appear_in_snapshot() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();

    let start = runner.send(HostMessage::MidiRealtimeStart).unwrap();
    let start_snapshot = snapshot_from(&start);
    assert_eq!(start_snapshot["transportIcon"], "play");
    assert_eq!(start_snapshot["transportFlash"], "measure");
    assert_eq!(start_snapshot["cpuLoadRatio"], 0.0);

    let tick = runner
        .send(HostMessage::TransportPulseStep {
            pulses: 24,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(true),
        })
        .unwrap();
    let tick_snapshot = snapshot_from(&tick);
    assert_eq!(tick_snapshot["transportFlash"], "beat");
    assert_eq!(tick_snapshot["eventDotOn"], true);
}

#[test]
fn external_midi_realtime_respects_clock_in_and_start_stop_settings() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.sync_source = SyncSource::External;

    runner.send(HostMessage::MidiRealtimeStart).unwrap();
    assert_eq!(runner.transport, RuntimeTransportState::Stopped);
    runner.midi_clock_in_enabled = true;
    runner.midi_respond_to_start_stop = false;
    runner.send(HostMessage::MidiRealtimeStart).unwrap();
    assert_eq!(runner.transport, RuntimeTransportState::Stopped);

    runner.midi_respond_to_start_stop = true;
    runner.send(HostMessage::MidiRealtimeStart).unwrap();
    assert_eq!(runner.transport, RuntimeTransportState::Playing);
    runner.midi_clock_in_enabled = false;
    runner.send(HostMessage::MidiRealtimeStop).unwrap();
    assert_eq!(runner.transport, RuntimeTransportState::Playing);
}

#[test]
fn switching_behavior_preserves_previous_behavior_config() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "life".into(),
        behavior_config: json!({ "randomCellsPerTick": 5, "randomTickInterval": 3 }),
        ..NativeRunnerConfig::default()
    })
    .unwrap();

    let _ = runner.send(HostMessage::DeviceInput {
        input: json!({ "type": "encoder_press", "id": "main" }),
    });
    let _ = runner.send(HostMessage::DeviceInput {
        input: json!({ "type": "encoder_press", "id": "main" }),
    });
    let _ = runner.send(HostMessage::DeviceInput {
        input: json!({ "type": "encoder_press", "id": "main" }),
    });
    let _ = runner.send(HostMessage::DeviceInput {
        input: json!({ "type": "encoder_turn", "delta": 2, "id": "main" }),
    });

    assert_eq!(runner.behavior.id(), "keys");

    let _ = runner.send(HostMessage::DeviceInput {
        input: json!({ "type": "encoder_press", "id": "main" }),
    });
    let _ = runner.send(HostMessage::DeviceInput {
        input: json!({ "type": "encoder_press", "id": "main" }),
    });

    let _ = runner.send(HostMessage::DeviceInput {
        input: json!({ "type": "encoder_turn", "delta": -2, "id": "main" }),
    });

    assert_eq!(runner.behavior.id(), "life");
    assert_eq!(runner.behavior_config["randomCellsPerTick"], 5);
    assert_eq!(runner.behavior_config["randomTickInterval"], 3);
}

#[test]
fn sequencer_scanned_sampler_assignment_triggers_assigned_sample_slot() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "sequencer".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.instruments[0].kind = "sampler".into();
    runner.instruments[0].sample_assignments = vec![NativeSampleAssignment {
        x: 0,
        y: 0,
        sample_slot: 2,
        level: None,
    }];
    runner.sense_parts[0].scan_mode = "scanning".into();
    runner.sense_parts[0].scan_axis = "rows".into();
    runner.sense_parts[0].scan_unit = "1/16".into();
    runner.sense_parts[0].scanned_slot = 0;
    runner.sense_parts[0].scanned_action = "note_on".into();
    runner.refresh_active_mapping_config();
    runner.refresh_active_interpretation_profile();
    runner
        .engine
        .set_interpretation_profile(runner.interpretation_profile.clone());

    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 0, "y": 0 }),
        })
        .unwrap();
    runner.transport = RuntimeTransportState::Playing;
    let messages = runner
        .send(HostMessage::TransportPulseStep {
            pulses: 6,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();

    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::MusicalEvents { events }
            if events.iter().any(|event| matches!(
                event,
                MusicalEvent::NoteOn { channel: 0, note: 38, .. }
            ))
    )));
}

#[test]
fn deferred_autosave_payload_restores_active_sequencer_grid_on_startup() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "sequencer".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.auto_save_default = true;

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 2, "y": 3 }),
        })
        .unwrap();
    let payload = messages
        .iter()
        .find_map(|message| match message {
            RunnerMessage::PlatformEffects { effects } => {
                effects.iter().find_map(|effect| match effect {
                    RuntimePlatformEffect::StoreSaveDefault { payload, mode }
                        if mode.as_deref() == Some("deferred") =>
                    {
                        Some(payload.clone())
                    }
                    _ => None,
                })
            }
            _ => None,
        })
        .expect("deferred save payload");

    let mut restored = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    restored
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::LoadDefaultResult {
                payload: Some(payload),
            },
        })
        .unwrap();

    assert_eq!(restored.behavior.id(), "sequencer");
    assert!(restored.engine.model().unwrap().cells[platform_core::grid_index(2, 3)]);
}

#[test]
fn shift_space_emergency_stops_internal_and_external_arms_resync() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.transport = RuntimeTransportState::Playing;
    runner.current_ppqn_pulse = 48;
    runner.tick = 5;
    runner.ui.shift_held = true;

    let stopped = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_s", "pressed": true }),
        })
        .unwrap();
    assert_eq!(runner.transport, RuntimeTransportState::Stopped);
    assert_eq!(runner.current_ppqn_pulse, 0);
    assert!(stopped.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects == &vec![RuntimePlatformEffect::MidiPanic]
    )));

    runner.transport = RuntimeTransportState::Playing;
    runner.current_ppqn_pulse = 48;
    runner.sync_source = SyncSource::External;
    let resync = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_s", "pressed": true }),
        })
        .unwrap();
    assert_eq!(runner.transport, RuntimeTransportState::Playing);
    assert_eq!(snapshot_from(&resync)["transport"]["ppqnPulse"], 48);
    assert!(matches!(
        resync.last(),
        Some(RunnerMessage::RuntimeStatus { status }) if status.pending_resync
    ));
}

#[test]
fn shift_back_clears_active_layer_state() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "sequencer".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 2, "y": 3 }),
        })
        .unwrap();
    assert!(runner.engine.model().unwrap().cells[platform_core::grid_index(2, 3)]);
    runner.ui.shift_held = true;

    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_a", "pressed": true }),
        })
        .unwrap();

    assert!(!runner.engine.model().unwrap().cells[platform_core::grid_index(2, 3)]);
}

#[test]
fn trigger_probability_grid_editor_cycles_cell_row_and_column() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.trigger_probability_assign = Some(0);
    runner.trigger_probability_maps[0] = vec!["zero".into(); GRID_WIDTH * GRID_HEIGHT];

    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 2, "y": 3 }),
        })
        .unwrap();
    assert_eq!(
        runner.trigger_probability_maps[0][3 * GRID_WIDTH + 2],
        "low"
    );

    runner.ui.shift_held = true;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 1, "y": 4 }),
        })
        .unwrap();
    assert!(
        runner.trigger_probability_maps[0][4 * GRID_WIDTH..5 * GRID_WIDTH]
            .iter()
            .all(|value| value == "low")
    );

    runner.ui.fn_held = true;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 6, "y": 1 }),
        })
        .unwrap();
    assert!(
        (0..GRID_HEIGHT).all(|y| runner.trigger_probability_maps[0][y * GRID_WIDTH + 6] == "low")
    );
}

#[test]
fn system_sound_menu_updates_global_sound_config() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.state.stack = vec![5, 1];
    runner.menu.state.cursor = 1;
    runner.menu.state.editing = true;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 3, "id": "main" }),
        })
        .unwrap();
    assert_eq!(runner.global_sound.note_length_ms, 150);

    runner.menu.state.cursor = 2;
    runner.menu.state.editing = true;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": -4, "id": "main" }),
        })
        .unwrap();
    assert_eq!(runner.global_sound.velocity_scale_pct, 80);

    runner.menu.state.cursor = 3;
    runner.menu.state.editing = true;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 2, "id": "main" }),
        })
        .unwrap();
    assert_eq!(runner.global_sound.velocity_curve, VelocityCurve::Hard);
}

#[test]
fn legacy_nested_sound_and_ui_fields_rehydrate_from_payload() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["sound"] = json!({
        "noteLengthMs": 321,
        "velocityScalePct": 77,
        "velocityCurve": "hard",
        "voiceStealingMode": "aggressive"
    });
    payload["runtimeConfig"]["midi"]["clockOutEnabled"] = json!(true);
    payload["runtimeConfig"]["midi"]["clockInEnabled"] = json!(true);
    payload["runtimeConfig"]["midi"]["respondToStartStop"] = json!(false);
    payload["runtimeConfig"]["gridBrightness"] = json!(42);
    payload["runtimeConfig"]["inputEventsWhilePaused"] = json!(false);
    payload["runtimeConfig"]["numericDisplayMode"] = json!("numbers");
    payload["runtimeConfig"]["screenSleepSeconds"] = json!(180);

    runner.apply_config_payload(payload).unwrap();

    assert_eq!(runner.global_sound.note_length_ms, 321);
    assert_eq!(runner.global_sound.velocity_scale_pct, 77);
    assert_eq!(runner.global_sound.velocity_curve, VelocityCurve::Hard);
    assert_eq!(runner.voice_stealing_mode, "aggressive");
    assert!(runner.midi_clock_out_enabled);
    assert!(runner.midi_clock_in_enabled);
    assert!(!runner.midi_respond_to_start_stop);
    assert_eq!(runner.ui.grid_brightness, 42);
    assert!(!runner.input_events_while_paused);
    assert_eq!(runner.ui.numeric_display_mode, "numbers");
    assert_eq!(runner.ui.screen_sleep_seconds, 180);
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["sound"]["noteLengthMs"],
        321
    );
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["inputEventsWhilePaused"],
        false
    );
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["sound"]["voiceStealingMode"],
        "aggressive"
    );
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["midi"]["clockOutEnabled"],
        true
    );
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["midi"]["respondToStartStop"],
        false
    );
}

#[test]
fn dance_fx_grid_press_and_release_emit_audio_commands() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.active_dance_mode = "fx".into();
    runner.dance_fx_assignments.push(NativeDanceFxAssignment {
        x: 2,
        y: 3,
        config: json!({
            "fxType": "stutter",
            "targetKey": "master",
            "params": { "rateHz": 12, "depthPct": 70 }
        }),
    });

    let start = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 2, "y": 3 }),
        })
        .unwrap();
    assert!(start.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if matches!(effects.as_slice(), [RuntimePlatformEffect::AudioCommand { command: RuntimeAudioCommand::MomentaryFxStart { id, fx_type, params, .. } }] if id == "momentary-fx:2:3" && fx_type == "stutter" && params.get("rateHz") == Some(&json!(12)))
    )));

    let stop = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_release", "x": 2, "y": 3 }),
        })
        .unwrap();
    assert!(stop.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects == &vec![RuntimePlatformEffect::AudioCommand { command: RuntimeAudioCommand::MomentaryFxStop { id: "momentary-fx:2:3".into() } }]
    )));
}

#[test]
fn dance_fx_same_config_assignment_toggles_cell_clear() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let config = json!({ "fxType": "stutter", "targetKey": "master", "params": { "rateHz": 8 } });
    runner.dance_fx_assign = Some(config.clone());
    runner.handle_dance_fx_assignment_grid_press(1, 2);
    assert_eq!(runner.dance_fx_assignments.len(), 1);

    runner.dance_fx_assign = Some(config);
    runner.handle_dance_fx_assignment_grid_press(1, 2);
    assert!(runner.dance_fx_assignments.is_empty());
}

#[test]
fn dance_fx_press_replaces_same_type_and_limits_concurrency() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.active_dance_mode = "fx".into();
    for (x, fx_type) in ["stutter", "freeze", "filter_sweep", "pitch_shift"]
        .iter()
        .enumerate()
    {
        runner.dance_fx_assignments.push(NativeDanceFxAssignment {
            x,
            y: 0,
            config: json!({ "fxType": fx_type, "targetKey": "master", "params": {} }),
        });
    }
    runner.dance_fx_assignments.push(NativeDanceFxAssignment {
        x: 4,
        y: 0,
        config: json!({ "fxType": "freeze", "targetKey": "master", "params": {} }),
    });
    runner.dance_fx_assignments.push(NativeDanceFxAssignment {
        x: 5,
        y: 0,
        config: json!({ "fxType": "glitch", "targetKey": "master", "params": {} }),
    });

    for x in 0..4 {
        assert!(!runner.dance_fx_press_effects(x, 0).is_empty());
    }
    assert_eq!(runner.active_dance_fx.len(), 4);

    let replacement = runner.dance_fx_press_effects(4, 0);
    assert_eq!(runner.active_dance_fx.len(), 4);
    assert!(
        matches!(replacement.as_slice(), [RuntimePlatformEffect::AudioCommand { command: RuntimeAudioCommand::MomentaryFxStop { id } }, RuntimePlatformEffect::AudioCommand { command: RuntimeAudioCommand::MomentaryFxStart { id: new_id, fx_type, .. } }] if id == "momentary-fx:1:0" && new_id == "momentary-fx:4:0" && fx_type == "freeze")
    );

    let limited = runner.dance_fx_press_effects(5, 0);
    assert!(limited.is_empty());
    assert_eq!(runner.active_dance_fx.len(), 4);
}

#[test]
fn dance_fx_overlay_marks_active_and_limited_cells() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.active_dance_mode = "fx".into();
    runner.dance_fx_assignments.push(NativeDanceFxAssignment {
        x: 0,
        y: 0,
        config: json!({ "fxType": "stutter", "targetKey": "master", "params": {} }),
    });
    runner.dance_fx_assignments.push(NativeDanceFxAssignment {
        x: 1,
        y: 0,
        config: json!({ "fxType": "stutter", "targetKey": "master", "params": {} }),
    });
    runner.active_dance_fx = vec![
        ("momentary-fx:0:0".into(), "stutter".into()),
        ("momentary-fx:2:0".into(), "freeze".into()),
        ("momentary-fx:3:0".into(), "filter_sweep".into()),
        ("momentary-fx:4:0".into(), "pitch_shift".into()),
    ];

    let snapshot = runner.snapshot().unwrap();
    let cells = snapshot["leds"]["cells"].as_array().unwrap();
    let active = &cells[display_index(0, 0)];
    let limited = &cells[display_index(1, 0)];

    let active_brightness = active["r"].as_i64().unwrap()
        + active["g"].as_i64().unwrap()
        + active["b"].as_i64().unwrap();
    let limited_brightness = limited["r"].as_i64().unwrap()
        + limited["g"].as_i64().unwrap()
        + limited["b"].as_i64().unwrap();
    assert!(active_brightness > limited_brightness);
}

#[test]
fn dance_fx_map_to_grid_stores_config_and_payload_round_trips() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.dance_fx_selected = json!({
        "fxType": "pitch_shift",
        "targetKey": "fx_bus_1",
        "params": { "semitones": 7, "cents": 12, "mixPct": 65 }
    });
    let _ = runner
        .execute_menu_action(crate::native_menu::NativeMenuAction::PlatformEffect(
            "dance.fx.map".into(),
        ))
        .unwrap();
    assert!(runner.dance_fx_assign.is_some());
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 4, "y": 5 }),
        })
        .unwrap();
    assert_eq!(runner.dance_fx_assignments.len(), 1);
    assert_eq!(
        runner.dance_fx_assignments[0].config["fxType"],
        "pitch_shift"
    );

    let payload = runner.config_payload();
    let mut loaded = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    loaded.apply_config_payload(payload).unwrap();
    assert_eq!(loaded.dance_fx_assignments.len(), 1);
    assert_eq!(loaded.dance_fx_assignments[0].x, 4);
    assert_eq!(
        loaded.dance_fx_assignments[0].config["params"]["semitones"],
        7
    );
}

#[test]
fn saved_step_rate_rehydrates_from_default_payload() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.algorithm_step_pulses = 6;
    let payload = runner.config_payload();
    let mut loaded = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    loaded.apply_config_payload(payload).unwrap();
    assert_eq!(loaded.algorithm_step_pulses, 6);
    assert_eq!(
        loaded.config_payload()["runtimeConfig"]["parts"][0]["l1"]["stepRate"],
        "1/16"
    );
}

#[test]
fn per_part_step_rates_round_trip_and_drive_immediate_parts() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "sequencer".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.part_behavior_ids[1] = "sequencer".into();
    runner.part_algorithm_step_pulses[0] = 6;
    runner.part_algorithm_step_pulses[1] = 24;
    runner.algorithm_step_pulses = 6;
    runner.sense_parts[0].scan_mode = "immediate".into();
    runner.sense_parts[1].scan_mode = "immediate".into();
    runner.sense_parts[0].stable_action = "note_on".into();
    runner.sense_parts[1].stable_action = "note_on".into();
    runner.refresh_active_mapping_config();
    runner.refresh_active_interpretation_profile();
    runner
        .engine
        .set_interpretation_profile(runner.interpretation_profile.clone());
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 0, "y": 0 }),
        })
        .unwrap();
    runner.select_active_part(1).unwrap();
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 0, "y": 0 }),
        })
        .unwrap();
    runner.select_active_part(0).unwrap();

    let payload = runner.config_payload();
    assert_eq!(
        payload["runtimeConfig"]["parts"][0]["l1"]["stepRate"],
        "1/16"
    );
    assert_eq!(
        payload["runtimeConfig"]["parts"][1]["l1"]["stepRate"],
        "1/4"
    );

    let mut restored = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    restored.apply_config_payload(payload).unwrap();
    assert_eq!(restored.part_algorithm_step_pulses[0], 6);
    assert_eq!(restored.part_algorithm_step_pulses[1], 24);

    restored.transport = RuntimeTransportState::Playing;
    let _first = restored
        .send(HostMessage::TransportPulseStep {
            pulses: 6,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();
    assert_eq!(restored.part_pulse_accumulators[0], 0);
    assert_eq!(restored.part_pulse_accumulators[1], 6);

    let _second = restored
        .send(HostMessage::TransportPulseStep {
            pulses: 18,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();
    assert_eq!(restored.part_pulse_accumulators[0], 0);
    assert_eq!(restored.part_pulse_accumulators[1], 0);
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

#[test]
fn sense_pitch_mapping_uses_lowest_starting_highest_and_both_axis_steps() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.sense_parts[0].lowest_note = 60;
    runner.sense_parts[0].starting_note = 64;
    runner.sense_parts[0].highest_note = 72;
    runner.sense_parts[0].scale = "major_pentatonic".into();
    runner.sense_parts[0].x_pitch_enabled = true;
    runner.sense_parts[0].x_pitch_steps = 2;
    runner.sense_parts[0].y_pitch_enabled = true;
    runner.sense_parts[0].y_pitch_steps = 5;

    let mapping = runner.mapping_config_for_part(0);
    assert_eq!(mapping.base_midi_note, 60);
    assert_eq!(mapping.starting_midi_note, 64);
    assert_eq!(mapping.max_midi_note, 72);
    assert_eq!(mapping.column_step_degrees, 2);
    assert_eq!(mapping.row_step_degrees, 5);
    let profile = runner.interpretation_profile_for_part(0);
    assert!(matches!(
        profile.x,
        platform_core::AxisStrategy::ScaleStep { step: 2 }
    ));
    assert!(matches!(
        profile.y,
        platform_core::AxisStrategy::ScaleStep { step: 5 }
    ));
}
