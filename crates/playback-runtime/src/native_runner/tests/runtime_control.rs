use super::*;
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
    assert_eq!(runner.sense_parts[1].scan_unit, "1/8");
    assert_eq!(runner.instruments[0].route, "fx_bus_1");
    assert_eq!(runner.instruments[1].name, "drums");
    assert_eq!(runner.toast.as_ref().unwrap().message, "Factory loaded");

    runner.sense_parts[1].scan_mode = "scanning".into();
    runner.select_active_part(1).unwrap();
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 0, "y": 0 }),
        })
        .unwrap();
    runner.select_active_part(0).unwrap();
    runner.send(HostMessage::MidiRealtimeStart).unwrap();

    let first = runner
        .send(HostMessage::TransportPulseStep {
            pulses: 6,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();
    assert!(musical_note_ons(&first).is_empty());

    let second = runner
        .send(HostMessage::TransportPulseStep {
            pulses: 6,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();
    assert!(musical_note_ons(&second)
        .iter()
        .any(|(channel, _)| *channel == 1));
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

    assert_eq!(runner.behavior.id(), "life");
    runner.make_deferred_menu_apply_due_for_test();
    assert!(runner.flush_deferred_menu_apply().unwrap().is_empty());
    let _ = runner.send(HostMessage::DeviceInput {
        input: json!({ "type": "encoder_press", "id": "main" }),
    });
    assert_eq!(runner.behavior.id(), "keys");

    let _ = runner.send(HostMessage::DeviceInput {
        input: json!({ "type": "encoder_press", "id": "main" }),
    });

    let _ = runner.send(HostMessage::DeviceInput {
        input: json!({ "type": "encoder_turn", "delta": -2, "id": "main" }),
    });

    runner.make_deferred_menu_apply_due_for_test();
    let _ = runner.flush_deferred_menu_apply().unwrap();
    assert_eq!(runner.behavior.id(), "keys");
    let _ = runner.send(HostMessage::DeviceInput {
        input: json!({ "type": "encoder_press", "id": "main" }),
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
            if events.iter().any(|event| matches!(event, MusicalEvent::NoteOn { channel: 0, note: 38, .. }))
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

    runner.ui.shift_held = false;
    runner.ui.combined_modifier_held = true;
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
    runner.menu.state.stack = vec![5, 3];
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
        "voiceStealingMode": "auto-hard"
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
    assert_eq!(runner.voice_stealing_mode, "auto-hard");
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
