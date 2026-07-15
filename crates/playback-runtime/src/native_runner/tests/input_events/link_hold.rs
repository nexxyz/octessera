use super::*;

#[test]
pub(crate) fn delayed_hold_note_on_is_cancelled_by_release_before_due() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "keys".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.transport = RuntimeTransportState::Playing;
    assert!(runner.menu.focus_item_key("instruments.0.noteBehavior"));
    runner.menu.state.editing = true;
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(runner.instruments[0].note_behavior, "hold");
    runner.pulses_layers[0].activate_timing.delay_steps = 1;

    let press = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 2, "y": 3 }),
            request_snapshot: None,
        })
        .unwrap();
    assert!(musical_note_ons(&press).is_empty());

    let release = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_release", "x": 2, "y": 3 }),
            request_snapshot: None,
        })
        .unwrap();
    assert!(release.iter().any(|message| matches!(
        message,
        RunnerMessage::MusicalEvents { events }
            if events.iter().any(|event| matches!(event, platform_core::MusicalEvent::NoteOff { .. }))
    )));

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
pub(crate) fn hold_note_retrigger_is_cancelled_by_release_before_repeat() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "keys".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.transport = RuntimeTransportState::Playing;
    assert!(runner.menu.focus_item_key("instruments.0.noteBehavior"));
    runner.menu.state.editing = true;
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(runner.instruments[0].note_behavior, "hold");
    runner.pulses_layers[0].activate_timing.retrigger_count = 1;

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
    assert!(release.iter().any(|message| matches!(
        message,
        RunnerMessage::MusicalEvents { events }
            if events.iter().any(|event| matches!(event, platform_core::MusicalEvent::NoteOff { .. }))
    )));

    let repeat = runner
        .send(HostMessage::TransportPulseStep {
            pulses: runner.algorithm_step_pulses,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();

    assert!(musical_note_ons(&repeat).is_empty());
}

#[test]
pub(crate) fn held_link_note_ons_do_not_schedule_indefinite_retriggers() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "keys".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.transport = RuntimeTransportState::Playing;
    assert!(runner.menu.focus_item_key("instruments.0.noteBehavior"));
    runner.menu.state.editing = true;
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(runner.instruments[0].note_behavior, "hold");
    runner.pulses_layers[0].activate_timing.retrigger_count = 1;

    let press = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 2, "y": 3 }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(musical_note_ons(&press).len(), 1);

    let repeat = runner
        .send(HostMessage::TransportPulseStep {
            pulses: runner.algorithm_step_pulses,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();

    assert!(musical_note_ons(&repeat).is_empty());
}
