use super::*;

#[test]
pub(crate) fn shift_space_emergency_stops_internal_and_external_arms_resync() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.transport = RuntimeTransportState::Playing;
    runner.current_ppqn_pulse = 48;
    runner.tick = 5;
    runner.ui.shift_held = true;

    let stopped = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_s", "pressed": true }),
            request_snapshot: None,
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
            request_snapshot: None,
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
    runner.ui.shift_held = true;

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

    runner.ui.shift_held = true;
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

    runner.ui.shift_held = false;
    runner.ui.combined_modifier_held = true;
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
    runner.menu.state.stack = vec![5, 3];
    runner.menu.state.cursor = 1;
    runner.menu.state.editing = true;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 3, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(runner.global_sound.note_length_ms, 150);

    runner.menu.state.cursor = 2;
    runner.menu.state.editing = true;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": -4, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(runner.global_sound.velocity_scale_pct, 80);

    runner.menu.state.cursor = 3;
    runner.menu.state.editing = true;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 2, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(runner.global_sound.velocity_curve, VelocityCurve::Hard);
}

#[test]
pub(crate) fn legacy_nested_sound_and_ui_fields_rehydrate_from_payload() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["sound"] = json!({
        "noteLengthMs": 321,
        "velocityScalePct": 77,
        "velocityCurve": "hard",
        "voiceStealingMode": "auto-hard"
    });
    payload["runtimeConfig"]["midi"]["clockOutEnabled"] = json!(true);
    payload["runtimeConfig"]["midi"]["clockInEnabled"] = json!(true);
    payload["runtimeConfig"]["midi"]["respondToStartStop"] = json!(false);
    payload["runtimeConfig"]["gridBrightness"] = json!(42);
    payload["runtimeConfig"]["inputEventsWhilePaused"] = json!(false);
    payload["runtimeConfig"]["numericDisplayMode"] = json!("numbers");
    payload["runtimeConfig"]["screenSleepSeconds"] = json!(180);
    payload["runtimeConfig"]
        .as_object_mut()
        .unwrap()
        .remove("dimTimerSeconds");

    runner.apply_config_payload(payload).unwrap();

    assert_eq!(runner.global_sound.note_length_ms, 321);
    assert_eq!(runner.global_sound.velocity_scale_pct, 77);
    assert_eq!(runner.global_sound.velocity_curve, VelocityCurve::Hard);
    assert_eq!(runner.voice_stealing_mode, "auto-hard");
    assert!(runner.midi_clock_out_enabled);
    assert!(runner.midi_clock_in_enabled);
    assert!(!runner.midi_respond_to_start_stop);
    assert_eq!(runner.ui.grid_brightness, 42);
    assert!(!runner.input_events_while_paused);
    assert_eq!(runner.ui.numeric_display_mode, "numbers");
    assert_eq!(runner.ui.screen_sleep_seconds, 180);
    assert_eq!(runner.ui.dim_timer_seconds, 180);
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
        "auto-hard"
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
pub(crate) fn legacy_screen_sleep_zero_disables_dim_timer_when_dim_timer_is_absent() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["screenSleepSeconds"] = json!(0);
    payload["runtimeConfig"]
        .as_object_mut()
        .unwrap()
        .remove("dimTimerSeconds");

    runner.apply_config_payload(payload).unwrap();

    assert_eq!(runner.ui.screen_sleep_seconds, 0);
    assert_eq!(runner.ui.dim_timer_seconds, 0);
}
