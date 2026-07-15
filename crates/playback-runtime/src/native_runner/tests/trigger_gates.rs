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
    runner.transport = RuntimeTransportState::Playing;

    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 0, "y": 0 }),
            request_snapshot: None,
        })
        .unwrap();
    assert!(runner.engine.model().unwrap().cells[platform_core::grid_index(0, 0)]);

    runner.ui.combined_modifier_held = true;
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

    runner.ui.combined_modifier_held = true;
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
    runner.ui.combined_modifier_held = true;

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
