use super::*;

#[test]
fn fast_path_audio_param_still_applies_immediately() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    assert!(runner
        .menu
        .focus_item_key("instruments.0.synth.amp.gainPct"));
    runner.menu.state.editing = true;

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": -10, "id": "main" }),
        })
        .unwrap();

    assert_eq!(runner.instruments[0].synth_gain_pct, 70);
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["instruments"][0]["synth"]["amp"]["gainPct"],
        70
    );
    assert_eq!(runner.audio_config_revision, 0);
    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::AudioCommands { commands }
            if commands.iter().any(|command| matches!(
                command,
                RuntimeAudioCommand::SetSynthParam { instrument_slot: 0, path, value }
                    if path == "synth.amp.gainPct" && (*value - 70.0).abs() < f32::EPSILON
            ))
    )));
}

#[test]
fn sampler_fast_path_uses_direct_audio_command_without_revision_bump() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].kind = "sampler".into();
    runner.instruments[0].name = "sampler".into();
    runner.menu.rebuild(runner.menu_config());
    assert!(runner.menu.focus_item_key("instruments.0.sample.tuneSemis"));
    runner.menu.state.editing = true;

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 7, "id": "main" }),
        })
        .unwrap();

    assert_eq!(runner.instruments[0].sample_tune_semis, 7);
    assert_eq!(runner.audio_config_revision, 0);
    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::AudioCommands { commands }
            if commands.iter().any(|command| matches!(
                command,
                RuntimeAudioCommand::SetSampleBankParam { instrument_slot: 0, path, value }
                    if path == "sample.tuneSemis" && (*value - 7.0).abs() < f32::EPSILON
            ))
    )));
}

#[test]
fn fx_param_fast_path_uses_direct_audio_command_without_revision_bump() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.turn_key("mixer.buses.0.slot1.type", 1);
    runner.apply_menu_state().unwrap();
    runner.audio_config_revision = 0;
    assert!(runner
        .menu
        .focus_item_key("mixer.buses.0.slot1.params.depthPct"));
    runner.menu.state.editing = true;

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
        })
        .unwrap();

    assert_eq!(runner.fx_buses[0].slot1_params["depthPct"], 61);
    assert_eq!(runner.audio_config_revision, 0);
    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::AudioCommands { commands }
            if commands.iter().any(|command| matches!(
                command,
                RuntimeAudioCommand::SetFxBusSlot { bus_index: 0, slot_index: 0, fx_type, params }
                    if fx_type == "tremolo" && params.get("depthPct") == Some(&json!(61))
            ))
    )));
}

#[test]
fn fx_param_fast_path_preserves_scaled_values() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.turn_key("mixer.buses.0.slot1.type", 1);
    runner.apply_menu_state().unwrap();
    runner.audio_config_revision = 0;
    assert!(runner
        .menu
        .focus_item_key("mixer.buses.0.slot1.params.rateHz"));
    runner.menu.state.editing = true;

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
        })
        .unwrap();

    assert_eq!(runner.fx_buses[0].slot1_params["rateHz"], 4.05);
    assert_eq!(runner.audio_config_revision, 0);
    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::AudioCommands { commands }
            if commands.iter().any(|command| matches!(
                command,
                RuntimeAudioCommand::SetFxBusSlot { bus_index: 0, slot_index: 0, fx_type, params }
                    if fx_type == "tremolo" && params.get("rateHz") == Some(&json!(4.05))
            ))
    )));
}

#[test]
fn fx_bus_pan_uses_direct_audio_command_without_revision_bump() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let _ = runner.messages_with_snapshot().unwrap();
    assert!(runner.menu.focus_item_key("mixer.buses.0.panPos"));
    runner.menu.state.editing = true;

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": -3, "id": "main" }),
        })
        .unwrap();

    assert_eq!(runner.fx_buses[0].pan_pos, 13);
    assert_eq!(runner.audio_config_revision, 0);
    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::AudioCommands { commands }
            if commands.iter().any(|command| matches!(
                command,
                RuntimeAudioCommand::SetFxBusMixer { bus_index: 0, pan_pos: Some(13) }
            ))
    )));
    assert!(!messages.iter().any(|message| matches!(
        message,
        RunnerMessage::AudioCommands { commands }
            if commands.iter().any(|command| matches!(command, RuntimeAudioCommand::SetAudioConfig { .. }))
    )));
}

#[test]
fn direct_dynamic_param_apply_does_not_bump_full_audio_config_revision() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let _ = runner.messages_with_snapshot().unwrap();
    assert!(runner
        .menu
        .focus_item_key("instruments.0.synth.amp.gainPct"));
    runner.menu.turn_key("instruments.0.synth.amp.gainPct", -10);

    runner.apply_menu_state().unwrap();
    let messages = runner.messages_with_snapshot().unwrap();

    assert_eq!(runner.instruments[0].synth_gain_pct, 70);
    assert_eq!(runner.audio_config_revision, 0);
    assert!(!messages.iter().any(|message| matches!(
        message,
        RunnerMessage::AudioCommands { commands }
            if commands.iter().any(|command| matches!(command, RuntimeAudioCommand::SetAudioConfig { .. }))
    )));
}

#[test]
fn unsupported_synth_param_apply_still_bumps_full_audio_config_revision() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let _ = runner.messages_with_snapshot().unwrap();
    assert!(runner
        .menu
        .focus_item_key("instruments.0.synth.osc1.levelPct"));
    runner
        .menu
        .turn_key("instruments.0.synth.osc1.levelPct", -10);

    runner.apply_menu_state().unwrap();
    let messages = runner.messages_with_snapshot().unwrap();

    assert_eq!(runner.audio_config_revision, 1);
    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::AudioCommands { commands }
            if commands.iter().any(|command| matches!(
                command,
                RuntimeAudioCommand::SetAudioConfig { revision: 1, .. }
            ))
    )));
}

#[test]
fn unsupported_sampler_param_apply_still_bumps_full_audio_config_revision() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].kind = "sampler".into();
    runner.instruments[0].name = "sampler".into();
    runner.menu.rebuild(runner.menu_config());
    let _ = runner.messages_with_snapshot().unwrap();
    assert!(runner
        .menu
        .focus_item_key("instruments.0.sample.filter.cutoffHz"));
    runner
        .menu
        .turn_key("instruments.0.sample.filter.cutoffHz", -10);

    runner.apply_menu_state().unwrap();
    let messages = runner.messages_with_snapshot().unwrap();

    assert_eq!(runner.audio_config_revision, 1);
    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::AudioCommands { commands }
            if commands.iter().any(|command| matches!(
                command,
                RuntimeAudioCommand::SetAudioConfig { revision: 1, .. }
            ))
    )));
}

#[test]
fn direct_topology_apply_bumps_full_audio_config_revision() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let _ = runner.messages_with_snapshot().unwrap();
    assert!(runner.menu.focus_item_key("instruments.0.type"));
    runner.menu.turn_key("instruments.0.type", 1);

    runner.apply_menu_state().unwrap();
    let messages = runner.messages_with_snapshot().unwrap();

    assert_eq!(runner.instruments[0].kind, "sampler");
    assert_eq!(runner.audio_config_revision, 1);
    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::AudioCommands { commands }
            if commands.iter().any(|command| matches!(
                command,
                RuntimeAudioCommand::SetAudioConfig { revision: 1, .. }
            ))
    )));
}
