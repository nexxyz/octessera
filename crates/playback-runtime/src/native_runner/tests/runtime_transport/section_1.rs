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
pub(crate) fn fn_encoder_turn_positive_single_steps_while_staying_paused() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.transport = RuntimeTransportState::Paused;
    runner.ui.fn_held = true;
    let before_tick = runner.tick;

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();

    assert_eq!(runner.transport, RuntimeTransportState::Paused);
    assert_eq!(runner.tick, before_tick + 1);
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
    runner.transport = RuntimeTransportState::Paused;
    runner.input_events_while_paused = true;
    runner.pulses_layers[0].activate_timing.delay_steps = 1;
    let queued = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 2, "y": 3 }),
            request_snapshot: None,
        })
        .unwrap();
    assert!(musical_note_ons(&queued).is_empty());
    runner.ui.fn_held = true;

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
    runner.transport = RuntimeTransportState::Paused;
    runner.ui.fn_held = true;
    let before_tick = runner.tick;
    let before_path = runner.menu.current_focus_path();

    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": -1, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();

    assert_eq!(runner.tick, before_tick);
    assert_eq!(runner.menu.current_focus_path(), before_path);
}

#[test]
pub(crate) fn fn_encoder_turn_asks_to_pause_first_while_playing() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.transport = RuntimeTransportState::Playing;
    runner.ui.fn_held = true;
    let before_tick = runner.tick;

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();

    assert_eq!(runner.transport, RuntimeTransportState::Playing);
    assert_eq!(runner.tick, before_tick);
    assert_eq!(snapshot_from(&messages)["display"]["toast"], "Pause first");
    assert!(messages
        .iter()
        .all(|message| !matches!(message, RunnerMessage::PlatformEffects { effects } if effects.iter().any(|effect| matches!(effect, RuntimePlatformEffect::MidiPanic)))));
}

#[test]
pub(crate) fn fn_play_reset_stops_before_sample_preview() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.transport = RuntimeTransportState::Playing;
    runner.tick = 4;
    runner.ui.fn_held = true;

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_s", "pressed": true }),
            request_snapshot: None,
        })
        .unwrap();

    assert_eq!(runner.transport, RuntimeTransportState::Stopped);
    assert_eq!(runner.tick, 0);
    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects.iter().any(|effect| matches!(effect, RuntimePlatformEffect::MidiPanic))
    )));
}

#[test]
pub(crate) fn combined_modifier_play_is_reserved_no_op() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.transport = RuntimeTransportState::Paused;
    runner.ui.combined_modifier_held = true;
    let before_tick = runner.tick;

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_s", "pressed": true }),
            request_snapshot: None,
        })
        .unwrap();

    assert_eq!(runner.transport, RuntimeTransportState::Paused);
    assert_eq!(runner.tick, before_tick);
    assert!(messages.iter().all(|message| !matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects.iter().any(|effect| matches!(effect, RuntimePlatformEffect::MidiPanic))
    )));
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
