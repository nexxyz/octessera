use super::*;

fn volume_binding() -> NativeParamBinding {
    volume_binding_for(0)
}

fn volume_binding_for(index: usize) -> NativeParamBinding {
    NativeParamBinding {
        key: format!("instruments.{index}.mixer.volume"),
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

#[test]
pub(crate) fn lfo_phase_advances_in_ppqn_and_wraps_at_period() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.link_lfos[0].enabled = true;
    runner.link_lfos[0].period = "1/1".into();
    runner.link_lfos[0].target = Some(volume_binding());
    runner.transport.transport = RuntimeTransportState::Playing;

    runner
        .send(HostMessage::TransportPulseStep {
            pulses: 24,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();
    assert_eq!(runner.link_lfos[0].phase_pulses, 24);

    runner
        .send(HostMessage::TransportPulseStep {
            pulses: 72,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();
    assert_eq!(runner.link_lfos[0].phase_pulses, 0);
}

#[test]
pub(crate) fn paused_lfo_outputs_freeze_and_new_enable_waits() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].volume = 50;
    for (index, phase) in [(0, 24), (1, 72)] {
        runner.link_lfos[index].enabled = true;
        runner.link_lfos[index].phase_pulses = phase;
        runner.link_lfos[index].target = Some(volume_binding());
    }
    runner.transport.transport = RuntimeTransportState::Playing;
    runner.recompose_lfo_audio(false).unwrap();
    let _ = runner.outbox.drain_audio_commands();

    runner.transport.transport = RuntimeTransportState::Paused;
    runner.recompose_lfo_audio(false).unwrap();
    assert!(runner.outbox.drain_audio_commands().is_empty());

    runner.link_lfos[0].enabled = false;
    runner.recompose_lfo_audio(false).unwrap();
    let resumed = runner.outbox.drain_audio_commands();
    assert!(
        resumed.iter().any(|command| matches!(
            command,
            RuntimeAudioCommand::SetInstrumentMixer {
                instrument_slot: 0,
                volume_pct: Some(0.0),
                ..
            }
        )),
        "{resumed:?}"
    );

    runner.link_lfos[0].enabled = true;
    runner.recompose_lfo_audio(false).unwrap();
    assert!(runner.outbox.drain_audio_commands().is_empty());

    runner.transport.transport = RuntimeTransportState::Playing;
    runner.recompose_lfo_audio(false).unwrap();
    let committed = runner.outbox.drain_audio_commands();
    assert!(
        committed.iter().any(|command| matches!(
            command,
            RuntimeAudioCommand::SetInstrumentMixer {
                instrument_slot: 0,
                volume_pct: Some(50.0),
                ..
            }
        )),
        "{committed:?}"
    );

    runner.transport.transport = RuntimeTransportState::Stopped;
    runner.reset_transport_position();
    let stopped = runner.outbox.drain_audio_commands();
    assert!(!runner.transient_lfo_overlay_for_key("instruments.0.mixer.volume"));
    assert!(
        stopped.is_empty()
            || stopped.iter().any(|command| matches!(
                command,
                RuntimeAudioCommand::SetInstrumentMixer {
                    instrument_slot: 0,
                    volume_pct: Some(50.0),
                    ..
                }
            ))
    );
}

#[test]
pub(crate) fn paused_active_lfo_recomputes_frozen_phase_for_setting_edits() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].volume = 50;
    runner.link_lfos[0].enabled = true;
    runner.link_lfos[0].phase_pulses = 24;
    runner.link_lfos[0].target = Some(volume_binding());
    runner.transport.transport = RuntimeTransportState::Playing;
    runner.recompose_lfo_audio(false).unwrap();
    let _ = runner.outbox.drain_audio_commands();

    runner.transport.transport = RuntimeTransportState::Paused;
    runner.link_lfos[0].depth_pct = 50;
    runner.recompose_lfo_audio(false).unwrap();
    assert!(runner
        .outbox
        .drain_audio_commands()
        .iter()
        .any(|command| matches!(
            command,
            RuntimeAudioCommand::SetInstrumentMixer {
                instrument_slot: 0,
                volume_pct: Some(75.0),
                ..
            }
        )));

    runner.link_lfos[0].period = "1/2".into();
    runner.recompose_lfo_audio(false).unwrap();
    assert!(runner
        .outbox
        .drain_audio_commands()
        .iter()
        .any(|command| matches!(
            command,
            RuntimeAudioCommand::SetInstrumentMixer {
                instrument_slot: 0,
                volume_pct: Some(50.0),
                ..
            }
        )));

    runner.link_lfos[0].period = "1/1".into();
    runner.link_lfos[0].depth_pct = 100;
    runner.recompose_lfo_audio(false).unwrap();
    let _ = runner.outbox.drain_audio_commands();
    let target = runner.link_lfos[0].target.as_mut().unwrap();
    target.user_min = Some(20.0);
    target.user_max = Some(80.0);
    runner.recompose_lfo_audio(false).unwrap();
    assert!(runner
        .outbox
        .drain_audio_commands()
        .iter()
        .any(|command| matches!(
            command,
            RuntimeAudioCommand::SetInstrumentMixer {
                instrument_slot: 0,
                volume_pct: Some(80.0),
                ..
            }
        )));

    runner.link_lfos[0].target.as_mut().unwrap().invert = true;
    runner.recompose_lfo_audio(false).unwrap();
    assert!(runner
        .outbox
        .drain_audio_commands()
        .iter()
        .any(|command| matches!(
            command,
            RuntimeAudioCommand::SetInstrumentMixer {
                instrument_slot: 0,
                volume_pct: Some(20.0),
                ..
            }
        )));
}

#[test]
pub(crate) fn paused_retargeted_lfo_waits_until_playing() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[1].volume = 50;
    runner.link_lfos[0].enabled = true;
    runner.link_lfos[0].phase_pulses = 24;
    runner.link_lfos[0].target = Some(volume_binding());
    runner.transport.transport = RuntimeTransportState::Playing;
    runner.recompose_lfo_audio(false).unwrap();
    let _ = runner.outbox.drain_audio_commands();

    runner.transport.transport = RuntimeTransportState::Paused;
    runner.link_lfos[0].target = Some(volume_binding_for(1));
    runner.recompose_lfo_audio(false).unwrap();
    assert!(runner.outbox.drain_audio_commands().is_empty());
    assert!(!runner.transient_lfo_overlay_for_key("instruments.1.mixer.volume"));

    runner.transport.transport = RuntimeTransportState::Playing;
    runner.recompose_lfo_audio(false).unwrap();
    assert!(runner
        .outbox
        .drain_audio_commands()
        .iter()
        .any(|command| matches!(
            command,
            RuntimeAudioCommand::SetInstrumentMixer {
                instrument_slot: 1,
                volume_pct: Some(100.0),
                ..
            }
        )));
}

#[test]
pub(crate) fn active_lfo_tick_does_not_reapply_unrelated_held_xy_source() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "life".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.select_layer_behavior(1, "brain").unwrap();
    runner.active_sparks_mode = "xy".into();
    runner.xy_touch = NativeXyTouch {
        x: 1.0,
        y: 0.5,
        display_x: 1.0,
        display_y: 0.5,
        active: true,
    };
    runner.xy_x_binding = Some(NativeParamBinding {
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
    runner.refresh_xy_runtime_sources();
    runner.process_modulation_step(false).unwrap();
    let _ = runner.outbox.drain_audio_commands();

    runner.link_lfos[0].enabled = true;
    runner.link_lfos[0].phase_pulses = 24;
    runner.link_lfos[0].target = Some(volume_binding());
    runner.transport.transport = RuntimeTransportState::Playing;
    runner.recompose_lfo_audio(false).unwrap();
    let _ = runner.outbox.drain_audio_commands();
    let rebuilds_after_lfo_start = runner.layer_behavior_rebuilds;

    runner.advance_global_lfo_audio(1).unwrap();

    assert_eq!(runner.layer_behavior_rebuilds, rebuilds_after_lfo_start);
}

#[test]
pub(crate) fn active_lfo_tick_does_no_work_for_unrelated_held_source() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[1].volume = 100;
    runner.xy_touch = NativeXyTouch {
        x: 0.5,
        y: 0.5,
        display_x: 0.5,
        display_y: 0.5,
        active: true,
    };
    runner.xy_x_binding = Some(volume_binding_for(1));
    runner.refresh_xy_runtime_sources();
    runner.process_modulation_step(false).unwrap();
    let _ = runner.outbox.drain_audio_commands();

    runner.link_lfos[0].enabled = true;
    runner.link_lfos[0].phase_pulses = 0;
    runner.link_lfos[0].target = Some(volume_binding());
    runner.transport.transport = RuntimeTransportState::Playing;
    runner.recompose_lfo_audio(false).unwrap();
    let _ = runner.outbox.drain_audio_commands();
    runner.instruments[1].volume = 35;

    runner.advance_global_lfo_audio(1).unwrap();

    assert_eq!(runner.instruments[1].volume, 35);
    assert!(!runner
        .outbox
        .drain_audio_commands()
        .iter()
        .any(|command| matches!(
            command,
            RuntimeAudioCommand::SetInstrumentMixer {
                instrument_slot: 1,
                ..
            }
        )));
}

#[test]
pub(crate) fn depth_zero_lfo_advances_at_24_ppqn_and_depth_change_uses_current_phase() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].volume = 50;
    runner.link_lfos[0].enabled = true;
    runner.link_lfos[0].period = "1/4".into();
    runner.link_lfos[0].depth_pct = 0;
    runner.link_lfos[0].target = Some(volume_binding());
    runner.transport.transport = RuntimeTransportState::Playing;
    runner.modulation_process_calls = 0;

    runner
        .send(HostMessage::TransportPulseStep {
            pulses: 1,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();
    assert_eq!(runner.link_lfos[0].phase_pulses, 1);
    assert_eq!(runner.modulation_process_calls, 1);

    runner
        .send(HostMessage::TransportPulseStep {
            pulses: 23,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();
    assert_eq!(runner.link_lfos[0].phase_pulses, 0);
    assert_eq!(runner.modulation_process_calls, 1);

    runner.link_lfos[0].depth_pct = 100;
    let messages = runner
        .send(HostMessage::TransportPulseStep {
            pulses: 1,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();
    assert_eq!(runner.link_lfos[0].phase_pulses, 1);
    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::AudioCommands { commands }
            if commands.iter().any(|command| matches!(
                command,
                RuntimeAudioCommand::SetInstrumentMixer {
                    instrument_slot: 0,
                    volume_pct: Some(value),
                    ..
                } if *value > 50.0 && *value < 70.0
            ))
    )));
}

#[test]
pub(crate) fn lfo_invert_applies_signed_delta_and_cancels_same_phase() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].volume = 50;
    for index in 0..2 {
        runner.link_lfos[index].enabled = true;
        runner.link_lfos[index].phase_pulses = 24;
        runner.link_lfos[index].target = Some(volume_binding());
    }
    runner.link_lfos[1].target.as_mut().unwrap().invert = true;
    runner.transport.transport = RuntimeTransportState::Playing;
    runner.recompose_lfo_audio(false).unwrap();
    assert!(runner
        .outbox
        .drain_audio_commands()
        .iter()
        .any(|command| matches!(
            command,
            RuntimeAudioCommand::SetInstrumentMixer {
                instrument_slot: 0,
                volume_pct: Some(50.0),
                ..
            }
        )));
}
