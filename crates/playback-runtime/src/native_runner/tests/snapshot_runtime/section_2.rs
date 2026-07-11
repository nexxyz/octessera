use super::*;

#[test]
pub(crate) fn transport_tick_advances_multiple_configured_layers() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "sequencer".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.transport = RuntimeTransportState::Playing;
    runner.algorithm_step_pulses = 24;
    runner.layer_behavior_ids[1] = "sequencer".into();
    runner.pulses_layers[0].scan_mode = "scanning".into();
    runner.pulses_layers[0].scanned_slot = 0;
    runner.pulses_layers[1].scan_mode = "scanning".into();
    runner.pulses_layers[1].scanned_slot = 1;
    runner.refresh_active_mapping_config();
    runner.refresh_active_interpretation_profile();
    runner
        .engine
        .set_interpretation_profile(runner.interpretation_profile.clone());
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 0, "y": 0 }),
            request_snapshot: None,
        })
        .unwrap();
    runner.select_active_layer(1).unwrap();
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 0, "y": 0 }),
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
    let notes = musical_note_ons(&messages);

    assert!(notes.iter().any(|(channel, _)| *channel == 0));
    assert!(notes.iter().any(|(channel, _)| *channel == 1));
}

#[test]
pub(crate) fn inactive_layer_transport_tick_applies_param_modulation() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "sequencer".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.transport = RuntimeTransportState::Playing;
    runner.layer_behavior_ids[1] = "sequencer".into();
    runner.pulses_layers[0].scan_mode = "scanning".into();
    runner.pulses_layers[0].scan_axis = "rows".into();
    runner.pulses_layers[0].scanned_slot = 0;
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
            request_snapshot: None,
        })
        .unwrap();
    runner.instruments[0].volume = 0;
    runner.select_active_layer(1).unwrap();

    runner
        .send(HostMessage::TransportPulseStep {
            pulses: 24,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();

    assert_eq!(runner.active_layer_index, 1);
    assert_eq!(runner.instruments[0].volume, 100);
}

#[test]
pub(crate) fn native_snapshot_reserves_bottom_oled_row_for_status() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.state.stack = vec![2, 0, 0, 2, 1];
    runner.menu.state.cursor = 0;
    let snapshot = runner.snapshot().unwrap();

    assert!(snapshot["display"]["lines"].as_array().unwrap().len() <= 6);
}

#[test]
pub(crate) fn native_snapshot_led_payload_is_flat_rgb() {
    let runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let snapshot = runner.snapshot().unwrap();

    assert!(snapshot["leds"].get("cells").is_none());
    assert_eq!(snapshot["leds"]["width"], GRID_WIDTH);
    assert_eq!(snapshot["leds"]["height"], GRID_HEIGHT);
    assert_eq!(
        snapshot["leds"]["rgb"].as_array().unwrap().len(),
        GRID_WIDTH * GRID_HEIGHT * 3
    );
}

#[test]
pub(crate) fn regular_menu_snapshot_keeps_seven_body_rows_above_reserved_status() {
    let runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let snapshot = runner.snapshot().unwrap();

    assert_eq!(snapshot["display"]["lines"].as_array().unwrap().len(), 6);
}

#[test]
pub(crate) fn scan_unit_advances_scanning_before_full_note_step_rate() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "sequencer".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.transport = RuntimeTransportState::Playing;
    runner.algorithm_step_pulses = 96;
    runner.pulses_layers[0].scan_mode = "scanning".into();
    runner.pulses_layers[0].scan_axis = "rows".into();
    runner.pulses_layers[0].scan_unit = "1/4".into();
    runner.refresh_active_mapping_config();
    runner.refresh_active_interpretation_profile();
    runner
        .engine
        .set_interpretation_profile(runner.interpretation_profile.clone());
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 0, "y": 0 }),
            request_snapshot: None,
        })
        .unwrap();
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 0, "y": 1 }),
            request_snapshot: None,
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
pub(crate) fn sequencer_grid_state_is_serialized_and_rehydrated_for_all_layers() {
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
    runner.select_active_layer(1).unwrap();
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 4, "y": 5 }),
            request_snapshot: None,
        })
        .unwrap();
    let payload = runner.config_payload();

    let mut loaded = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    loaded.apply_config_payload(payload).unwrap();
    loaded.select_active_layer(0).unwrap();
    assert!(loaded.engine.model().unwrap().cells[platform_core::grid_index(2, 3)]);
    loaded.select_active_layer(1).unwrap();
    assert!(loaded.engine.model().unwrap().cells[platform_core::grid_index(4, 5)]);
}

#[test]
pub(crate) fn save_grid_state_controls_saved_state_payload_and_restore() {
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
    let mut payload = runner.config_payload();

    assert_eq!(
        payload["runtimeConfig"]["layers"][0]["worlds"]["saveGridState"],
        true
    );
    assert!(!payload["runtimeConfig"]["layers"][0]["worlds"]["savedState"].is_null());
    assert!(payload["runtimeConfig"]["layers"][0]["worlds"]["savedState"]["generation"].is_null());
    assert!(payload["runtimeConfig"]["layers"][0]["worlds"]["savedState"]["tickCounter"].is_null());
    assert!(payload["runtimeConfig"]["layers"][0]["worlds"]["behaviorState"].is_null());

    let mut legacy_payload = payload.clone();
    let saved_state = legacy_payload["runtimeConfig"]["layers"][0]["worlds"]["savedState"].clone();
    legacy_payload["runtimeConfig"]["layers"][0]["worlds"]["savedState"] = Value::Null;
    legacy_payload["runtimeConfig"]["layers"][0]["worlds"]["behaviorState"] = saved_state;
    let mut legacy_loaded = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    legacy_loaded.apply_config_payload(legacy_payload).unwrap();
    assert!(legacy_loaded.engine.model().unwrap().cells[platform_core::grid_index(2, 3)]);

    payload["runtimeConfig"]["layers"][0]["worlds"]["saveGridState"] = json!(false);
    let mut loaded = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    loaded.apply_config_payload(payload).unwrap();

    assert!(!loaded.engine.model().unwrap().cells[platform_core::grid_index(2, 3)]);
    assert_eq!(
        loaded.config_payload()["runtimeConfig"]["layers"][0]["worlds"]["saveGridState"],
        false
    );
    assert!(
        loaded.config_payload()["runtimeConfig"]["layers"][0]["worlds"]["savedState"].is_null()
    );
}
