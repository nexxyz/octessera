use super::*;

#[test]
pub(crate) fn snapshot_settings_include_complete_audio_config_shapes() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].sample_base_velocity = 72;
    runner.instruments[0].sample_amp_env = json!({ "attackMs": 11 });
    runner.instruments[0].sample_filter = json!({ "type": "highpass", "cutoffHz": 1200 });
    runner.instruments[0].sample_filter_env = json!({ "releaseMs": 222 });
    runner.fx_buses[0].slot1_type = "delay".into();
    runner.fx_buses[0].slot1_params = json!({ "timeMs": 333, "feedback": 0.42, "mixPct": 44 });
    runner.global_fx_slots[0] = "distortion".into();
    runner.global_fx_params[0] = json!({ "drive": 3.5, "clip": 0.75, "mixPct": 88 });

    let snapshot = runner.snapshot().unwrap();

    assert_eq!(
        snapshot["settings"]["instruments"][0]["sample"]["baseVelocity"],
        72
    );
    assert_eq!(
        snapshot["settings"]["instruments"][0]["sample"]["ampEnv"]["attackMs"],
        11
    );
    assert_eq!(
        snapshot["settings"]["instruments"][0]["sample"]["filter"]["type"],
        "highpass"
    );
    assert_eq!(
        snapshot["settings"]["instruments"][0]["sample"]["filterEnv"]["releaseMs"],
        222
    );
    assert_eq!(
        snapshot["settings"]["mixer"]["buses"][0]["slot1"]["params"]["feedback"],
        0.42
    );
    assert_eq!(
        snapshot["settings"]["mixer"]["master"]["slots"][0]["params"]["clip"],
        0.75
    );
}

#[test]
pub(crate) fn first_snapshot_emits_full_audio_config_command() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.ui.master_volume = 64;

    let messages = runner.messages_with_snapshot().unwrap();

    let command = messages
        .iter()
        .find_map(|message| match message {
            RunnerMessage::AudioCommands { commands } => commands.iter().find_map(|command| {
                if let RuntimeAudioCommand::SetAudioConfig { revision, config } = command {
                    Some((*revision, config))
                } else {
                    None
                }
            }),
            _ => None,
        })
        .expect("full audio config command");
    assert_eq!(command.0, runner.audio_config_revision);
    assert_eq!(command.1["masterVolume"], 64);
    assert_eq!(command.1["voiceStealingMode"], "auto-balanced");
    assert!(command.1["instruments"].is_array());
}

#[test]
pub(crate) fn device_input_json_defaults_to_snapshot_response() {
    let message: HostMessage = serde_json::from_value(json!({
        "type": "device_input",
        "input": { "type": "button_s", "pressed": true }
    }))
    .unwrap();
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();

    let messages = runner.send(message).unwrap();

    assert!(messages
        .iter()
        .any(|message| matches!(message, RunnerMessage::Snapshot { .. })));
}

#[test]
pub(crate) fn device_input_can_skip_snapshot_response() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_s", "pressed": true }),
            request_snapshot: Some(false),
        })
        .unwrap();

    assert!(!messages
        .iter()
        .any(|message| matches!(message, RunnerMessage::Snapshot { .. })));
    assert!(messages
        .iter()
        .any(|message| matches!(message, RunnerMessage::RuntimeStatus { .. })));
    assert!(messages
        .iter()
        .any(|message| matches!(message, RunnerMessage::AudioCommands { .. })));
}

#[test]
pub(crate) fn voice_stealing_mode_change_emits_full_audio_config_command() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let _ = runner.messages_with_snapshot().unwrap();
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["sound"]["voiceStealingMode"] = json!("auto-hard");

    runner.apply_config_payload(payload).unwrap();
    let messages = runner.messages_with_snapshot().unwrap();

    let config = messages
        .iter()
        .find_map(|message| match message {
            RunnerMessage::AudioCommands { commands } => commands.iter().find_map(|command| {
                if let RuntimeAudioCommand::SetAudioConfig { config, .. } = command {
                    Some(config)
                } else {
                    None
                }
            }),
            _ => None,
        })
        .expect("full audio config command");
    assert_eq!(config["voiceStealingMode"], "auto-hard");
}

#[test]
pub(crate) fn runtime_snapshot_serializes_menu_scroll_metadata() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let _ = runner.menu.press();
    runner.menu.state.cursor = 7;

    let snapshot = runner.snapshot().unwrap();

    assert_eq!(snapshot["display"]["scrollOffset"], 1);
    assert_eq!(snapshot["display"]["totalRows"], 8);
    assert_eq!(snapshot["display"]["visibleRows"], 7);
}

#[test]
pub(crate) fn selected_long_menu_row_scrolls_once_slowly_only_while_highlighted() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.state.stack = vec![1];
    runner.menu.state.cursor = 3;

    runner.menu_scroll_offset = 0;
    let initial = runner.snapshot().unwrap();
    assert_eq!(initial["display"]["lines"][3], "> Events when ... On");

    runner.menu_scroll_offset = 3;
    let still_waiting = runner.snapshot().unwrap();
    assert_eq!(still_waiting["display"]["lines"][3], "> Events when ... On");

    runner.menu_scroll_offset = 4;
    let scrolled = runner.snapshot().unwrap();
    assert_ne!(scrolled["display"]["lines"][3], "> Events when ... On");

    runner.menu_scroll_offset = 27;
    let end_hold = runner.snapshot().unwrap();
    assert_ne!(end_hold["display"]["lines"][3], "> Events when ... On");

    runner.menu_scroll_offset = 99;
    let finished = runner.snapshot().unwrap();
    assert_eq!(finished["display"]["lines"][3], "> Events when ... On");

    runner.menu_scroll_offset = 4;
    runner.menu.state.editing = true;
    let editing = runner.snapshot().unwrap();
    assert_eq!(editing["display"]["lines"][3], "> Events when paused:");
}

#[test]
pub(crate) fn menu_navigation_resets_selected_row_scroll() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu_scroll_offset = 12;
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(runner.menu_scroll_offset, 1);
}

#[test]
pub(crate) fn unbound_aux_inputs_show_toast_without_navigating_menu() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.aux_auto_map_enabled = false;
    let original_stack = runner.menu.state.stack.clone();
    let original_cursor = runner.menu.state.cursor;

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "aux1" }),
            request_snapshot: None,
        })
        .unwrap();
    let snapshot = runner.snapshot().unwrap();

    assert_eq!(runner.menu.state.stack, original_stack);
    assert_eq!(runner.menu.state.cursor, original_cursor);
    assert_eq!(snapshot["display"]["toast"], "Turn-1: No binding");
}

#[test]
pub(crate) fn toasts_expire_after_timeout() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.oled_mode = NativeOledMode::Normal;
    runner.oled_splash_text.clear();
    runner.oled_splash_until = None;
    runner.set_toast_for_test("Temporary toast");

    assert_eq!(
        runner.snapshot().unwrap()["display"]["toast"],
        "Temporary toast"
    );

    runner.age_toast_state_for_test(2500);
    let _ = runner.messages_with_snapshot().unwrap();

    assert_eq!(runner.snapshot().unwrap()["display"]["toast"], "");
}

#[test]
pub(crate) fn aux_turn_toast_cooldown_keeps_first_then_shows_latest() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.state.stack = vec![2, 0, 0, 2, 3];
    runner.menu.state.cursor = 1;

    let first = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "id": "aux1", "delta": 1 }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(
        snapshot_from(&first)["display"]["toast"],
        "Turn-1: Cutoff: 223"
    );

    let second = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "id": "aux1", "delta": 1 }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(
        snapshot_from(&second)["display"]["toast"],
        "Turn-1: Cutoff: 223"
    );

    let third = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "id": "aux1", "delta": 1 }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(
        snapshot_from(&third)["display"]["toast"],
        "Turn-1: Cutoff: 223"
    );

    runner.age_toast_state_for_test(600);
    let after = runner.messages_with_snapshot().unwrap();

    assert_eq!(
        snapshot_from(&after)["display"]["toast"],
        "Turn-1: Cutoff: 225"
    );
}

#[test]
pub(crate) fn scanning_sequencer_pattern_emits_different_rows_over_scan_steps() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "sequencer".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.transport = RuntimeTransportState::Playing;
    runner.algorithm_step_pulses = 24;
    runner.pulses_layers[0].scan_mode = "scanning".into();
    runner.pulses_layers[0].scan_axis = "rows".into();
    runner.pulses_layers[0].scan_unit = "1/4".into();
    runner.pulses_layers[0].scanned_slot = 1;
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
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 0, "y": 1 }),
            request_snapshot: None,
        })
        .unwrap();

    let first = runner
        .send(HostMessage::TransportPulseStep {
            pulses: 24,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();
    let second = runner
        .send(HostMessage::TransportPulseStep {
            pulses: 24,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();

    let first_notes = musical_note_ons(&first);
    let second_notes = musical_note_ons(&second);
    assert!(first_notes.iter().any(|(channel, _)| *channel == 1));
    assert!(second_notes.iter().any(|(channel, _)| *channel == 1));
    assert_ne!(first_notes, second_notes);
}
