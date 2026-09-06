use super::*;

#[test]
pub(crate) fn transport_stop_drains_active_and_inactive_held_notes_to_their_routes() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "keys".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.instruments[0].note_behavior = "hold".into();
    runner.instruments[1].note_behavior = "hold".into();
    runner.instruments[1].kind = "midi".into();
    runner.instruments[1].midi_enabled = true;
    runner.instruments[1].midi_channel = 3;
    runner.pulses_layers[1].event_enabled = true;
    runner.pulses_layers[1].activate_slot = 1;
    runner.select_layer_behavior(1, "keys").unwrap();
    runner.sync_engine_runtime_config();

    let active_press = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 2, "y": 3 }),
            request_snapshot: None,
        })
        .unwrap();
    assert!(active_press.iter().any(|message| matches!(
        message,
        RunnerMessage::MusicalEvents { events }
            if events.iter().any(|event| matches!(
                event,
                MusicalEvent::NoteOn { duration_ms: None, .. }
            ))
    )));

    runner.select_active_layer(1).unwrap();
    let inactive_press = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 2, "y": 3 }),
            request_snapshot: None,
        })
        .unwrap();
    assert!(inactive_press.iter().any(|message| matches!(
        message,
        RunnerMessage::MidiEvents { events }
            if events.iter().any(|event| matches!(
                event,
                MusicalEvent::NoteOn { channel: 2, duration_ms: None, .. }
            ))
    )));
    runner.select_active_layer(0).unwrap();

    let stopped = runner.send(HostMessage::TransportStop).unwrap();
    assert!(stopped.iter().any(|message| matches!(
        message,
        RunnerMessage::MusicalEvents { events }
            if events.iter().any(|event| matches!(event, MusicalEvent::NoteOff { .. }))
    )));
    assert!(stopped.iter().any(|message| matches!(
        message,
        RunnerMessage::MidiEvents { events }
            if events.iter().any(|event| matches!(event, MusicalEvent::NoteOff { channel: 2, .. }))
    )));
}

#[test]
pub(crate) fn pause_continue_and_manual_single_step_preserve_phase_but_stop_start_restarts_it() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "sequencer".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.pulses_layers[0].scan_mode = "scanning".into();
    runner.pulses_layers[0].scan_axis = "rows".into();
    runner.pulses_layers[0].scan_unit = "1/16".into();
    runner.pulses_layers[0].scanned_action = "note_on".into();
    runner.refresh_active_interpretation_profile();
    runner
        .engine
        .set_interpretation_profile(runner.interpretation_profile.clone());
    runner.transport.transport = RuntimeTransportState::Paused;
    runner.display.ui.fn_held = true;

    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(runner.transport.tick, 1);
    assert_eq!(runner.transport.transport, RuntimeTransportState::Paused);

    runner.display.ui.fn_held = false;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_s", "pressed": true }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(runner.transport.transport, RuntimeTransportState::Playing);
    assert_eq!(runner.transport.tick, 1);

    runner.send(HostMessage::MidiRealtimeStop).unwrap();
    assert_eq!(runner.transport.tick, 0);
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_s", "pressed": true }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(runner.transport.transport, RuntimeTransportState::Playing);
    assert_eq!(runner.transport.tick, 0);
}

#[test]
pub(crate) fn transport_origin_resets_probability_rng_but_pause_continue_and_panic_do_not() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.send(HostMessage::MidiRealtimeStart).unwrap();
    runner.trigger_probability_rng = 42;

    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_s", "pressed": true }),
            request_snapshot: None,
        })
        .unwrap();
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_s", "pressed": true }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(runner.trigger_probability_rng, 42);

    let panic = runner
        .execute_confirmed_action(NativeMenuAction::PlatformEffect("midi.panic".into()))
        .unwrap();
    assert_eq!(panic, Some(RuntimePlatformEffect::MidiPanic));
    assert_eq!(runner.trigger_probability_rng, 42);

    runner.send(HostMessage::MidiRealtimeStop).unwrap();
    assert_eq!(
        runner.trigger_probability_rng,
        TRIGGER_PROBABILITY_RNG_INITIAL_SEED
    );
}

#[test]
pub(crate) fn external_clock_advances_only_while_enabled_and_playing() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.transport.sync_source = SyncSource::External;
    runner.midi_clock_in_enabled = true;
    runner.transport.current_ppqn_pulse = 7;

    runner
        .send(HostMessage::MidiRealtimeClock { pulses: 3 })
        .unwrap();
    assert_eq!(runner.transport.current_ppqn_pulse, 7);

    runner.send(HostMessage::MidiRealtimeStart).unwrap();
    runner
        .send(HostMessage::MidiRealtimeClock { pulses: 3 })
        .unwrap();
    assert_eq!(runner.transport.current_ppqn_pulse, 3);

    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_s", "pressed": true }),
            request_snapshot: None,
        })
        .unwrap();
    runner
        .send(HostMessage::MidiRealtimeClock { pulses: 3 })
        .unwrap();
    assert_eq!(runner.transport.current_ppqn_pulse, 3);

    runner.send(HostMessage::MidiRealtimeContinue).unwrap();
    runner.midi_clock_in_enabled = false;
    runner
        .send(HostMessage::MidiRealtimeClock { pulses: 3 })
        .unwrap();
    assert_eq!(runner.transport.current_ppqn_pulse, 3);

    runner.midi_clock_in_enabled = true;
    runner.transport.sync_source = SyncSource::Internal;
    runner
        .send(HostMessage::MidiRealtimeClock { pulses: 3 })
        .unwrap();
    assert_eq!(runner.transport.current_ppqn_pulse, 3);
}

#[test]
pub(crate) fn pending_external_resync_waits_for_playable_clocks() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.transport.sync_source = SyncSource::External;
    runner.midi_clock_in_enabled = true;
    runner.transport.current_ppqn_pulse = 95;
    runner.transport.pending_resync = true;

    runner
        .send(HostMessage::MidiRealtimeClock { pulses: 2 })
        .unwrap();
    assert_eq!(runner.transport.current_ppqn_pulse, 95);
    assert!(runner.transport.pending_resync);

    runner.transport.transport = RuntimeTransportState::Paused;
    runner
        .send(HostMessage::MidiRealtimeClock { pulses: 2 })
        .unwrap();
    assert_eq!(runner.transport.current_ppqn_pulse, 95);
    assert!(runner.transport.pending_resync);

    runner.transport.transport = RuntimeTransportState::Playing;
    runner
        .send(HostMessage::MidiRealtimeClock { pulses: 2 })
        .unwrap();
    assert_eq!(runner.transport.current_ppqn_pulse, 1);
    assert!(!runner.transport.pending_resync);
}
