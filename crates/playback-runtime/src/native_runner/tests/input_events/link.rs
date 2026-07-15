use super::*;

#[test]
pub(crate) fn link_event_timing_payload_round_trips_and_clamps() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner
        .apply_config_payload(json!({
            "runtimeConfig": {
                "layers": [{
                    "pulses": {
                        "mapping": {
                            "activate": { "slot": "1", "action": "note_on", "delaySteps": 99, "retriggerCount": 99 },
                            "stable": { "slot": "1", "action": "note_on", "delaySteps": 2, "retriggerCount": 3 },
                            "deactivate": { "slot": "1", "action": "note_off", "delaySteps": 4, "retriggerCount": 5 },
                            "scanned": { "slot": "1", "action": "note_on", "delaySteps": 6, "retriggerCount": 7 },
                            "scanned_empty": { "slot": "none", "action": "none", "delaySteps": 8, "retriggerCount": 9 }
                        }
                    }
                }]
            }
        }))
        .unwrap();

    let layer = &runner.pulses_layers[0];
    assert_eq!(layer.activate_timing.delay_steps, 16);
    assert_eq!(layer.activate_timing.retrigger_count, 8);
    assert_eq!(layer.stable_timing.delay_steps, 2);
    assert_eq!(layer.deactivate_timing.retrigger_count, 5);
    assert_eq!(layer.scanned_timing.delay_steps, 6);
    assert_eq!(layer.scanned_empty_timing.retrigger_count, 8);
    let payload = runner.config_payload();
    assert_eq!(
        payload["runtimeConfig"]["layers"][0]["pulses"]["mapping"]["activate"]["delaySteps"],
        16
    );
    assert_eq!(
        payload["runtimeConfig"]["layers"][0]["pulses"]["mapping"]["stable"]["retriggerCount"],
        3
    );
}

#[test]
pub(crate) fn link_event_zero_delay_preserves_immediate_original_with_retrigger() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "keys".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.transport = RuntimeTransportState::Playing;
    runner.pulses_layers[0].activate_timing.retrigger_count = 1;

    let press = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 2, "y": 3 }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(musical_note_ons(&press).len(), 1);

    let retrigger = runner
        .send(HostMessage::TransportPulseStep {
            pulses: runner.algorithm_step_pulses,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();
    assert_eq!(musical_note_ons(&retrigger).len(), 1);
}

#[test]
pub(crate) fn link_event_delay_and_retrigger_schedule_routed_events() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "keys".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.transport = RuntimeTransportState::Playing;
    runner.pulses_layers[0].activate_timing.delay_steps = 1;
    runner.pulses_layers[0].activate_timing.retrigger_count = 1;

    let press = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 2, "y": 3 }),
            request_snapshot: None,
        })
        .unwrap();
    assert!(musical_note_ons(&press).is_empty());

    let tick = runner
        .send(HostMessage::TransportPulseStep {
            pulses: runner.algorithm_step_pulses,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();
    assert_eq!(musical_note_ons(&tick).len(), 1);

    let retrigger = runner
        .send(HostMessage::TransportPulseStep {
            pulses: runner.algorithm_step_pulses,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();
    assert_eq!(musical_note_ons(&retrigger).len(), 1);
}

#[test]
pub(crate) fn deactivate_note_off_uses_deactivate_timing() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "keys".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.transport = RuntimeTransportState::Playing;
    runner.instruments[0].note_behavior = "hold".into();
    runner.pulses_layers[0].deactivate_timing.delay_steps = 1;
    let press = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 2, "y": 3 }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(musical_note_ons(&press).len(), 1);

    let release = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_release", "x": 2, "y": 3 }),
            request_snapshot: None,
        })
        .unwrap();
    assert!(!release.iter().any(|message| matches!(
        message,
        RunnerMessage::MusicalEvents { events }
            if events.iter().any(|event| matches!(event, platform_core::MusicalEvent::NoteOff { .. }))
    )));

    let tick = runner
        .send(HostMessage::TransportPulseStep {
            pulses: runner.algorithm_step_pulses,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();
    assert!(tick.iter().any(|message| matches!(
        message,
        RunnerMessage::MusicalEvents { events }
            if events.iter().any(|event| matches!(event, platform_core::MusicalEvent::NoteOff { .. }))
    )));
}

#[test]
pub(crate) fn mixed_note_off_and_note_on_use_their_own_link_timing() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "keys".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.transport = RuntimeTransportState::Playing;
    runner.pulses_layers[0].deactivate_timing.delay_steps = 0;
    runner.pulses_layers[0].activate_timing.delay_steps = 1;
    let deactivate = platform_core::CellTriggerIntent {
        x: 2,
        y: 3,
        degree: 0,
        kind: platform_core::CellTriggerKind::Deactivate,
    };
    let activate = platform_core::CellTriggerIntent {
        x: 3,
        y: 3,
        degree: 1,
        kind: platform_core::CellTriggerKind::Activate,
    };
    let model = runner.engine.model().unwrap();

    let immediate = runner
        .messages_with_input_result(platform_core::NativeInputResult {
            events: vec![
                platform_core::MusicalEvent::NoteOff {
                    channel: 0,
                    note: 60,
                },
                platform_core::MusicalEvent::NoteOn {
                    channel: 0,
                    note: 62,
                    velocity: 96,
                    duration_ms: Some(150),
                },
            ],
            emitted_events: vec![],
            mapped_intents: vec![activate.clone()],
            event_intents: vec![Some(deactivate), Some(activate)],
            model,
        })
        .unwrap();

    assert!(immediate.iter().any(|message| matches!(
        message,
        RunnerMessage::MusicalEvents { events }
            if events.iter().any(|event| matches!(event, platform_core::MusicalEvent::NoteOff { .. }))
    )));
    assert!(musical_note_ons(&immediate).is_empty());

    let delayed = runner
        .send(HostMessage::TransportPulseStep {
            pulses: runner.algorithm_step_pulses,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();

    assert_eq!(musical_note_ons(&delayed), vec![(0, 62)]);
}

#[test]
pub(crate) fn events_without_mapped_intents_bypass_link_timing() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "keys".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.transport = RuntimeTransportState::Playing;
    runner.pulses_layers[0].activate_timing.delay_steps = 1;
    let model = runner.engine.model().unwrap();

    let immediate = runner
        .messages_with_input_result(platform_core::NativeInputResult {
            events: vec![platform_core::MusicalEvent::NoteOn {
                channel: 0,
                note: 60,
                velocity: 96,
                duration_ms: Some(150),
            }],
            emitted_events: vec![],
            mapped_intents: vec![],
            event_intents: vec![None],
            model,
        })
        .unwrap();

    assert_eq!(musical_note_ons(&immediate), vec![(0, 60)]);

    let delayed = runner
        .send(HostMessage::TransportPulseStep {
            pulses: runner.algorithm_step_pulses,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();

    assert!(musical_note_ons(&delayed).is_empty());
}

#[test]
pub(crate) fn link_routing_rejects_event_intent_length_mismatch() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "keys".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    let model = runner.engine.model().unwrap();

    let result = runner.messages_with_input_result(platform_core::NativeInputResult {
        events: vec![platform_core::MusicalEvent::NoteOn {
            channel: 0,
            note: 60,
            velocity: 96,
            duration_ms: Some(150),
        }],
        emitted_events: vec![],
        mapped_intents: vec![],
        event_intents: vec![],
        model,
    });

    assert!(result
        .unwrap_err()
        .contains("event intent metadata length mismatch"));
}

#[test]
pub(crate) fn same_note_events_with_distinct_link_timing_are_split_before_dedupe() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "keys".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.transport = RuntimeTransportState::Playing;
    runner.pulses_layers[0].activate_timing.delay_steps = 0;
    runner.pulses_layers[0].scanned_timing.delay_steps = 1;
    let activate = platform_core::CellTriggerIntent {
        x: 2,
        y: 3,
        degree: 0,
        kind: platform_core::CellTriggerKind::Activate,
    };
    let scanned = platform_core::CellTriggerIntent {
        x: 2,
        y: 3,
        degree: 0,
        kind: platform_core::CellTriggerKind::Scanned,
    };
    let model = runner.engine.model().unwrap();

    let immediate = runner
        .messages_with_input_result(platform_core::NativeInputResult {
            events: vec![
                platform_core::MusicalEvent::NoteOn {
                    channel: 0,
                    note: 60,
                    velocity: 80,
                    duration_ms: Some(150),
                },
                platform_core::MusicalEvent::NoteOn {
                    channel: 0,
                    note: 60,
                    velocity: 96,
                    duration_ms: Some(150),
                },
            ],
            emitted_events: vec![],
            mapped_intents: vec![activate.clone(), scanned.clone()],
            event_intents: vec![Some(activate), Some(scanned)],
            model,
        })
        .unwrap();

    assert_eq!(musical_note_ons(&immediate), vec![(0, 60)]);

    let delayed = runner
        .send(HostMessage::TransportPulseStep {
            pulses: runner.algorithm_step_pulses,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();

    assert_eq!(musical_note_ons(&delayed), vec![(0, 60)]);
}

#[test]
pub(crate) fn delayed_link_queue_clears_on_transport_reset() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "keys".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.transport = RuntimeTransportState::Playing;
    runner.pulses_layers[0].activate_timing.delay_steps = 1;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 2, "y": 3 }),
            request_snapshot: None,
        })
        .unwrap();
    runner.reset_transport_position();

    let tick = runner
        .send(HostMessage::TransportPulseStep {
            pulses: runner.algorithm_step_pulses,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();

    assert!(musical_note_ons(&tick).is_empty());
}
