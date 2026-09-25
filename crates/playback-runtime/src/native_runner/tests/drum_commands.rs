use super::*;

fn input(runner: &mut NativeRunner, input: Value) -> Vec<RunnerMessage> {
    runner
        .send(HostMessage::DeviceInput {
            input,
            request_snapshot: None,
        })
        .unwrap()
}

fn commands(messages: &[RunnerMessage]) -> Vec<RuntimeAudioCommand> {
    messages
        .iter()
        .flat_map(|message| match message {
            RunnerMessage::AudioCommands { commands } => commands.clone(),
            _ => Vec::new(),
        })
        .collect()
}

fn drum_runner() -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["instruments"][0]["type"] = json!("drum");
    runner.apply_config_payload(payload).unwrap();
    runner.menu.rebuild(runner.menu_config());
    let _ = runner.messages_with_snapshot().unwrap();
    runner
}

#[test]
fn sound_is_structural_but_kit_voice_and_common_numeric_edits_are_typed() {
    let mut runner = drum_runner();
    let revision = runner.audio_config_revision;
    let other = runner.instruments[1].clone();
    for (key, path, voice) in [
        ("instruments.0.drum.voices.0.decayMs", "drum.decayMs", 0),
        ("instruments.0.drum.voices.0.tonePct", "drum.tonePct", 0),
        ("instruments.0.drum.voices.0.attackMs", "drum.attackMs", 0),
        ("instruments.0.drum.voices.0.tuneSemis", "drum.tuneSemis", 0),
        ("instruments.0.drum.amp.gainPct", "drum.amp.gainPct", 0),
        (
            "instruments.0.drum.filter.resonance",
            "drum.filter.resonance",
            0,
        ),
    ] {
        assert!(runner.menu.focus_item_key(key), "{key}");
        runner.menu.state.editing = true;
        let edited = commands(&input(
            &mut runner,
            json!({ "type": "encoder_turn", "id": "main", "delta": 1 }),
        ));
        assert!(
            matches!(&edited[..], [RuntimeAudioCommand::SetDrumParam {
            instrument_slot: 0, voice: actual_voice, path: actual_path, ..
        }] if *actual_voice == voice && actual_path == path),
            "{key}: {edited:?}"
        );
    }
    assert_eq!(runner.instruments[1], other);
    assert_eq!(runner.audio_config_revision, revision);

    let before = runner.instruments[0].drum_config["voices"][0].clone();
    assert!(runner
        .menu
        .focus_item_key("instruments.0.drum.voices.0.sound"));
    runner.menu.state.editing = true;
    let changed = commands(&input(
        &mut runner,
        json!({ "type": "encoder_turn", "id": "main", "delta": 1 }),
    ));
    assert!(
        matches!(&changed[..], [RuntimeAudioCommand::SetInstrumentSlot {
        instrument_slot: 0, config, ..
    }] if config["type"] == "drum" && config["drum"]["voices"][0]["sound"] == "snare"),
        "{changed:?}"
    );
    assert_ne!(runner.instruments[0].drum_config["voices"][0], before);
    assert_eq!(
        runner.instruments[0].drum_config["voices"][0],
        super::super::drum_config::drum_voice_default("snare").unwrap()
    );
    assert_eq!(runner.instruments[1], other);
}

#[test]
fn aux_and_xy_next_hit_voice_targets_emit_only_voice_scoped_scalars() {
    let mut runner = drum_runner();
    runner.drum_selected_voices[0] = 2;
    runner.menu.rebuild(runner.menu_config());
    runner.transport.transport = RuntimeTransportState::Playing;
    assert!(!super::super::modulation_audio::is_live_link_lfo_target(
        "instruments.0.drum.voices.2.tonePct"
    ));
    let other = runner.instruments[1].clone();
    assert!(runner
        .menu
        .focus_item_key("aux:0:turn.instruments.0.drum.voices.2.tonePct"));
    input(
        &mut runner,
        json!({ "type": "encoder_press", "id": "main" }),
    );
    let aux = commands(&input(
        &mut runner,
        json!({ "type": "encoder_turn", "id": "aux1", "delta": -1 }),
    ));
    assert!(
        matches!(&aux[..], [RuntimeAudioCommand::SetDrumParam {
        instrument_slot: 0, voice: 2, path, ..
    }] if path == "drum.tonePct"),
        "{aux:?}"
    );
    assert_eq!(runner.instruments[1], other);

    runner.active_play_mode = "xy".into();
    runner.xy_smoothing_ms = 0;
    runner.menu.rebuild(runner.menu_config());
    assert!(runner
        .menu
        .focus_item_key("xy:x.instruments.0.drum.voices.2.decayMs"));
    input(
        &mut runner,
        json!({ "type": "encoder_press", "id": "main" }),
    );
    assert_eq!(
        runner.xy_x_binding.as_ref().unwrap().key,
        "instruments.0.drum.voices.2.decayMs"
    );
    let _ = runner.messages_with_snapshot().unwrap();
    let xy = commands(&input(
        &mut runner,
        json!({ "type": "grid_press", "x": 7, "y": 0 }),
    ));
    assert!(
        matches!(&xy[..], [RuntimeAudioCommand::SetDrumParam {
        instrument_slot: 0, voice: 2, path, ..
    }] if path == "drum.decayMs"),
        "{xy:?}"
    );
    assert_eq!(runner.instruments[1], other);
    let saved = runner.config_payload();
    assert_eq!(
        saved["runtimeConfig"]["instruments"][0]["drum"]["voices"][2]["decayMs"],
        runner.instruments[0].drum_config["voices"][2]["decayMs"]
    );
}

#[test]
fn preview_is_one_transient_hit_without_cell_offset_or_config_command() {
    let mut runner = drum_runner();
    runner.instruments[0].drum_config["voices"][4]["tuneSemis"] = json!(-5);
    runner.instruments[0].drum_config["assignments"] =
        json!([{ "x": 0, "y": 0, "voice": 4, "tuneSemis": 24 }]);
    let _ = runner.messages_with_snapshot().unwrap();
    runner
        .execute_menu_action(crate::native_menu::NativeMenuAction::PlatformEffect(
            "drum.preview:0:4".into(),
        ))
        .unwrap();
    let preview = runner.messages_with_snapshot().unwrap();
    let hits = preview
        .iter()
        .flat_map(|message| match message {
            RunnerMessage::DrumHits { hits } => hits.clone(),
            _ => Vec::new(),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        hits,
        vec![crate::protocol::DrumHit {
            instrument_slot: 0,
            voice: 4,
            tune_semis: 0,
            velocity: 100,
        }]
    );
    assert!(commands(&preview).is_empty());
    assert!(!preview.iter().any(|message| matches!(
        message,
        RunnerMessage::MidiEvents { .. } | RunnerMessage::MusicalEvents { .. }
    )));
    assert_eq!(
        runner.instruments[0].drum_config["assignments"][0]["tuneSemis"],
        24
    );
    assert_eq!(
        runner.instruments[0].drum_config["voices"][4]["tuneSemis"],
        -5
    );
}
