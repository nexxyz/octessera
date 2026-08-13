use super::*;
use std::time::{Duration, Instant};

#[test]
pub(crate) fn transport_and_event_indicators_appear_in_snapshot() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let display_start = Instant::now();
    runner.display.transients.set_test_now(display_start);

    let start = runner.send(HostMessage::MidiRealtimeStart).unwrap();
    let start_snapshot = snapshot_from(&start);
    assert_eq!(start_snapshot["transportIcon"], "play");
    assert_eq!(start_snapshot["transportFlash"], "measure");
    assert_eq!(
        start_snapshot["neoKeyLeds"],
        json!({
            "back": [221, 130, 205],
            "space": [99, 210, 63],
            "shift": [67, 68, 71],
            "fn": [67, 68, 71]
        })
    );
    assert_eq!(start_snapshot["cpuLoadRatio"], 0.0);

    let tick = runner
        .send(HostMessage::TransportPulseStep {
            pulses: 24,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(true),
        })
        .unwrap();
    let tick_snapshot = snapshot_from(&tick);
    assert_eq!(tick_snapshot["transportFlash"], "measure");
    assert_eq!(tick_snapshot["eventDotOn"], true);
    assert_eq!(tick_snapshot["neoKeyLeds"]["space"], json!([99, 210, 63]));

    runner
        .display
        .transients
        .set_test_now(display_start + Duration::from_millis(91));
    let beat = runner
        .send(HostMessage::TransportPulseStep {
            pulses: 24,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(true),
        })
        .unwrap();
    assert_eq!(
        snapshot_from(&beat)["neoKeyLeds"]["space"],
        json!([255, 212, 71])
    );

    runner.transport.transport = RuntimeTransportState::Paused;
    let paused_snapshot = runner.snapshot().unwrap();
    assert_eq!(paused_snapshot["transportIcon"], "pause");
    assert_eq!(
        paused_snapshot["neoKeyLeds"]["space"],
        json!([53, 207, 242])
    );

    runner.transport.transport = RuntimeTransportState::Stopped;
    let stopped_snapshot = runner.snapshot().unwrap();
    assert_eq!(stopped_snapshot["transportIcon"], "stop");
    assert_eq!(
        stopped_snapshot["neoKeyLeds"]["space"],
        json!([221, 130, 205])
    );
}

#[test]
pub(crate) fn modifier_led_snapshot_priority_is_native() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    assert_eq!(
        runner.snapshot().unwrap()["neoKeyLeds"]["shift"],
        json!([67, 68, 71])
    );

    runner.display.ui.shift_held = true;
    assert_eq!(
        runner.snapshot().unwrap()["neoKeyLeds"]["shift"],
        json!([255, 212, 71])
    );
    runner.display.ui.fn_held = true;
    assert_eq!(
        runner.snapshot().unwrap()["neoKeyLeds"]["fn"],
        json!([255, 212, 71])
    );

    runner.display.ui.combined_modifier_held = true;
    assert_eq!(
        runner.snapshot().unwrap()["neoKeyLeds"]["shift"],
        json!([53, 207, 242])
    );
}

#[test]
pub(crate) fn midi_device_event_precedes_snapshot_and_status() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "keys".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.instruments[0].kind = "midi".into();
    runner.instruments[0].midi_enabled = true;
    runner.transport.transport = RuntimeTransportState::Playing;
    runner.refresh_active_mapping_config();

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 2, "y": 3 }),
            request_snapshot: Some(false),
        })
        .unwrap();
    let midi_index = messages
        .iter()
        .position(|message| matches!(message, RunnerMessage::MidiEvents { .. }))
        .expect("midi device event should emit MIDI events");
    let snapshot_index = messages
        .iter()
        .position(|message| matches!(message, RunnerMessage::Snapshot { .. }))
        .expect("midi device event should emit a snapshot");
    let status_index = messages
        .iter()
        .position(|message| matches!(message, RunnerMessage::RuntimeStatus { .. }))
        .expect("midi device event should emit status");
    assert!(midi_index < snapshot_index);
    assert!(snapshot_index < status_index);
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
    runner.transport.layer_pulse_accumulators[0] = 5;
    runner.transport.tick = 7;
    runner.transport.current_ppqn_pulse = 42;

    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_s", "pressed": true }),
            request_snapshot: None,
        })
        .unwrap();

    assert_eq!(runner.transport.transport, RuntimeTransportState::Playing);
    assert_eq!(runner.transport.tick, 0);
    assert_eq!(runner.transport.current_ppqn_pulse, 0);
    assert_eq!(runner.transport.layer_pulse_accumulators[0], 0);
}

#[test]
pub(crate) fn scanning_sequencer_emits_scanned_notes_with_state_notes_disabled() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "sequencer".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.transport.transport = RuntimeTransportState::Playing;
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

    runner.transport.transport = RuntimeTransportState::Playing;
    let _ = runner
        .send(HostMessage::TransportPulseStep {
            pulses: 3,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();
    assert!(runner.transport.layer_pulse_accumulators[0] > 0);

    runner.display.ui.shift_held = true;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_s", "pressed": true }),
            request_snapshot: None,
        })
        .unwrap();
    runner.display.ui.shift_held = false;

    assert_eq!(runner.transport.transport, RuntimeTransportState::Stopped);
    assert_eq!(runner.transport.tick, 0);
    assert_eq!(runner.transport.layer_pulse_accumulators[0], 0);

    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_s", "pressed": true }),
            request_snapshot: None,
        })
        .unwrap();

    assert_eq!(runner.transport.transport, RuntimeTransportState::Playing);
    assert_eq!(runner.transport.tick, 0);
    assert_eq!(runner.transport.current_ppqn_pulse, 0);
    assert_eq!(runner.transport.layer_pulse_accumulators[0], 0);
}

#[test]
pub(crate) fn fn_encoder_turn_positive_single_steps_while_staying_paused() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.transport.transport = RuntimeTransportState::Paused;
    runner.display.ui.fn_held = true;
    let before_tick = runner.transport.tick;

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();

    assert_eq!(runner.transport.transport, RuntimeTransportState::Paused);
    assert_eq!(runner.transport.tick, before_tick + 1);
    assert!(messages
        .iter()
        .all(|message| !matches!(message, RunnerMessage::PlatformEffects { effects } if effects.iter().any(|effect| matches!(effect, RuntimePlatformEffect::MidiPanic)))));
    let snapshot = snapshot_from(&messages);
    assert_eq!(snapshot["transportIcon"], "pause");
}

#[test]
pub(crate) fn fn_encoder_single_step_matures_delayed_link_queue() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "keys".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.transport.transport = RuntimeTransportState::Paused;
    runner.input_events_while_paused = true;
    runner.pulses_layers[0].activate_timing.delay_steps = 1;
    let queued = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 2, "y": 3 }),
            request_snapshot: None,
        })
        .unwrap();
    assert!(musical_note_ons(&queued).is_empty());
    runner.display.ui.fn_held = true;

    let stepped = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();

    assert_eq!(musical_note_ons(&stepped).len(), 1);
}

#[test]
pub(crate) fn fn_encoder_turn_negative_is_consumed_without_step_or_menu_turn() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.transport.transport = RuntimeTransportState::Paused;
    runner.display.ui.fn_held = true;
    let before_tick = runner.transport.tick;
    let before_path = runner.menu.current_focus_path();

    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": -1, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();

    assert_eq!(runner.transport.tick, before_tick);
    assert_eq!(runner.menu.current_focus_path(), before_path);
}

#[test]
pub(crate) fn fn_encoder_turn_asks_to_pause_first_while_playing() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.transport.transport = RuntimeTransportState::Playing;
    runner.display.ui.fn_held = true;
    let before_tick = runner.transport.tick;

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();

    assert_eq!(runner.transport.transport, RuntimeTransportState::Playing);
    assert_eq!(runner.transport.tick, before_tick);
    assert_eq!(snapshot_from(&messages)["display"]["toast"], "Pause first");
    assert!(messages
        .iter()
        .all(|message| !matches!(message, RunnerMessage::PlatformEffects { effects } if effects.iter().any(|effect| matches!(effect, RuntimePlatformEffect::MidiPanic)))));
}

#[test]
pub(crate) fn fn_play_reset_stops_before_sample_preview() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.transport.transport = RuntimeTransportState::Playing;
    runner.transport.tick = 4;
    runner.display.ui.fn_held = true;

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_s", "pressed": true }),
            request_snapshot: None,
        })
        .unwrap();

    assert_eq!(runner.transport.transport, RuntimeTransportState::Stopped);
    assert_eq!(runner.transport.tick, 0);
    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects.iter().any(|effect| matches!(effect, RuntimePlatformEffect::MidiPanic))
    )));
}

#[test]
pub(crate) fn combined_modifier_play_is_reserved_no_op() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.transport.transport = RuntimeTransportState::Paused;
    runner.display.ui.combined_modifier_held = true;
    let before_tick = runner.transport.tick;

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_s", "pressed": true }),
            request_snapshot: None,
        })
        .unwrap();

    assert_eq!(runner.transport.transport, RuntimeTransportState::Paused);
    assert_eq!(runner.transport.tick, before_tick);
    assert!(messages.iter().all(|message| !matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects.iter().any(|effect| matches!(effect, RuntimePlatformEffect::MidiPanic))
    )));
}

#[test]
pub(crate) fn stop_then_start_restarts_scanning_from_first_lane() {
    let mut runner = configured_scanning_sequencer_runner();
    runner.transport.transport = RuntimeTransportState::Playing;
    runner
        .send(HostMessage::TransportPulseStep {
            pulses: 6,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();

    runner.display.ui.shift_held = true;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_s", "pressed": true }),
            request_snapshot: None,
        })
        .unwrap();
    runner.display.ui.shift_held = false;
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
