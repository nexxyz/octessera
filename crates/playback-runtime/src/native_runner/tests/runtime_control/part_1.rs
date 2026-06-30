use super::*;

#[test]
pub(crate) fn factory_load_applies_native_factory_without_loading_user_default() {
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
pub(crate) fn external_midi_realtime_respects_clock_in_and_start_stop_settings() {
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
pub(crate) fn switching_behavior_preserves_previous_behavior_config() {
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

    assert_eq!(runner.behavior.id(), "keys");
    runner.make_deferred_menu_apply_due_for_test();
    let _ = runner.flush_deferred_menu_apply().unwrap();
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
    assert_eq!(runner.behavior.id(), "life");
    let _ = runner.send(HostMessage::DeviceInput {
        input: json!({ "type": "encoder_press", "id": "main" }),
    });
    assert_eq!(runner.behavior.id(), "life");
    assert_eq!(runner.behavior_config["randomCellsPerTick"], 5);
    assert_eq!(runner.behavior_config["randomTickInterval"], 3);
}

#[test]
pub(crate) fn sequencer_scanned_sampler_assignment_triggers_assigned_sample_slot() {
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
pub(crate) fn deferred_autosave_payload_restores_active_sequencer_grid_on_startup() {
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
