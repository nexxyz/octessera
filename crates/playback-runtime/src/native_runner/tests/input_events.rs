use super::*;
mod link;
mod link_arp;
mod link_hold;

#[test]
pub(crate) fn interpreting_behavior_grid_press_and_release_emit_musical_events() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "keys".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();

    let press = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 2, "y": 3 }),
            request_snapshot: None,
        })
        .unwrap();
    let release = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_release", "x": 2, "y": 3 }),
            request_snapshot: None,
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
pub(crate) fn keys_with_hold_note_behavior_sustains_until_release() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "keys".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    assert!(runner.menu.focus_item_key("instruments.0.noteBehavior"));
    runner.menu.state.editing = true;
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(runner.instruments[0].note_behavior, "hold");

    let press = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 2, "y": 3 }),
            request_snapshot: None,
        })
        .unwrap();
    assert!(press.iter().any(|message| matches!(
        message,
        RunnerMessage::MusicalEvents { events }
            if events.iter().any(|event| matches!(event, platform_core::MusicalEvent::NoteOn { duration_ms: None, .. }))
    )));

    let release = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_release", "x": 2, "y": 3 }),
            request_snapshot: None,
        })
        .unwrap();
    assert!(release.iter().any(|message| matches!(
        message,
        RunnerMessage::MusicalEvents { events }
            if events.iter().any(|event| matches!(event, platform_core::MusicalEvent::NoteOff { .. }))
    )));
}

#[test]
pub(crate) fn input_events_while_paused_false_suppresses_paused_grid_events() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "keys".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.input_events_while_paused = false;

    let paused_press = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 2, "y": 3 }),
            request_snapshot: None,
        })
        .unwrap();
    runner.transport = RuntimeTransportState::Playing;
    let playing_press = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 3, "y": 3 }),
            request_snapshot: None,
        })
        .unwrap();

    assert!(musical_note_ons(&paused_press).is_empty());
    assert!(musical_note_ons(&playing_press)
        .iter()
        .any(|(_, note)| *note > 0));
}

#[test]
pub(crate) fn trigger_probability_zero_suppresses_input_transition_events() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "keys".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.pulses_layers[0].trigger_probability_mode = "zero".into();
    runner.refresh_active_interpretation_profile();

    let press = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 2, "y": 3 }),
            request_snapshot: None,
        })
        .unwrap();

    assert!(musical_note_ons(&press).is_empty());
}

#[test]
pub(crate) fn none_trigger_targets_do_not_apply_runtime_modulation() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "keys".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    bind_x_to_instrument_volume(&mut runner);
    runner.instruments[0].volume = 10;
    runner.pulses_layers[0].activate_action = "none".into();
    runner.refresh_active_mapping_config();
    runner.refresh_active_interpretation_profile();

    let press = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 7, "y": 3 }),
            request_snapshot: None,
        })
        .unwrap();

    assert!(musical_note_ons(&press).is_empty());
    assert_eq!(runner.instruments[0].volume, 10);
}

#[test]
pub(crate) fn probability_suppressed_events_do_not_apply_runtime_modulation() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "keys".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    bind_x_to_instrument_volume(&mut runner);
    runner.instruments[0].volume = 10;
    runner.pulses_layers[0].trigger_probability_mode = "zero".into();
    runner.refresh_active_interpretation_profile();

    let press = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 7, "y": 3 }),
            request_snapshot: None,
        })
        .unwrap();

    assert!(musical_note_ons(&press).is_empty());
    assert_eq!(runner.instruments[0].volume, 10);
}

#[test]
pub(crate) fn event_enabled_false_suppresses_input_transition_events() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "keys".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.pulses_layers[0].event_enabled = false;
    runner.refresh_active_interpretation_profile();
    runner
        .engine
        .set_interpretation_profile(runner.interpretation_profile.clone());

    let press = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 2, "y": 3 }),
            request_snapshot: None,
        })
        .unwrap();

    assert!(musical_note_ons(&press).is_empty());
}

#[test]
pub(crate) fn trigger_probability_custom_zero_cell_suppresses_transport_events() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "sequencer".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.transport = RuntimeTransportState::Playing;
    runner.algorithm_step_pulses = 24;
    runner.pulses_layers[0].scan_mode = "scanning".into();
    runner.pulses_layers[0].scan_axis = "rows".into();
    runner.pulses_layers[0].scan_unit = "1/4".into();
    runner.pulses_layers[0].scanned_action = "note_on".into();
    runner.pulses_layers[0].trigger_probability_mode = "custom".into();
    runner.trigger_probability_maps[0][2] = "zero".into();
    runner.refresh_active_mapping_config();
    runner.refresh_active_interpretation_profile();
    runner
        .engine
        .set_interpretation_profile(runner.interpretation_profile.clone());

    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 2, "y": 0 }),
            request_snapshot: None,
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
pub(crate) fn scanned_note_on_events_apply_runtime_modulation() {
    let mut runner = scanning_sequencer_runner();
    bind_x_to_instrument_volume(&mut runner);
    runner.instruments[0].volume = 10;
    runner.pulses_layers[0].scanned_action = "note_on".into();
    runner.refresh_active_mapping_config();

    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 7, "y": 0 }),
            request_snapshot: None,
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

    assert!(!musical_note_ons(&messages).is_empty());
    assert_eq!(runner.instruments[0].volume, 100);
}

#[test]
pub(crate) fn scanned_empty_note_on_events_apply_runtime_modulation() {
    let mut runner = scanning_sequencer_runner();
    bind_x_to_instrument_volume(&mut runner);
    runner.instruments[0].volume = 10;
    runner.pulses_layers[0].scanned_action = "none".into();
    runner.pulses_layers[0].scanned_empty_action = "note_on".into();
    runner.refresh_active_mapping_config();

    let messages = runner
        .send(HostMessage::TransportPulseStep {
            pulses: 24,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();

    assert!(!musical_note_ons(&messages).is_empty());
    assert_eq!(runner.instruments[0].volume, 100);
}

#[test]
pub(crate) fn non_interpreting_sequencer_grid_press_does_not_emit_input_event() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "sequencer".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 2, "y": 3 }),
            request_snapshot: None,
        })
        .unwrap();

    assert!(!messages
        .iter()
        .any(|message| matches!(message, RunnerMessage::MusicalEvents { .. })));
}

#[test]
pub(crate) fn grid_state_edit_emits_deferred_auto_save_when_enabled() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "sequencer".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.auto_save_default = true;

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 2, "y": 3 }),
            request_snapshot: None,
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
pub(crate) fn scan_progress_overlay_is_dim_white_and_preserves_live_cell_color() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "sequencer".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.pulses_layers[0].scan_mode = "scanning".into();
    runner.pulses_layers[0].scan_axis = "rows".into();
    runner.tick = 0;
    runner.refresh_active_interpretation_profile();
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 0, "y": 0 }),
            request_snapshot: None,
        })
        .unwrap();
    runner.send(HostMessage::MidiRealtimeStart).unwrap();
    runner.send(HostMessage::MidiRealtimeStart).unwrap();

    let snapshot = runner.snapshot().unwrap();
    let cells = led_cells(&snapshot);
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
pub(crate) fn switching_active_layer_preserves_current_layer_engine_state() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "sequencer".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.layer_behavior_ids[1] = "sequencer".into();
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 2, "y": 3 }),
            request_snapshot: None,
        })
        .unwrap();

    runner.select_active_layer(1).unwrap();
    runner.select_active_layer(0).unwrap();

    let model = runner.engine.model().unwrap();
    assert!(model.cells[platform_core::grid_index(2, 3)]);
}

#[test]
pub(crate) fn reverse_scan_direction_starts_from_last_lane() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "sequencer".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.pulses_layers[0].scan_mode = "scanning".into();
    runner.pulses_layers[0].scan_axis = "rows".into();
    runner.pulses_layers[0].scan_direction = "reverse".into();
    runner.tick = 0;
    runner.refresh_active_interpretation_profile();

    let snapshot = runner.snapshot().unwrap();
    let cells = led_cells(&snapshot);
    let bottom_row = cells[display_index(0, 0)].as_object().unwrap();
    let top_row = cells[display_index(0, GRID_HEIGHT - 1)]
        .as_object()
        .unwrap();

    assert!(top_row["r"].as_i64().unwrap() > bottom_row["r"].as_i64().unwrap());
}

#[test]
pub(crate) fn scan_sections_limit_overlay_to_current_section_lane() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "sequencer".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.pulses_layers[0].scan_mode = "scanning".into();
    runner.pulses_layers[0].scan_axis = "rows".into();
    runner.pulses_layers[0].scan_sections = 2;
    runner.tick = 0;
    runner.refresh_active_interpretation_profile();

    let snapshot = runner.snapshot().unwrap();
    let cells = led_cells(&snapshot);
    let in_section = cells[display_index(3, 0)].as_object().unwrap();
    let out_of_section = cells[display_index(4, 0)].as_object().unwrap();

    assert!(in_section["r"].as_i64().unwrap() > out_of_section["r"].as_i64().unwrap());
}

#[test]
pub(crate) fn pulses_scan_menu_exposes_none_and_scanned_empty_targets() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.pulses_layers[0].scan_mode = "scanning".into();
    runner.menu.rebuild(runner.menu_config());
    let scan_group = &runner.menu.root.children[1].children[4].children[0];
    let labels = scan_group
        .children
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();

    assert!(labels.contains(&"Empty Inst"));
    assert!(labels.contains(&"Empty Trig"));
    assert!(runner
        .menu
        .value_for_key("layers.0.pulses.mapping.scanned_empty.slot")
        .is_some_and(|value| value != "none"));
}

fn bind_x_to_instrument_volume(runner: &mut NativeRunner) {
    runner.param_mods[0].x[0] = Some(NativeParamBinding {
        key: "instruments.0.mixer.volume".into(),
        label: Some("Volume".into()),
        kind: "number".into(),
        min: Some(0.0),
        max: Some(100.0),
        step: Some(1.0),
        user_min: None,
        user_max: None,
        options: vec![],
        invert: false,
    });
}

fn scanning_sequencer_runner() -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "sequencer".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.transport = RuntimeTransportState::Playing;
    runner.algorithm_step_pulses = 24;
    runner.pulses_layers[0].scan_mode = "scanning".into();
    runner.pulses_layers[0].scan_axis = "rows".into();
    runner.pulses_layers[0].scan_unit = "1/4".into();
    runner.refresh_active_mapping_config();
    runner.refresh_active_interpretation_profile();
    runner
        .engine
        .set_interpretation_profile(runner.interpretation_profile.clone());
    runner
}
