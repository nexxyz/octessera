use super::*;

#[test]
pub(crate) fn saved_step_rate_rehydrates_from_default_payload() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.algorithm_step_pulses = 6;
    let payload = runner.config_payload();
    let mut loaded = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    loaded.apply_config_payload(payload).unwrap();
    assert_eq!(loaded.algorithm_step_pulses, 6);
    assert_eq!(
        loaded.config_payload()["runtimeConfig"]["layers"][0]["worlds"]["stepRate"],
        "1/16"
    );
}

#[test]
pub(crate) fn per_layer_step_rates_round_trip_and_drive_non_scanning_layers() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "sequencer".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.layer_behavior_ids[1] = "sequencer".into();
    runner.layer_algorithm_step_pulses[0] = 6;
    runner.layer_algorithm_step_pulses[1] = 24;
    runner.algorithm_step_pulses = 6;
    runner.pulses_layers[0].scan_mode = "none".into();
    runner.pulses_layers[1].scan_mode = "none".into();
    runner.pulses_layers[0].stable_action = "note_on".into();
    runner.pulses_layers[1].stable_action = "note_on".into();
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
    runner.select_active_layer(0).unwrap();

    let payload = runner.config_payload();
    assert_eq!(
        payload["runtimeConfig"]["layers"][0]["worlds"]["stepRate"],
        "1/16"
    );
    assert_eq!(
        payload["runtimeConfig"]["layers"][1]["worlds"]["stepRate"],
        "1/4"
    );

    let mut restored = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    restored.apply_config_payload(payload).unwrap();
    assert_eq!(restored.layer_algorithm_step_pulses[0], 6);
    assert_eq!(restored.layer_algorithm_step_pulses[1], 24);

    restored.transport = RuntimeTransportState::Playing;
    let _first = restored
        .send(HostMessage::TransportPulseStep {
            pulses: 6,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();
    assert_eq!(restored.layer_pulse_accumulators[0], 0);
    assert_eq!(restored.layer_pulse_accumulators[1], 6);

    let _second = restored
        .send(HostMessage::TransportPulseStep {
            pulses: 18,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();
    assert_eq!(restored.layer_pulse_accumulators[0], 0);
    assert_eq!(restored.layer_pulse_accumulators[1], 0);
}

#[test]
pub(crate) fn pulses_pitch_mapping_uses_lowest_starting_highest_and_both_axis_steps() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.pulses_layers[0].lowest_note = 60;
    runner.pulses_layers[0].starting_note = 64;
    runner.pulses_layers[0].highest_note = 72;
    runner.pulses_layers[0].scale = "major_pentatonic".into();
    runner.pulses_layers[0].x_pitch_enabled = true;
    runner.pulses_layers[0].x_pitch_steps = 2;
    runner.pulses_layers[0].y_pitch_enabled = true;
    runner.pulses_layers[0].y_pitch_steps = 5;

    let mapping = runner.mapping_config_for_layer(0);
    assert_eq!(mapping.base_midi_note, 60);
    assert_eq!(mapping.starting_midi_note, 64);
    assert_eq!(mapping.max_midi_note, 72);
    assert_eq!(mapping.column_step_degrees, 2);
    assert_eq!(mapping.row_step_degrees, 5);
    let profile = runner.interpretation_profile_for_layer(0);
    assert!(matches!(
        profile.x,
        platform_core::AxisStrategy::ScaleStep { step: 2 }
    ));
    assert!(matches!(
        profile.y,
        platform_core::AxisStrategy::ScaleStep { step: 5 }
    ));
}

#[test]
pub(crate) fn layer_mapping_derives_from_stable_base_config_and_layer_slot_defaults() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.base_mapping_config.base_midi_note = 40;
    runner.base_mapping_config.starting_midi_note = 43;
    runner.base_mapping_config.max_midi_note = 88;
    runner.base_mapping_config.activate.channel = 9;
    runner.pulses_layers[1].x_pitch_enabled = false;
    runner.pulses_layers[1].y_pitch_enabled = false;

    let layer_two_mapping = runner.mapping_config_for_layer(1);

    assert_eq!(layer_two_mapping.base_midi_note, 36);
    assert_eq!(layer_two_mapping.starting_midi_note, 60);
    assert_eq!(layer_two_mapping.max_midi_note, 74);
    assert_eq!(layer_two_mapping.activate.channel, 1);
    assert_eq!(layer_two_mapping.scanned.channel, 1);
}
