use super::*;

#[test]
pub(crate) fn transport_and_event_indicators_appear_in_snapshot() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();

    let start = runner.send(HostMessage::MidiRealtimeStart).unwrap();
    assert!(start.iter().any(|message| matches!(
        message,
        RunnerMessage::UiPulse {
            pulse: RuntimeUiPulse::TransportFlash { flash, .. }
        } if flash == "measure"
    )));
    let start_snapshot = snapshot_from(&start);
    assert_eq!(start_snapshot["transportIcon"], "play");
    assert_eq!(start_snapshot["transportFlash"], "measure");
    assert_eq!(start_snapshot["cpuLoadRatio"], 0.0);

    let tick = runner
        .send(HostMessage::TransportPulseStep {
            pulses: 24,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(true),
        })
        .unwrap();
    assert!(tick.iter().any(|message| matches!(
        message,
        RunnerMessage::UiPulse {
            pulse: RuntimeUiPulse::TriggerPulse { .. }
        }
    )));
    assert!(tick.iter().any(|message| matches!(
        message,
        RunnerMessage::UiPulse {
            pulse: RuntimeUiPulse::TransportFlash { flash, .. }
        } if flash == "beat"
    )));
    let tick_snapshot = snapshot_from(&tick);
    assert_eq!(tick_snapshot["transportFlash"], "beat");
    assert_eq!(tick_snapshot["eventDotOn"], true);

    runner.transport = RuntimeTransportState::Paused;
    let paused_snapshot = runner.snapshot().unwrap();
    assert_eq!(paused_snapshot["transportIcon"], "pause");

    runner.transport = RuntimeTransportState::Stopped;
    let stopped_snapshot = runner.snapshot().unwrap();
    assert_eq!(stopped_snapshot["transportIcon"], "stop");
}

pub(crate) fn configured_scanning_sequencer_runner() -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "sequencer".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
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
    runner
}

#[test]
pub(crate) fn startup_playback_resets_scan_accumulators() {
    let mut runner = configured_scanning_sequencer_runner();
    runner.layer_pulse_accumulators[0] = 5;
    runner.tick = 7;
    runner.current_ppqn_pulse = 42;

    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_s", "pressed": true }),
            request_snapshot: None,
        })
        .unwrap();

    assert_eq!(runner.transport, RuntimeTransportState::Playing);
    assert_eq!(runner.tick, 0);
    assert_eq!(runner.current_ppqn_pulse, 0);
    assert_eq!(runner.layer_pulse_accumulators[0], 0);
}

#[test]
pub(crate) fn scanning_sequencer_emits_scanned_notes_with_state_notes_disabled() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "sequencer".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.transport = RuntimeTransportState::Playing;
    runner.pulses_layers[0].scan_mode = "scanning".into();
    runner.pulses_layers[0].scan_axis = "rows".into();
    runner.pulses_layers[0].scan_unit = "1/16".into();
    runner.pulses_layers[0].state_notes_enabled = false;
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

    let messages = runner
        .send(HostMessage::TransportPulseStep {
            pulses: 6,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();

    assert!(!musical_note_ons(&messages).is_empty());
}

#[test]
pub(crate) fn stop_then_start_restarts_scanning_from_zero_accumulator() {
    let mut runner = configured_scanning_sequencer_runner();

    runner.transport = RuntimeTransportState::Playing;
    let _ = runner
        .send(HostMessage::TransportPulseStep {
            pulses: 3,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();
    assert!(runner.layer_pulse_accumulators[0] > 0);

    runner.ui.shift_held = true;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_s", "pressed": true }),
            request_snapshot: None,
        })
        .unwrap();
    runner.ui.shift_held = false;

    assert_eq!(runner.transport, RuntimeTransportState::Stopped);
    assert_eq!(runner.tick, 0);
    assert_eq!(runner.layer_pulse_accumulators[0], 0);

    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_s", "pressed": true }),
            request_snapshot: None,
        })
        .unwrap();

    assert_eq!(runner.transport, RuntimeTransportState::Playing);
    assert_eq!(runner.tick, 0);
    assert_eq!(runner.current_ppqn_pulse, 0);
    assert_eq!(runner.layer_pulse_accumulators[0], 0);
}

#[test]
pub(crate) fn stop_then_start_restarts_scanning_from_first_lane() {
    let mut runner = configured_scanning_sequencer_runner();
    runner.transport = RuntimeTransportState::Playing;
    runner
        .send(HostMessage::TransportPulseStep {
            pulses: 6,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();

    runner.ui.shift_held = true;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_s", "pressed": true }),
            request_snapshot: None,
        })
        .unwrap();
    runner.ui.shift_held = false;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_s", "pressed": true }),
            request_snapshot: None,
        })
        .unwrap();

    let snapshot = runner.snapshot().unwrap();
    let cells = led_cells(&snapshot);
    let first_lane = cells[display_index(0, 0)].as_object().unwrap();
    let second_lane = cells[display_index(0, 1)].as_object().unwrap();

    assert!(first_lane["r"].as_i64().unwrap() > second_lane["r"].as_i64().unwrap());
}
