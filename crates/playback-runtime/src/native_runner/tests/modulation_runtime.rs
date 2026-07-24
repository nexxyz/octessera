use super::*;

fn volume_binding() -> NativeParamBinding {
    NativeParamBinding {
        key: "instruments.0.mixer.volume".into(),
        label: Some("Volume".into()),
        kind: "number".into(),
        min: Some(0.0),
        max: Some(100.0),
        step: Some(1.0),
        user_min: None,
        user_max: None,
        options: vec![],
        invert: false,
    }
}

fn ranged_volume_binding(min: f64, max: f64) -> NativeParamBinding {
    NativeParamBinding {
        user_min: Some(min),
        user_max: Some(max),
        ..volume_binding()
    }
}

fn pan_binding() -> NativeParamBinding {
    NativeParamBinding {
        key: "instruments.0.mixer.panPos".into(),
        label: Some("Pan".into()),
        kind: "number".into(),
        min: Some(0.0),
        max: Some(32.0),
        step: Some(1.0),
        user_min: None,
        user_max: None,
        options: vec![],
        invert: false,
    }
}

fn audio_commands(messages: &[RunnerMessage]) -> Vec<RuntimeAudioCommand> {
    messages
        .iter()
        .filter_map(|message| match message {
            RunnerMessage::AudioCommands { commands } => Some(commands.clone()),
            _ => None,
        })
        .flatten()
        .collect()
}

#[test]
pub(crate) fn global_lfos_sum_shared_instrument_mixer_target_once() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].volume = 50;
    for (index, phase) in [(0, 24), (1, 72)] {
        runner.link_lfos[index].enabled = true;
        runner.link_lfos[index].phase_pulses = phase;
        runner.link_lfos[index].target = Some(volume_binding());
    }
    runner.transport.transport = RuntimeTransportState::Playing;

    let messages = runner
        .send(HostMessage::TransportPulseStep {
            pulses: 1,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();
    let commands = audio_commands(&messages);
    let mixer = commands
        .iter()
        .find_map(|command| match command {
            RuntimeAudioCommand::SetInstrumentMixer {
                instrument_slot: 0,
                volume_pct,
                pan_pos,
            } => Some((*volume_pct, *pan_pos)),
            _ => None,
        })
        .expect("shared LFO target command");
    assert_eq!(mixer.0, Some(50.0));
    assert_eq!(
        commands
            .iter()
            .filter(|command| matches!(
                command,
                RuntimeAudioCommand::SetInstrumentMixer {
                    instrument_slot: 0,
                    ..
                }
            ))
            .count(),
        1
    );
}

#[test]
pub(crate) fn keyed_base_rebase_preserves_xy_source_and_clear_restores_new_base() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].volume = 30;
    runner.xy_touch = NativeXyTouch {
        x: 0.5,
        y: 0.5,
        display_x: 0.5,
        display_y: 0.5,
        active: true,
    };
    runner.xy_x_binding = Some(volume_binding());
    runner.refresh_xy_runtime_sources();
    runner.process_modulation_step(false).unwrap();
    assert_eq!(runner.instruments[0].volume, 50);

    assert!(runner
        .menu
        .set_number_value_for_key("instruments.0.mixer.volume", 40));
    runner
        .apply_or_schedule_menu_key("instruments.0.mixer.volume")
        .unwrap();
    assert_eq!(runner.instruments[0].volume, 50);

    runner.set_param_binding_target("xy:x", None);
    assert_eq!(runner.instruments[0].volume, 40);
}

#[test]
pub(crate) fn dirty_key_recomposition_includes_all_contributors_for_that_key() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].volume = 50;
    let source_a = crate::native_runner::modulation_source::ModulationSourceId::layer_axis(
        0,
        crate::native_runner::modulation_source::ModulationAxis::X,
        0,
    )
    .unwrap();
    let source_b = crate::native_runner::modulation_source::ModulationSourceId::layer_axis(
        0,
        crate::native_runner::modulation_source::ModulationAxis::X,
        1,
    )
    .unwrap();

    runner.set_runtime_source_input(source_a, volume_binding(), 0.5);
    runner.set_runtime_source_input(source_b, volume_binding(), 0.25);
    runner.process_dirty_modulation_step(false).unwrap();
    assert_eq!(runner.instruments[0].volume, 25);
    let _ = runner.outbox.drain_audio_commands();

    runner.set_runtime_source_input(source_a, volume_binding(), 1.0);
    runner.process_dirty_modulation_step(false).unwrap();
    assert_eq!(runner.instruments[0].volume, 75);
}

#[test]
pub(crate) fn multi_target_grid_modulation_aggregates_revision_and_deferred_autosave() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.select_layer_behavior(1, "brain").unwrap();
    runner.auto_save_default = true;
    runner.config_revision = 0;
    runner.dirty_revision = None;
    runner.fast_autosave_marks = 0;
    runner.layer_behavior_rebuilds = 0;
    runner.behavior_state_serialization_calls.set(0);
    runner.pending.pending_autosave_payload_due_at = None;
    runner.param_mods[0].x[0] = Some(volume_binding());
    runner.param_mods[0].x[1] = Some(pan_binding());
    runner.param_mods[0].y[0] = Some(NativeParamBinding {
        key: "layers.1.worlds.behaviorConfig.randomSeedCells".into(),
        label: Some("Seed Cells".into()),
        kind: "number".into(),
        min: Some(0.0),
        max: Some(20.0),
        step: Some(1.0),
        user_min: None,
        user_max: None,
        options: vec![],
        invert: false,
    });

    runner.apply_runtime_modulation(
        &[platform_core::CellTriggerIntent {
            x: 7,
            y: 7,
            degree: 0,
            kind: platform_core::CellTriggerKind::Activate,
        }],
        0,
    );

    assert_eq!(runner.config_revision, 1);
    assert_eq!(runner.fast_autosave_marks, 1);
    assert_eq!(runner.layer_behavior_rebuilds, 1);
    assert_eq!(runner.behavior_state_serialization_calls.get(), 0);
    assert_eq!(runner.layer_behavior_configs[1]["randomSeedCells"], 20);
    assert!(runner.pending.pending_autosave_payload_due_at.is_some());
    assert!(runner.pending.pending_save_revision.is_none());

    runner.make_deferred_menu_apply_due_for_test();
    let messages = runner.flush_deferred_menu_apply().unwrap();
    assert_eq!(
        messages
            .iter()
            .filter(|message| matches!(
                message,
                RunnerMessage::PlatformEffects { effects }
                    if effects.iter().any(|effect| matches!(
                        effect,
                        RuntimePlatformEffect::StoreSaveDefault { .. }
                    ))
            ))
            .count(),
        1
    );
}

#[test]
pub(crate) fn lfo_ranges_only_scale_deltas_and_canonical_clamp_is_order_independent() {
    for ranges in [[(0.0, 100.0), (20.0, 40.0)], [(20.0, 40.0), (0.0, 100.0)]] {
        let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
        runner.instruments[0].volume = 50;
        for (index, (min, max)) in ranges.into_iter().enumerate() {
            runner.link_lfos[index].enabled = true;
            runner.link_lfos[index].phase_pulses = 24;
            runner.link_lfos[index].depth_pct = 100;
            runner.link_lfos[index].target = Some(ranged_volume_binding(min, max));
        }
        runner.transport.transport = RuntimeTransportState::Playing;

        runner.recompose_lfo_audio(false).unwrap();
        let commands = runner.outbox.drain_audio_commands();
        assert!(commands.iter().any(|command| matches!(
            command,
            RuntimeAudioCommand::SetInstrumentMixer {
                instrument_slot: 0,
                volume_pct: Some(100.0),
                ..
            }
        )));
    }
}

#[test]
pub(crate) fn global_sound_and_note_behavior_modulation_syncs_engine_once() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.param_mods[0].x[0] = Some(NativeParamBinding {
        key: "instruments.0.noteBehavior".into(),
        label: Some("Note Behavior".into()),
        kind: "enum".into(),
        min: None,
        max: None,
        step: None,
        user_min: None,
        user_max: None,
        options: vec!["oneshot".into(), "hold".into()],
        invert: false,
    });
    runner.param_mods[0].x[1] = Some(NativeParamBinding {
        key: "sound.noteLengthMs".into(),
        label: Some("Note Length".into()),
        kind: "number".into(),
        min: Some(30.0),
        max: Some(2000.0),
        step: Some(10.0),
        user_min: None,
        user_max: None,
        options: vec![],
        invert: false,
    });
    runner.engine_runtime_sync_calls = 0;

    runner.apply_runtime_modulation(
        &[platform_core::CellTriggerIntent {
            x: 7,
            y: 0,
            degree: 0,
            kind: platform_core::CellTriggerKind::Activate,
        }],
        0,
    );

    assert_eq!(runner.instruments[0].note_behavior, "hold");
    assert_eq!(runner.note_behaviors[0], platform_core::NoteBehavior::Hold);
    assert_eq!(runner.global_sound.note_length_ms, 2000);
    assert_eq!(runner.engine_runtime_sync_calls, 1);
}

#[test]
pub(crate) fn unchanged_ppqn_without_lfo_does_not_run_modulation_process() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.transport.transport = RuntimeTransportState::Playing;
    runner.modulation_process_calls = 0;

    runner
        .send(HostMessage::TransportPulseStep {
            pulses: 24,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();

    assert_eq!(runner.modulation_process_calls, 0);
}

#[test]
pub(crate) fn active_pulses_modulation_refreshes_mapping_before_next_event() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.param_mods[0].x[0] = Some(NativeParamBinding {
        key: "layers.0.pulses.x.pitch.steps".into(),
        label: Some("X Steps".into()),
        kind: "number".into(),
        min: Some(-16.0),
        max: Some(16.0),
        step: Some(1.0),
        user_min: None,
        user_max: None,
        options: vec![],
        invert: false,
    });
    runner.active_pulses_refresh_calls = 0;

    runner.apply_runtime_modulation(
        &[platform_core::CellTriggerIntent {
            x: 7,
            y: 0,
            degree: 0,
            kind: platform_core::CellTriggerKind::Activate,
        }],
        0,
    );

    assert_eq!(runner.pulses_layers[0].x_pitch_steps, 16);
    assert_eq!(runner.mapping_config.column_step_degrees, 16);
    assert_eq!(runner.active_pulses_refresh_calls, 1);
}

#[test]
pub(crate) fn lfo_pause_holds_stop_restores_and_base_edit_recomposes() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].volume = 50;
    runner.link_lfos[0].enabled = true;
    runner.link_lfos[0].phase_pulses = 24;
    runner.link_lfos[0].target = Some(volume_binding());
    runner.transport.transport = RuntimeTransportState::Playing;
    let _ = runner
        .send(HostMessage::TransportPulseStep {
            pulses: 1,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();
    let phase = runner.link_lfos[0].phase_pulses;

    runner.transport.transport = RuntimeTransportState::Paused;
    let paused = runner
        .send(HostMessage::TransportPulseStep {
            pulses: 12,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();
    assert_eq!(runner.link_lfos[0].phase_pulses, phase);
    assert!(!audio_commands(&paused)
        .iter()
        .any(|command| matches!(command, RuntimeAudioCommand::SetInstrumentMixer { .. })));

    runner.transport.transport = RuntimeTransportState::Stopped;
    runner.reset_transport_position();
    let restored = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 0, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    assert!(audio_commands(&restored).iter().any(|command| matches!(
        command,
        RuntimeAudioCommand::SetInstrumentMixer {
            instrument_slot: 0,
            volume_pct: Some(50.0),
            ..
        }
    )));

    runner.link_lfos[0].enabled = true;
    runner.link_lfos[0].phase_pulses = 0;
    runner.transport.transport = RuntimeTransportState::Playing;
    runner.recompose_lfo_audio(false).unwrap();
    assert!(runner.transient_lfo_overlay_for_key("instruments.0.mixer.volume"));
    assert!(runner
        .menu
        .set_number_value_for_key("instruments.0.mixer.volume", 40));
    runner
        .apply_or_schedule_menu_key("instruments.0.mixer.volume")
        .unwrap();
    assert_eq!(runner.instruments[0].volume, 40);
    assert!(runner.transient_lfo_overlay_for_key("instruments.0.mixer.volume"));
    let base_edit = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 0, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    assert!(audio_commands(&base_edit).iter().any(|command| matches!(
        command,
        RuntimeAudioCommand::SetInstrumentMixer {
            instrument_slot: 0,
            volume_pct: Some(value),
            ..
        } if (value - 40.0).abs() < 0.01
    )));
}

#[test]
pub(crate) fn full_audio_replay_reapplies_lfo_overlay_and_failed_config_preserves_it() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].volume = 50;
    runner.link_lfos[0].enabled = true;
    runner.link_lfos[0].phase_pulses = 24;
    runner.link_lfos[0].target = Some(volume_binding());
    runner.transport.transport = RuntimeTransportState::Playing;
    let _ = runner.recompose_lfo_audio(false);
    runner.audio_config_revision += 1;
    runner.last_snapshot_audio_config_revision = None;
    runner.queue_audio_config_if_changed();
    let replay = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 0, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    let replay_commands = audio_commands(&replay);
    assert!(matches!(
        replay_commands.first(),
        Some(RuntimeAudioCommand::SetAudioConfig { .. })
    ));
    assert!(replay_commands.iter().any(|command| matches!(
        command,
        RuntimeAudioCommand::SetInstrumentMixer {
            instrument_slot: 0,
            ..
        }
    )));

    let before = runner.config_payload();
    let mut invalid = before.clone();
    invalid["runtimeConfig"]["linkLfos"] = json!([]);
    assert!(runner.apply_config_payload(invalid).is_err());
    assert_eq!(runner.config_payload(), before);
    assert!(runner.transient_lfo_overlay_for_key("instruments.0.mixer.volume"));
}
