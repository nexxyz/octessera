use super::*;

#[test]
pub(crate) fn factory_load_applies_native_factory_without_loading_user_default() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let effect = runner
        .execute_confirmed_action(NativeMenuAction::PlatformEffect("factory.load".into()))
        .unwrap();

    assert!(effect.is_none());
    assert_eq!(runner.layer_behavior_ids[0], "life");
    assert_eq!(runner.layer_behavior_ids[1], "sequencer");
    assert_eq!(runner.layer_behavior_ids[2], "looper");
    assert_eq!(runner.layer_algorithm_step_pulses[1], 12);
    assert_eq!(runner.pulses_layers[1].scan_unit, "1/8");
    assert_eq!(runner.instruments[0].route, "fx_bus_1");
    assert_eq!(runner.instruments[1].name, "Sampler");
    assert_eq!(runner.toast.as_ref().unwrap().message, "Factory loaded");

    runner.pulses_layers[1].scan_mode = "scanning".into();
    runner.select_active_layer(1).unwrap();
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 0, "y": 0 }),
            request_snapshot: None,
        })
        .unwrap();
    runner.select_active_layer(0).unwrap();
    runner.send(HostMessage::MidiRealtimeStart).unwrap();

    let first = runner
        .send(HostMessage::TransportPulseStep {
            pulses: 6,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();

    let second = runner
        .send(HostMessage::TransportPulseStep {
            pulses: 6,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();
    let mut note_ons = musical_note_ons(&first);
    note_ons.extend(musical_note_ons(&second));
    assert!(note_ons.iter().any(|(channel, _)| *channel == 0));
    assert!(note_ons.iter().any(|(channel, _)| *channel == 1));
}

#[test]
pub(crate) fn clear_all_confirmation_cancel_is_noop() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.current_preset_name = Some("keeper".into());
    runner
        .execute_menu_action(NativeMenuAction::PlatformEffect("system.clearAll".into()))
        .unwrap();
    assert_eq!(
        runner.confirm_dialog.as_ref().unwrap().title,
        "Confirm Clear All"
    );

    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();

    assert_eq!(runner.current_preset_name.as_deref(), Some("keeper"));
    assert_eq!(runner.layer_behavior_ids[0], "life");
    assert_eq!(runner.audio_config_revision, 0);
}

#[test]
pub(crate) fn clear_all_confirm_stops_and_resets_patch_state() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.current_preset_name = Some("old".into());
    runner.preset_rename_source = Some("old".into());
    runner.preset_draft_name = "custom draft".into();
    runner.preset_names = vec!["old".into()];
    runner.instruments[0].kind = "synth".into();
    runner.fx_buses[0].slot1_type = "delay".into();
    runner.sparks_fx_assign = Some(json!({ "type": "delay" }));
    runner.aux_auto_map_enabled = false;
    runner.ui.ghost_cells = false;
    runner.ui.master_volume = 77;
    runner.midi_enabled = true;
    runner.transport = RuntimeTransportState::Playing;

    runner
        .execute_confirmed_action(NativeMenuAction::PlatformEffect("system.clearAll".into()))
        .unwrap();

    assert_eq!(runner.transport, RuntimeTransportState::Stopped);
    assert!(runner
        .outbox
        .drain_platform_effects()
        .iter()
        .any(|effect| matches!(effect, RuntimePlatformEffect::MidiPanic)));
    assert!(runner.layer_behavior_ids.iter().all(|id| id == "none"));
    assert!(runner.layer_behavior_configs.iter().all(Value::is_null));
    assert!(runner.config_payload()["runtimeConfig"]["layers"]
        .as_array()
        .unwrap()
        .iter()
        .all(|layer| layer["worlds"].get("savedState").is_none()));
    assert!(runner
        .instruments
        .iter()
        .all(|instrument| instrument.kind == "none"));
    assert!(runner
        .fx_buses
        .iter()
        .all(|bus| bus.slot1_type == "none" && bus.slot2_type == "none"));
    assert!(runner.global_fx_slots.iter().all(|slot| slot == "none"));
    assert_eq!(runner.trigger_probability_assign, None);
    assert!(runner
        .trigger_probability_maps
        .iter()
        .flatten()
        .all(|mode| mode == "full"));
    assert_eq!(runner.current_preset_name, None);
    assert_eq!(runner.preset_rename_source, None);
    assert_ne!(runner.preset_draft_name, "custom draft");
    assert_eq!(runner.preset_names, vec!["old"]);
    assert!(!runner.aux_auto_map_enabled);
    assert!(!runner.ui.ghost_cells);
    assert_eq!(runner.ui.master_volume, 77);
    assert!(runner.midi_enabled);
    assert_eq!(runner.audio_config_revision, 1);
    assert_eq!(
        runner.menu.value_for_key("instruments.0.type").as_deref(),
        Some("none")
    );
    assert_eq!(
        runner
            .menu
            .value_for_key("mixer.buses.0.slot1.type")
            .as_deref(),
        Some("none")
    );
    assert_eq!(runner.toast.as_ref().unwrap().message, "Cleared all");
    let messages = runner.messages_with_snapshot().unwrap();
    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::AudioCommands { commands }
            if commands.iter().any(|command| matches!(
                command,
                RuntimeAudioCommand::SetAudioConfig { revision: 1, config }
                    if config["instruments"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .all(|instrument| instrument["type"] == "none")
                    && config["mixer"]["buses"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .all(|bus| bus["slot1"]["type"] == "none" && bus["slot2"]["type"] == "none")
            ))
    )));
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

    select_behavior(&mut runner, "keys");

    assert_eq!(runner.behavior.id(), "keys");
    assert_eq!(runner.behavior.id(), "keys");

    select_behavior(&mut runner, "life");
    assert_eq!(runner.behavior.id(), "life");
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

    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 0, "y": 0 }),
            request_snapshot: None,
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
            request_snapshot: None,
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
