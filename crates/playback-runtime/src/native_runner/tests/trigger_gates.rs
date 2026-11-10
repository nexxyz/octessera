use super::*;

#[test]
pub(crate) fn combined_modifier_layer_toggle_preserves_sequencer_tiles() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "sequencer".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.layers[0].link.scan_mode = ScanMode::Scanning;
    runner.layers[0].link.scan_axis = ScanAxis::Rows;
    runner.layers[0].link.scan_unit = "1/16".into();
    runner.layers[0].link.scanned_slot = 0;
    runner.layers[0].link.scanned_action = TriggerAction::NoteOn;
    runner.refresh_active_mapping_config();
    runner.refresh_active_interpretation_profile();
    runner.transport.transport = RuntimeTransportState::Playing;

    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 0, "y": 0 }),
            request_snapshot: None,
        })
        .unwrap();
    assert!(runner.active_engine().model().unwrap().tiles[platform_core::grid_index(0, 0)]);

    runner.display.ui.combined_modifier_held = true;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 0, "y": 0 }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(
        runner.layers[0].trigger_gate_mode,
        TriggerProbabilityMode::Zero
    );
    assert!(runner.active_engine().model().unwrap().tiles[platform_core::grid_index(0, 0)]);

    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 0, "y": 0 }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(
        runner.layers[0].trigger_gate_mode,
        TriggerProbabilityMode::Full
    );
    assert!(runner.active_engine().model().unwrap().tiles[platform_core::grid_index(0, 0)]);
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
    assert_eq!(
        runner.layers[0].trigger_gate_mode,
        TriggerProbabilityMode::Zero
    );

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
    assert_eq!(
        runner.layers[0].trigger_gate_mode,
        TriggerProbabilityMode::Full
    );

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
    assert_eq!(
        runner.layers[2].trigger_gate_mode,
        TriggerProbabilityMode::Zero
    );
    assert_eq!(
        runner.layers[2].link.trigger_probability_mode,
        TriggerProbabilityMode::Zero
    );
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

    let before_state = runner.active_engine().serialized_state().unwrap();
    let before_grid = runner.active_engine().model().unwrap().tiles;
    let before_behavior = runner.active_behavior().id().to_string();
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

    assert_eq!(
        runner.layers[0].trigger_gate_mode,
        TriggerProbabilityMode::Zero
    );
    assert_eq!(
        runner.layers[0].link.trigger_probability_mode,
        TriggerProbabilityMode::Zero
    );
    assert_eq!(
        runner.active_engine().serialized_state().unwrap(),
        before_state
    );
    assert_eq!(runner.active_engine().model().unwrap().tiles, before_grid);
    assert_eq!(runner.active_behavior().id(), before_behavior);
    assert_eq!(runner.active_layer_index, before_active_layer);
    assert_eq!(runner.transport.tick, before_tick);
    assert_eq!(runner.transport.current_ppqn_pulse, before_ppqn_pulse);
    assert_eq!(runner.transport.layer_ticks, before_layer_ticks);
    assert_eq!(
        runner.transport.layer_pulse_accumulators,
        before_layer_pulses
    );
}
