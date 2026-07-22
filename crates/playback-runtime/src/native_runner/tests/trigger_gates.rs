use super::*;

#[test]
pub(crate) fn combined_modifier_layer_toggle_preserves_sequencer_cells() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "sequencer".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.pulses_layers[0].scan_mode = "scanning".into();
    runner.pulses_layers[0].scan_axis = "rows".into();
    runner.pulses_layers[0].scan_unit = "1/16".into();
    runner.pulses_layers[0].scanned_slot = 0;
    runner.pulses_layers[0].scanned_action = "note_on".into();
    runner.refresh_active_mapping_config();
    runner.refresh_active_interpretation_profile();
    runner
        .engine
        .set_interpretation_profile(runner.interpretation_profile.clone());
    runner.transport.transport = RuntimeTransportState::Playing;

    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 0, "y": 0 }),
            request_snapshot: None,
        })
        .unwrap();
    assert!(runner.engine.model().unwrap().cells[platform_core::grid_index(0, 0)]);

    runner.display.ui.combined_modifier_held = true;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 0, "y": 0 }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(runner.trigger_gate_modes[0], "zero");
    assert!(runner.engine.model().unwrap().cells[platform_core::grid_index(0, 0)]);

    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 0, "y": 0 }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(runner.trigger_gate_modes[0], "full");
    assert!(runner.engine.model().unwrap().cells[platform_core::grid_index(0, 0)]);
}

#[test]
pub(crate) fn combined_modifier_layer_toggle_restores_triggered_input_events_after_reenable() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "keys".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();

    let before = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 2, "y": 3 }),
            request_snapshot: None,
        })
        .unwrap();
    assert!(before.iter().any(|message| matches!(
        message,
        RunnerMessage::MusicalEvents { events }
            if events.iter().any(|event| matches!(event, MusicalEvent::NoteOn { .. }))
    )));

    runner.display.ui.combined_modifier_held = true;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 0, "y": 0 }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(runner.trigger_gate_modes[0], "zero");

    let muted = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 3, "y": 3 }),
            request_snapshot: None,
        })
        .unwrap();
    assert!(!muted.iter().any(|message| matches!(
        message,
        RunnerMessage::MusicalEvents { events }
            if events.iter().any(|event| matches!(event, MusicalEvent::NoteOn { .. }))
    )));

    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 0, "y": 0 }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(runner.trigger_gate_modes[0], "full");

    let restored = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 4, "y": 3 }),
            request_snapshot: None,
        })
        .unwrap();
    assert!(restored.iter().any(|message| matches!(
        message,
        RunnerMessage::MusicalEvents { events }
            if events.iter().any(|event| matches!(event, MusicalEvent::NoteOn { .. }))
    )));
}

#[test]
pub(crate) fn combined_modifier_left_column_toggles_selected_layer_without_switching() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.active_layer_index = 0;
    runner.display.ui.combined_modifier_held = true;

    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 0, "y": 2 }),
            request_snapshot: None,
        })
        .unwrap();

    assert_eq!(runner.active_layer_index, 0);
    assert_eq!(runner.trigger_gate_modes[2], "zero");
    assert_eq!(runner.pulses_layers[2].trigger_probability_mode, "zero");
}

#[test]
pub(crate) fn hardware_trigger_gate_toggle_preserves_active_layer_state_and_transport() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "sequencer".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.transport.transport = RuntimeTransportState::Playing;

    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 2, "y": 3 }),
            request_snapshot: None,
        })
        .unwrap();
    runner
        .send(HostMessage::TransportPulseStep {
            pulses: 12,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();

    let before_state = runner.engine.serialized_state().unwrap();
    let before_grid = runner.engine.model().unwrap().cells;
    let before_behavior = runner.behavior.id().to_string();
    let before_active_layer = runner.active_layer_index;
    let before_tick = runner.transport.tick;
    let before_ppqn_pulse = runner.transport.current_ppqn_pulse;
    let before_layer_ticks = runner.transport.layer_ticks.clone();
    let before_layer_pulses = runner.transport.layer_pulse_accumulators.clone();

    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_combined_modifier", "pressed": true }),
            request_snapshot: None,
        })
        .unwrap();
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 0, "y": 0 }),
            request_snapshot: None,
        })
        .unwrap();
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_combined_modifier", "pressed": false }),
            request_snapshot: None,
        })
        .unwrap();

    assert_eq!(runner.trigger_gate_modes[0], "zero");
    assert_eq!(runner.pulses_layers[0].trigger_probability_mode, "zero");
    assert_eq!(runner.engine.serialized_state().unwrap(), before_state);
    assert_eq!(runner.engine.model().unwrap().cells, before_grid);
    assert_eq!(runner.behavior.id(), before_behavior);
    assert_eq!(runner.active_layer_index, before_active_layer);
    assert_eq!(runner.transport.tick, before_tick);
    assert_eq!(runner.transport.current_ppqn_pulse, before_ppqn_pulse);
    assert_eq!(runner.transport.layer_ticks, before_layer_ticks);
    assert_eq!(
        runner.transport.layer_pulse_accumulators,
        before_layer_pulses
    );
}
