use super::*;

#[test]
fn saved_step_rate_rehydrates_from_default_payload() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.algorithm_step_pulses = 6;
    let payload = runner.config_payload();
    let mut loaded = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    loaded.apply_config_payload(payload).unwrap();
    assert_eq!(loaded.algorithm_step_pulses, 6);
    assert_eq!(
        loaded.config_payload()["runtimeConfig"]["parts"][0]["l1"]["stepRate"],
        "1/16"
    );
}

#[test]
fn per_part_step_rates_round_trip_and_drive_non_scanning_parts() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "sequencer".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.part_behavior_ids[1] = "sequencer".into();
    runner.part_algorithm_step_pulses[0] = 6;
    runner.part_algorithm_step_pulses[1] = 24;
    runner.algorithm_step_pulses = 6;
    runner.sense_parts[0].scan_mode = "none".into();
    runner.sense_parts[1].scan_mode = "none".into();
    runner.sense_parts[0].stable_action = "note_on".into();
    runner.sense_parts[1].stable_action = "note_on".into();
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
    runner.select_active_part(0).unwrap();

    let payload = runner.config_payload();
    assert_eq!(
        payload["runtimeConfig"]["parts"][0]["l1"]["stepRate"],
        "1/16"
    );
    assert_eq!(
        payload["runtimeConfig"]["parts"][1]["l1"]["stepRate"],
        "1/4"
    );

    let mut restored = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    restored.apply_config_payload(payload).unwrap();
    assert_eq!(restored.part_algorithm_step_pulses[0], 6);
    assert_eq!(restored.part_algorithm_step_pulses[1], 24);

    restored.transport = RuntimeTransportState::Playing;
    let _first = restored
        .send(HostMessage::TransportPulseStep {
            pulses: 6,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();
    assert_eq!(restored.part_pulse_accumulators[0], 0);
    assert_eq!(restored.part_pulse_accumulators[1], 6);

    let _second = restored
        .send(HostMessage::TransportPulseStep {
            pulses: 18,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();
    assert_eq!(restored.part_pulse_accumulators[0], 0);
    assert_eq!(restored.part_pulse_accumulators[1], 0);
}

#[test]
fn sense_pitch_mapping_uses_lowest_starting_highest_and_both_axis_steps() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.sense_parts[0].lowest_note = 60;
    runner.sense_parts[0].starting_note = 64;
    runner.sense_parts[0].highest_note = 72;
    runner.sense_parts[0].scale = "major_pentatonic".into();
    runner.sense_parts[0].x_pitch_enabled = true;
    runner.sense_parts[0].x_pitch_steps = 2;
    runner.sense_parts[0].y_pitch_enabled = true;
    runner.sense_parts[0].y_pitch_steps = 5;

    let mapping = runner.mapping_config_for_part(0);
    assert_eq!(mapping.base_midi_note, 60);
    assert_eq!(mapping.starting_midi_note, 64);
    assert_eq!(mapping.max_midi_note, 72);
    assert_eq!(mapping.column_step_degrees, 2);
    assert_eq!(mapping.row_step_degrees, 5);
    let profile = runner.interpretation_profile_for_part(0);
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
fn part_mapping_derives_from_stable_base_config_and_part_slot_defaults() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.base_mapping_config.base_midi_note = 40;
    runner.base_mapping_config.starting_midi_note = 43;
    runner.base_mapping_config.max_midi_note = 88;
    runner.base_mapping_config.activate.channel = 9;
    runner.sense_parts[1].x_pitch_enabled = false;
    runner.sense_parts[1].y_pitch_enabled = false;

    let part_two_mapping = runner.mapping_config_for_part(1);

    assert_eq!(part_two_mapping.base_midi_note, 36);
    assert_eq!(part_two_mapping.starting_midi_note, 60);
    assert_eq!(part_two_mapping.max_midi_note, 74);
    assert_eq!(part_two_mapping.activate.channel, 1);
    assert_eq!(part_two_mapping.scanned.channel, 1);
}
