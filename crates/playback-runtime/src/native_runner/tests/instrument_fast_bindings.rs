use super::*;

fn input(runner: &mut NativeRunner, input: Value) -> Vec<RunnerMessage> {
    runner
        .send(HostMessage::DeviceInput {
            input,
            request_snapshot: None,
        })
        .unwrap()
}

fn assert_only_scalar(messages: &[RunnerMessage], slot: usize, path: &str) {
    let commands = messages
        .iter()
        .flat_map(|message| match message {
            RunnerMessage::AudioCommands { commands } => commands.as_slice(),
            _ => &[],
        })
        .collect::<Vec<_>>();
    assert_eq!(commands.len(), 1, "{path}: {commands:?}");
    assert!(
        matches!(commands[0],
            RuntimeAudioCommand::SetSynthParam { instrument_slot, path: actual, .. }
            | RuntimeAudioCommand::SetFmParam { instrument_slot, path: actual, .. }
            | RuntimeAudioCommand::SetSampleBankParam { instrument_slot, path: actual, .. }
            if *instrument_slot == slot && actual == path
        ),
        "{commands:?}"
    );
}

fn numeric_binding(key: &str, min: f64, max: f64) -> NativeParamBinding {
    NativeParamBinding {
        key: key.into(),
        label: Some(key.into()),
        kind: "number".into(),
        min: Some(min),
        max: Some(max),
        step: Some(1.0),
        user_min: None,
        user_max: None,
        options: vec![],
        invert: false,
    }
}

#[test]
fn fm_real_picker_aux_and_xy_device_inputs_only_emit_targeted_scalars() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].kind = "fm".into();
    runner.instruments[1].kind = "synth".into();
    runner.menu.rebuild(runner.menu_config());
    let _ = runner.messages_with_snapshot().unwrap();
    assert!(runner
        .menu
        .focus_item_key("aux:0:turn.instruments.0.fm.index"));
    input(
        &mut runner,
        json!({ "type": "encoder_press", "id": "main" }),
    );
    assert_eq!(
        runner.aux_bindings[0]
            .as_ref()
            .and_then(|binding| binding.turn_key.as_deref()),
        Some("instruments.0.fm.index")
    );
    let untouched = runner.instruments[1].clone();
    runner.transport.transport = RuntimeTransportState::Playing;
    let messages = input(
        &mut runner,
        json!({ "type": "encoder_turn", "id": "aux1", "delta": 1 }),
    );
    assert_eq!(runner.instruments[0].fm_config["index"], 51);
    assert_only_scalar(&messages, 0, "fm.index");
    assert_eq!(runner.instruments[1], untouched);

    runner.active_play_mode = "xy".into();
    runner.xy_smoothing_ms = 0;
    runner.menu.rebuild(runner.menu_config());
    assert!(runner
        .menu
        .focus_item_key("xy:x.instruments.0.fm.filter.cutoffHz"));
    input(
        &mut runner,
        json!({ "type": "encoder_press", "id": "main" }),
    );
    assert_eq!(
        runner.xy_x_binding.as_ref().unwrap().key,
        "instruments.0.fm.filter.cutoffHz"
    );
    let _ = runner.messages_with_snapshot().unwrap();
    let messages = input(&mut runner, json!({ "type": "grid_press", "x": 7, "y": 0 }));
    assert_only_scalar(&messages, 0, "fm.filter.cutoffHz");
    assert_eq!(runner.instruments[1], untouched);
    let saved = runner.config_payload();
    let mut restored = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    restored.apply_config_payload(saved).unwrap();
    assert_eq!(
        restored.xy_x_binding.unwrap().key,
        "instruments.0.fm.filter.cutoffHz"
    );
    assert_eq!(
        restored.aux_bindings[0]
            .as_ref()
            .unwrap()
            .turn_key
            .as_deref(),
        Some("instruments.0.fm.index")
    );
}

#[test]
fn synth_and_sample_sustained_device_edits_never_replace_instrument_slots() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[1].kind = "synth".into();
    runner.menu.rebuild(runner.menu_config());
    runner.transport.transport = RuntimeTransportState::Playing;
    let _ = runner.messages_with_snapshot().unwrap();
    runner.aux_bindings[0] = Some(NativeAuxBinding {
        turn_key: Some("instruments.0.synth.osc1.levelPct".into()),
        press_action: None,
    });
    let second = runner.instruments[1].clone();
    let messages = input(
        &mut runner,
        json!({ "type": "encoder_turn", "id": "aux1", "delta": -1 }),
    );
    assert_only_scalar(&messages, 0, "synth.osc1.levelPct");
    assert_eq!(runner.instruments[1], second);

    runner.active_play_mode = "xy".into();
    runner.xy_smoothing_ms = 0;
    runner.xy_x_binding = Some(numeric_binding(
        "instruments.0.synth.filter.cutoffHz",
        0.0,
        255.0,
    ));
    let messages = input(&mut runner, json!({ "type": "grid_press", "x": 7, "y": 0 }));
    assert_only_scalar(&messages, 0, "synth.filter.cutoffHz");
    assert_eq!(runner.instruments[1], second);

    let mut sample_config = runner.config_payload();
    sample_config["runtimeConfig"]["instruments"][0]["type"] = json!("sampler");
    runner.apply_config_payload(sample_config).unwrap();
    runner.active_play_mode = "xy".into();
    runner.xy_smoothing_ms = 0;
    runner.xy_x_binding = Some(numeric_binding(
        "instruments.0.sample.filter.cutoffHz",
        0.0,
        255.0,
    ));
    runner.clear_all_modulation_sources();
    let _ = runner.messages_with_snapshot().unwrap();
    let messages = input(&mut runner, json!({ "type": "grid_press", "x": 6, "y": 0 }));
    assert_only_scalar(&messages, 0, "sample.filter.cutoffHz");
    runner.aux_bindings[0] = Some(NativeAuxBinding {
        turn_key: Some("instruments.0.sample.amp.gainPct".into()),
        press_action: None,
    });
    runner.menu.rebuild(runner.menu_config());
    let messages = input(
        &mut runner,
        json!({ "type": "encoder_turn", "id": "aux1", "delta": -1 }),
    );
    assert_only_scalar(&messages, 0, "sample.amp.gainPct");
    assert_eq!(runner.instruments[1], second);
}

#[test]
fn held_fm_xy_menu_edit_rebases_without_double_command_then_release_restores_new_base() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].kind = "fm".into();
    runner.menu.rebuild(runner.menu_config());
    runner.active_play_mode = "xy".into();
    runner.xy_smoothing_ms = 0;
    runner.xy_x_binding = Some(numeric_binding("instruments.0.fm.index", 0.0, 100.0));
    let _ = runner.messages_with_snapshot().unwrap();
    let messages = input(&mut runner, json!({ "type": "grid_press", "x": 3, "y": 0 }));
    assert_only_scalar(&messages, 0, "fm.index");
    assert!(runner.menu.focus_item_key("instruments.0.fm.index"));
    runner.menu.state.editing = true;
    let messages = input(
        &mut runner,
        json!({ "type": "encoder_turn", "id": "main", "delta": 1 }),
    );
    assert!(!messages
        .iter()
        .any(|message| matches!(message, RunnerMessage::AudioCommands { .. })));
    let new_base = runner
        .menu
        .number_for_key("instruments.0.fm.index")
        .unwrap();
    runner.set_param_binding_target("xy:x", None);
    let commands = runner.outbox.drain_audio_commands();
    assert!(
        commands.iter().any(|command| matches!(command,
            RuntimeAudioCommand::SetFmParam { instrument_slot: 0, path, value, .. }
            if path == "fm.index" && *value == new_base as f32
        )),
        "{commands:?}"
    );
    assert_eq!(runner.instruments[0].fm_config["index"], new_base);
}

#[test]
fn synth_gain_xy_then_aux_edit_persists_composed_gain_without_replacing_slot() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.active_play_mode = "xy".into();
    runner.xy_smoothing_ms = 0;
    runner.xy_x_binding = Some(numeric_binding(
        "instruments.0.synth.amp.gainPct",
        0.0,
        100.0,
    ));
    runner.aux_bindings[0] = Some(NativeAuxBinding {
        turn_key: Some("instruments.0.synth.amp.gainPct".into()),
        press_action: None,
    });
    let _ = runner.messages_with_snapshot().unwrap();
    let first = input(&mut runner, json!({ "type": "grid_press", "x": 3, "y": 0 }));
    assert_only_scalar(&first, 0, "synth.amp.gainPct");
    let second = input(
        &mut runner,
        json!({ "type": "encoder_turn", "id": "aux1", "delta": -1 }),
    );
    assert!(!second.iter().any(|message| matches!(message,
        RunnerMessage::AudioCommands { commands } if commands.iter().any(|command| matches!(command,
            RuntimeAudioCommand::SetInstrumentSlot { .. } | RuntimeAudioCommand::SetAudioConfig { .. }
        ))
    )));
    let gain = runner.instruments[0].synth_gain_pct;
    assert_eq!(runner.instruments[0].synth_config["amp"]["gainPct"], gain);
    assert_eq!(
        runner.instrument_audio_config(0).unwrap()["synth"]["amp"]["gainPct"],
        gain
    );
    let saved = runner.config_payload();
    assert_eq!(
        saved["runtimeConfig"]["instruments"][0]["synth"]["amp"]["gainPct"],
        gain
    );
    let mut restored = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    restored.apply_config_payload(saved).unwrap();
    assert_eq!(restored.instruments[0].synth_gain_pct, gain);
    assert_eq!(restored.instruments[0].synth_config["amp"]["gainPct"], gain);
}
