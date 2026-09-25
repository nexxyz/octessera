use super::*;

fn input(runner: &mut NativeRunner, value: Value) -> Vec<RunnerMessage> {
    runner
        .send(HostMessage::DeviceInput {
            input: value,
            request_snapshot: None,
        })
        .unwrap()
}

fn assert_scalar(messages: &[RunnerMessage], path: &str) {
    let commands = messages
        .iter()
        .flat_map(|message| match message {
            RunnerMessage::AudioCommands { commands } => commands.as_slice(),
            _ => &[],
        })
        .collect::<Vec<_>>();
    assert!(
        matches!(&commands[..], [RuntimeAudioCommand::SetPluckParam {
        instrument_slot: 0, path: actual, ..
    }] if actual == path),
        "{path}: {commands:?}"
    );
}

#[test]
fn pluck_menu_numeric_device_edits_emit_only_targeted_scalar() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].kind = "pluck".into();
    runner.menu.rebuild(runner.menu_config());
    let _ = runner.messages_with_snapshot().unwrap();
    for (field, delta) in [
        ("decayMs", 1),
        ("brightnessPct", 1),
        ("pickPositionPct", 1),
        ("amp.gainPct", -1),
        ("amp.velocitySensitivityPct", -1),
        ("ampEnv.releaseMs", 1),
        ("filter.cutoffHz", 1),
        ("filter.resonance", 1),
        ("filter.envAmountPct", 1),
        ("filter.keyTrackingPct", 1),
        ("filterEnv.attackMs", 1),
    ] {
        let key = format!("instruments.0.pluck.{field}");
        assert!(runner.menu.focus_item_key(&key), "{key}");
        runner.menu.state.editing = true;
        let messages = input(
            &mut runner,
            json!({ "type": "encoder_turn", "id": "main", "delta": delta }),
        );
        assert_scalar(&messages, &format!("pluck.{field}"));
    }
    assert_eq!(runner.instruments[0].pluck_config["decayMs"], 1505);
    assert_eq!(runner.instruments[0].pluck_config["brightnessPct"], 66);
    assert_eq!(runner.instruments[0].pluck_config["pickPositionPct"], 26);
    assert_eq!(runner.audio_config_revision, 0);
}

#[test]
fn pluck_string_aux_and_xy_picker_composition_do_not_replace_other_slot() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].kind = "pluck".into();
    runner.instruments[1].kind = "synth".into();
    runner.menu.rebuild(runner.menu_config());
    let _ = runner.messages_with_snapshot().unwrap();
    let other = runner.instruments[1].clone();
    let mapping = runner.resolve_aux_auto_map("", Some("instruments.0.pluck.decayMs"), None);
    assert_eq!(
        mapping
            .iter()
            .map(|slot| slot.as_ref().unwrap().turn.as_ref().unwrap().key.as_str())
            .collect::<Vec<_>>(),
        [
            "instruments.0.pluck.decayMs",
            "instruments.0.pluck.brightnessPct",
            "instruments.0.pluck.pickPositionPct",
            "instruments.0.pluck.amp.gainPct",
        ]
    );
    assert!(runner.menu.focus_item_key("instruments.0.pluck.decayMs"));
    let aux = input(
        &mut runner,
        json!({ "type": "encoder_turn", "id": "aux1", "delta": 1 }),
    );
    assert_scalar(&aux, "pluck.decayMs");
    assert_eq!(runner.instruments[1], other);

    runner.active_play_mode = "xy".into();
    runner.xy_smoothing_ms = 0;
    runner.menu.rebuild(runner.menu_config());
    assert!(runner
        .menu
        .focus_item_key("xy:x.instruments.0.pluck.filter.cutoffHz"));
    input(
        &mut runner,
        json!({ "type": "encoder_press", "id": "main" }),
    );
    assert_eq!(
        runner.xy_x_binding.as_ref().unwrap().key,
        "instruments.0.pluck.filter.cutoffHz"
    );
    let _ = runner.messages_with_snapshot().unwrap();
    let xy = input(&mut runner, json!({ "type": "grid_press", "x": 3, "y": 0 }));
    assert_scalar(&xy, "pluck.filter.cutoffHz");
    assert_eq!(runner.instruments[1], other);
    let saved = runner.config_payload();
    let mut loaded = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    loaded.apply_config_payload(saved).unwrap();
    assert_eq!(
        loaded.xy_x_binding.unwrap().key,
        "instruments.0.pluck.filter.cutoffHz"
    );
}

#[test]
fn pluck_held_xy_edit_rebases_and_pick_position_is_next_pluck_only() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].kind = "pluck".into();
    runner.menu.rebuild(runner.menu_config());
    runner.active_play_mode = "xy".into();
    runner.xy_smoothing_ms = 0;
    runner.xy_x_binding = Some(NativeParamBinding {
        key: "instruments.0.pluck.brightnessPct".into(),
        label: None,
        kind: "number".into(),
        min: Some(0.0),
        max: Some(100.0),
        step: Some(1.0),
        user_min: None,
        user_max: None,
        options: vec![],
        invert: false,
    });
    let _ = runner.messages_with_snapshot().unwrap();
    let held = input(&mut runner, json!({ "type": "grid_press", "x": 3, "y": 0 }));
    assert_scalar(&held, "pluck.brightnessPct");
    assert!(runner
        .menu
        .focus_item_key("instruments.0.pluck.brightnessPct"));
    runner.menu.state.editing = true;
    let edit = input(
        &mut runner,
        json!({ "type": "encoder_turn", "id": "main", "delta": 1 }),
    );
    assert!(!edit
        .iter()
        .any(|message| matches!(message, RunnerMessage::AudioCommands { .. })));
    let new_base = runner
        .menu
        .number_for_key("instruments.0.pluck.brightnessPct")
        .unwrap();
    runner.set_param_binding_target("xy:x", None);
    assert!(
        matches!(&runner.outbox.drain_audio_commands()[..], [RuntimeAudioCommand::SetPluckParam {
        instrument_slot: 0, path, value, ..
    }] if path == "pluck.brightnessPct" && *value == new_base as f32)
    );
    assert_eq!(
        runner.instruments[0].pluck_config["brightnessPct"],
        new_base
    );

    assert!(!super::super::modulation_audio::is_live_link_lfo_target(
        "instruments.0.pluck.pickPositionPct"
    ));
    assert!(runner
        .menu
        .focus_item_key("instruments.0.pluck.pickPositionPct"));
    runner.menu.state.editing = true;
    let pick = input(
        &mut runner,
        json!({ "type": "encoder_turn", "id": "main", "delta": 1 }),
    );
    assert_scalar(&pick, "pluck.pickPositionPct");
    assert_eq!(runner.instruments[0].pluck_config["pickPositionPct"], 26);
}

#[test]
fn pluck_live_lfo_targets_recompose_as_scalars_without_ring_replacement_commands() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].kind = "pluck".into();
    runner.transport.transport = RuntimeTransportState::Playing;
    for (field, min, max) in [("decayMs", 100.0, 5000.0), ("brightnessPct", 0.0, 100.0)] {
        let key = format!("instruments.0.pluck.{field}");
        assert!(super::super::modulation_audio::is_live_link_lfo_target(
            &key
        ));
        runner.link_lfos[0].enabled = true;
        runner.link_lfos[0].phase_pulses = 3;
        runner.link_lfos[0].target = Some(NativeParamBinding {
            key,
            label: None,
            kind: "number".into(),
            min: Some(min),
            max: Some(max),
            step: Some(1.0),
            user_min: None,
            user_max: None,
            options: vec![],
            invert: false,
        });
        runner.recompose_lfo_audio(false).unwrap();
        assert!(
            matches!(&runner.outbox.drain_audio_commands()[..], [RuntimeAudioCommand::SetPluckParam {
            instrument_slot: 0, path, ..
        }] if path == &format!("pluck.{field}"))
        );
        runner.link_lfos[0].enabled = false;
        runner.recompose_lfo_audio(false).unwrap();
        assert!(runner
            .outbox
            .drain_audio_commands()
            .iter()
            .all(|command| matches!(
                command,
                RuntimeAudioCommand::SetPluckParam {
                    instrument_slot: 0,
                    ..
                }
            )));
    }
    assert!(!super::super::modulation_audio::is_live_link_lfo_target(
        "instruments.0.pluck.pickPositionPct"
    ));
}
