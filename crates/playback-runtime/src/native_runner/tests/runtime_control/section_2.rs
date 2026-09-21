use super::*;

#[test]
pub(crate) fn shift_space_stops_internal_and_external_arms_resync() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.transport.transport = RuntimeTransportState::Playing;
    runner.transport.current_ppqn_pulse = 48;
    runner.transport.tick = 5;
    runner.display.ui.shift_held = true;

    let stopped = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_s", "pressed": true }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(runner.transport.transport, RuntimeTransportState::Stopped);
    assert_eq!(runner.transport.current_ppqn_pulse, 0);
    assert!(stopped.iter().all(|message| !matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects == &vec![RuntimePlatformEffect::MidiPanic]
    )));

    runner.transport.transport = RuntimeTransportState::Playing;
    runner.transport.current_ppqn_pulse = 48;
    runner.transport.sync_source = SyncSource::External;
    let resync = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_s", "pressed": true }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(runner.transport.transport, RuntimeTransportState::Playing);
    assert_eq!(snapshot_from(&resync)["transport"]["ppqnPulse"], 48);
    assert!(matches!(
        resync.last(),
        Some(RunnerMessage::RuntimeStatus { status }) if status.pending_resync
    ));
}

#[test]
pub(crate) fn external_resync_splits_clock_batch_at_measure_boundary() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.midi_enabled = true;
    runner.transport.sync_source = SyncSource::External;
    runner.midi_clock_in_enabled = true;
    runner.transport.transport = RuntimeTransportState::Playing;
    runner.transport.current_ppqn_pulse = 95;
    runner.transport.layer_pulse_accumulators[0] = 5;
    runner.transport.tick = 7;
    runner.transport.pending_resync = true;
    let grid_before = runner.engine.model().unwrap().cells;

    let messages = runner
        .send(HostMessage::MidiRealtimeClock { pulses: 2 })
        .unwrap();

    assert_eq!(runner.transport.current_ppqn_pulse, 1);
    assert_eq!(runner.transport.transport, RuntimeTransportState::Playing);
    assert!(!runner.transport.pending_resync);
    assert_eq!(runner.transport.tick, 0);
    assert_eq!(runner.transport.layer_pulse_accumulators[0], 1);
    assert_eq!(runner.engine.model().unwrap().cells, grid_before);
    assert!(matches!(
        messages.last(),
        Some(RunnerMessage::RuntimeStatus { status })
            if status.current_ppqn_pulse == 1
                && !status.pending_resync
                && status.transport == RuntimeTransportState::Playing
    ));
}

#[test]
pub(crate) fn shift_back_clears_active_layer_state() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "sequencer".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 2, "y": 3 }),
            request_snapshot: None,
        })
        .unwrap();
    assert!(runner.engine.model().unwrap().cells[platform_core::grid_index(2, 3)]);
    runner.display.ui.shift_held = true;

    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_a", "pressed": true }),
            request_snapshot: None,
        })
        .unwrap();

    assert!(!runner.engine.model().unwrap().cells[platform_core::grid_index(2, 3)]);
}

#[test]
pub(crate) fn trigger_probability_grid_editor_cycles_cell_row_and_column() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.trigger_probability_assign = Some(0);
    runner.trigger_probability_maps[0] = vec!["zero".into(); GRID_WIDTH * GRID_HEIGHT];

    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 2, "y": 3 }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(
        runner.trigger_probability_maps[0][3 * GRID_WIDTH + 2],
        "low"
    );

    runner.display.ui.shift_held = true;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 1, "y": 4 }),
            request_snapshot: None,
        })
        .unwrap();
    assert!(
        runner.trigger_probability_maps[0][4 * GRID_WIDTH..5 * GRID_WIDTH]
            .iter()
            .all(|value| value == "low")
    );

    runner.display.ui.shift_held = false;
    runner.display.ui.combined_modifier_held = true;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 6, "y": 1 }),
            request_snapshot: None,
        })
        .unwrap();
    assert!(
        (0..GRID_HEIGHT).all(|y| runner.trigger_probability_maps[0][y * GRID_WIDTH + 6] == "low")
    );
}

#[test]
pub(crate) fn system_sound_menu_updates_global_sound_config() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    assert!(runner.menu.focus_item_key("sound.noteLengthMs"));
    runner.menu.state.editing = true;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 3, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(runner.global_sound.note_length_ms, 150);

    assert!(runner.menu.focus_item_key("sound.velocityScalePct"));
    runner.menu.state.editing = true;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": -4, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(runner.global_sound.velocity_scale_pct, 80);

    assert!(runner.menu.focus_item_key("sound.velocityCurve"));
    runner.menu.state.editing = true;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 2, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(runner.global_sound.velocity_curve, VelocityCurve::Hard);
}
