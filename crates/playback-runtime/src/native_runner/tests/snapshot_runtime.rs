use super::*;

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
    runner.aux_auto_map_enabled = false;
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
    assert_eq!(snapshot["display"]["toast"], "T1: No binding");
}

#[test]
fn toasts_expire_after_timeout() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.set_toast_for_test("Temporary toast");

    assert_eq!(
        runner.snapshot().unwrap()["display"]["toast"],
        "Temporary toast"
    );

    runner.age_toast_state_for_test(2500);
    let _ = runner.messages_with_snapshot().unwrap();

    assert_eq!(runner.snapshot().unwrap()["display"]["toast"], "");
}

#[test]
fn aux_turn_toast_cooldown_keeps_first_then_shows_latest() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.state.stack = vec![2, 0, 0, 2, 2];
    runner.menu.state.cursor = 1;

    let first = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "id": "aux1", "delta": 1 }),
        })
        .unwrap();
    assert_eq!(snapshot_from(&first)["display"]["toast"], "T1: Cutoff: 223");

    let second = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "id": "aux1", "delta": 1 }),
        })
        .unwrap();
    assert_eq!(
        snapshot_from(&second)["display"]["toast"],
        "T1: Cutoff: 223"
    );

    let third = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "id": "aux1", "delta": 1 }),
        })
        .unwrap();
    assert_eq!(snapshot_from(&third)["display"]["toast"], "T1: Cutoff: 223");

    runner.age_toast_state_for_test(600);
    let after = runner.messages_with_snapshot().unwrap();

    assert_eq!(snapshot_from(&after)["display"]["toast"], "T1: Cutoff: 225");
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
