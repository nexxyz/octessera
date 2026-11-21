use super::*;

fn pattern_phase(runner: &NativeRunner, layer_index: usize) -> u64 {
    let state = if layer_index == runner.active_layer_index {
        runner.active_engine().state()
    } else {
        runner
            .layers
            .get(layer_index)
            .map(|layer| &layer.engine)
            .expect("inactive layer engine")
            .state()
    };
    match state {
        platform_core::NativeBehaviorState::Pattern(state) => state.phase,
        _ => panic!("expected pattern state"),
    }
}

fn looper_step(runner: &NativeRunner) -> usize {
    match runner.active_engine().state() {
        platform_core::NativeBehaviorState::Looper(state) => state.step_index,
        _ => panic!("expected looper state"),
    }
}

fn pattern_runner() -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "weave".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.test_set_midi_enabled(true);
    runner.select_layer_behavior(1, "weave").unwrap();
    runner
}

#[test]
fn transport_stop_resets_active_looper_phase_and_preserves_recorded_sequence() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "looper".into(),
        behavior_config: json!({ "mode": "overdub", "lengthSteps": 2 }),
        note_behaviors: vec![platform_core::NoteBehavior::Hold; 16],
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.test_set_midi_enabled(true);

    runner.send(HostMessage::MidiRealtimeStart).unwrap();
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 2, "y": 3 }),
            request_snapshot: None,
        })
        .unwrap();
    runner
        .send(HostMessage::TransportPulseStep {
            pulses: 6,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: None,
        })
        .unwrap();
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_release", "x": 2, "y": 3 }),
            request_snapshot: None,
        })
        .unwrap();

    let recorded_steps = runner.active_engine().serialized_state().unwrap()["steps"].clone();
    assert_ne!(looper_step(&runner), 0);
    assert!(recorded_steps.as_array().is_some_and(|steps| {
        steps
            .iter()
            .any(|step| !step.as_array().unwrap().is_empty())
    }));

    runner.send(HostMessage::TransportStop).unwrap();

    assert_eq!(looper_step(&runner), 0);
    assert_eq!(
        runner.active_engine().serialized_state().unwrap()["steps"],
        recorded_steps
    );
}

#[test]
fn transport_stop_resets_active_and_inactive_pattern_phases_but_pause_continue_preserves_them() {
    let mut runner = pattern_runner();
    runner.send(HostMessage::MidiRealtimeStart).unwrap();
    runner
        .send(HostMessage::TransportPulseStep {
            pulses: 12,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: None,
        })
        .unwrap();

    let active_phase = pattern_phase(&runner, 0);
    let inactive_phase = pattern_phase(&runner, 1);
    assert!(active_phase > 0);
    assert!(inactive_phase > 0);

    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_s", "pressed": true }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(runner.transport.transport, RuntimeTransportState::Paused);
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_s", "pressed": true }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(runner.transport.transport, RuntimeTransportState::Playing);
    assert_eq!(pattern_phase(&runner, 0), active_phase);
    assert_eq!(pattern_phase(&runner, 1), inactive_phase);

    runner.send(HostMessage::TransportStop).unwrap();

    assert_eq!(pattern_phase(&runner, 0), 0);
    assert_eq!(pattern_phase(&runner, 1), 0);
}

#[test]
fn external_resync_boundary_resets_active_and_inactive_pattern_phases() {
    let mut runner = pattern_runner();
    runner.test_set_midi_enabled(true);
    runner.send(HostMessage::MidiRealtimeStart).unwrap();
    runner
        .send(HostMessage::TransportPulseStep {
            pulses: 12,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: None,
        })
        .unwrap();
    assert!(pattern_phase(&runner, 0) > 0);
    assert!(pattern_phase(&runner, 1) > 0);

    runner.transport.sync_source = SyncSource::External;
    runner.midi.clock_in_enabled = true;
    runner.transport.current_ppqn_pulse = 95;
    runner.transport.pending_resync = true;
    runner
        .send(HostMessage::MidiRealtimeClock { pulses: 1 })
        .unwrap();

    assert_eq!(pattern_phase(&runner, 0), 0);
    assert_eq!(pattern_phase(&runner, 1), 0);
}

fn pulse(runner: &mut NativeRunner, pulses: u32) {
    runner
        .send(HostMessage::TransportPulseStep {
            pulses,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: None,
        })
        .unwrap();
}

#[test]
fn a_pattern_keeps_its_place_in_the_cycle_when_a_setting_turns() {
    let mut runner = pattern_runner();
    runner.send(HostMessage::MidiRealtimeStart).unwrap();
    pulse(&mut runner, 48);
    let phase = pattern_phase(&runner, 0);
    assert!(phase > 2, "{phase}");

    runner
        .edit_menu_key("layers.0.build.behaviorConfig.densityPct", 1)
        .unwrap();

    assert_eq!(pattern_phase(&runner, 0), phase);
    assert!(runner.layer_behavior_config(0).get("phase").is_none());
}

#[test]
fn a_rate_resync_restarts_the_pattern_cycle_on_the_bar() {
    let mut runner = pattern_runner();
    runner.send(HostMessage::MidiRealtimeStart).unwrap();
    pulse(&mut runner, 48);
    runner.play_live.rate_resyncs[1] = true;
    pulse(&mut runner, 48);

    assert!(pattern_phase(&runner, 0) > 2);
    assert!(
        pattern_phase(&runner, 1) <= 1,
        "{}",
        pattern_phase(&runner, 1)
    );
}
