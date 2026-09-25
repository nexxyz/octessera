use super::*;

fn instrument_runner(kind: &str) -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].kind = kind.into();
    runner.menu.rebuild(runner.menu_config());
    runner.transport.transport = RuntimeTransportState::Playing;
    let _ = runner.messages_with_snapshot().unwrap();
    runner
}

fn focus(runner: &mut NativeRunner, key: &str) {
    assert!(runner.menu.focus_item_key(key), "{key}");
}

fn assert_map(runner: &NativeRunner, expected: [Option<(&str, &str)>; 4]) {
    for (index, expected) in expected.into_iter().enumerate() {
        let slot = runner.effective_aux_slot(index);
        assert_eq!(
            slot.turn
                .as_ref()
                .map(|turn| (turn.key.as_str(), turn.label.as_str())),
            expected,
            "A{}",
            index + 1
        );
    }
}

fn turn(runner: &mut NativeRunner, id: &str) -> Vec<RuntimeAudioCommand> {
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "id": id, "delta": 1 }),
            request_snapshot: None,
        })
        .unwrap()
        .iter()
        .flat_map(|message| match message {
            RunnerMessage::AudioCommands { commands } => commands.clone(),
            _ => Vec::new(),
        })
        .collect()
}

fn assert_overlay(runner: &mut NativeRunner, title: &str, labels: [&str; 3]) {
    runner.display.ui.fn_held = true;
    runner.display.fn_hold_started_at = Some(Instant::now() - Duration::from_millis(1600));
    let snapshot = runner.snapshot().unwrap();
    assert_eq!(snapshot["display"]["title"], "AUTO MAP");
    let lines = snapshot["display"]["lines"].as_array().unwrap();
    assert_eq!(lines[0], title);
    for (index, label) in labels.into_iter().enumerate() {
        assert_eq!(lines[index + 1], format!("A{} {label}", index + 1));
    }
    runner.display.ui.fn_held = false;
}

#[test]
fn fm_tone_auto_map_turns_index_without_replacing_any_slot() {
    let mut runner = instrument_runner("fm");
    let other = runner.instruments[1].clone();
    focus(&mut runner, "instruments.0.fm.ratio");
    assert_map(
        &runner,
        [
            Some(("instruments.0.fm.index", "Index")),
            Some(("instruments.0.fm.filter.cutoffHz", "Cutoff")),
            Some(("instruments.0.fm.filter.resonance", "Res")),
            Some(("instruments.0.fm.amp.gainPct", "Gain")),
        ],
    );
    assert_overlay(&mut runner, "Aux Map", ["Index", "Cutoff", "Res"]);
    let commands = turn(&mut runner, "aux1");
    assert!(
        matches!(&commands[..], [RuntimeAudioCommand::SetFmParam {
        instrument_slot: 0, path, value: 51.0, ..
    }] if path == "fm.index"),
        "{commands:?}"
    );
    assert_eq!(runner.instruments[1], other);
    focus(&mut runner, "instruments.0.fm.filter.cutoffHz");
    assert_map(
        &runner,
        [
            Some(("instruments.0.fm.filter.cutoffHz", "Cutoff")),
            Some(("instruments.0.fm.filter.resonance", "Res")),
            Some(("instruments.0.fm.filter.envAmountPct", "Env")),
            Some(("instruments.0.fm.filter.keyTrackingPct", "Key")),
        ],
    );
    focus(&mut runner, "instruments.0.fm.amp.gainPct");
    assert_map(
        &runner,
        [
            Some(("instruments.0.fm.amp.gainPct", "Gain")),
            Some(("instruments.0.fm.amp.velocitySensitivityPct", "Vel")),
            None,
            None,
        ],
    );
}

#[test]
fn plucked_string_auto_map_turns_decay_without_replacing_any_slot() {
    let mut runner = instrument_runner("pluck");
    let other = runner.instruments[1].clone();
    focus(&mut runner, "instruments.0.pluck.decayMs");
    assert_map(
        &runner,
        [
            Some(("instruments.0.pluck.decayMs", "Decay")),
            Some(("instruments.0.pluck.brightnessPct", "Bright")),
            Some(("instruments.0.pluck.pickPositionPct", "Pick")),
            Some(("instruments.0.pluck.amp.gainPct", "Gain")),
        ],
    );
    assert_overlay(&mut runner, "Aux Map", ["Decay", "Bright", "Pick"]);
    let commands = turn(&mut runner, "aux1");
    assert!(
        matches!(&commands[..], [RuntimeAudioCommand::SetPluckParam {
        instrument_slot: 0, path, value: 1505.0, ..
    }] if path == "pluck.decayMs"),
        "{commands:?}"
    );
    assert_eq!(runner.instruments[1], other);
    focus(&mut runner, "instruments.0.pluck.filter.cutoffHz");
    assert_map(
        &runner,
        [
            Some(("instruments.0.pluck.filter.cutoffHz", "Cutoff")),
            Some(("instruments.0.pluck.filter.resonance", "Res")),
            Some(("instruments.0.pluck.filter.envAmountPct", "Env")),
            Some(("instruments.0.pluck.filter.keyTrackingPct", "Key")),
        ],
    );
    focus(&mut runner, "instruments.0.pluck.amp.gainPct");
    assert_map(
        &runner,
        [
            Some(("instruments.0.pluck.amp.gainPct", "Gain")),
            Some(("instruments.0.pluck.amp.velocitySensitivityPct", "Vel")),
            None,
            None,
        ],
    );
}

#[test]
fn drum_voice_auto_map_follows_selected_voice_and_edits_only_its_tone() {
    let mut runner = instrument_runner("drum");
    let other = runner.instruments[1].clone();
    focus(&mut runner, "instruments.0.drum.voice");
    assert_map(
        &runner,
        [
            Some(("instruments.0.drum.voices.0.tuneSemis", "Tune")),
            Some(("instruments.0.drum.voices.0.decayMs", "Decay")),
            Some(("instruments.0.drum.voices.0.tonePct", "Tone")),
            Some(("instruments.0.drum.voices.0.attackMs", "Attack")),
        ],
    );
    runner.menu.state.editing = true;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "id": "main", "delta": 2 }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(runner.drum_selected_voices[0], 2);
    let _ = runner.messages_with_snapshot().unwrap();
    focus(&mut runner, "instruments.0.drum.voice");
    let voice_two = [
        Some(("instruments.0.drum.voices.2.tuneSemis", "Tune")),
        Some(("instruments.0.drum.voices.2.decayMs", "Decay")),
        Some(("instruments.0.drum.voices.2.tonePct", "Tone")),
        Some(("instruments.0.drum.voices.2.attackMs", "Attack")),
    ];
    assert_map(&runner, voice_two);
    assert_overlay(&mut runner, "Aux Map", ["Tune", "Decay", "Tone"]);
    focus(&mut runner, "instruments.0.drum.voices.2.tonePct");
    assert_map(&runner, voice_two);
    let original = runner.instruments[0].drum_config["voices"][0].clone();
    let commands = turn(&mut runner, "aux3");
    assert!(
        matches!(&commands[..], [RuntimeAudioCommand::SetDrumParam {
        instrument_slot: 0, voice: 2, path, ..
    }] if path == "drum.tonePct"),
        "{commands:?}"
    );
    assert_eq!(runner.instruments[0].drum_config["voices"][0], original);
    assert_eq!(runner.instruments[1], other);
    focus(&mut runner, "instruments.0.drum.filter.cutoffHz");
    assert_map(
        &runner,
        [
            Some(("instruments.0.drum.filter.cutoffHz", "Cutoff")),
            Some(("instruments.0.drum.filter.resonance", "Res")),
            Some(("instruments.0.drum.filter.envAmountPct", "Env")),
            Some(("instruments.0.drum.filter.keyTrackingPct", "Key")),
        ],
    );
    focus(&mut runner, "instruments.0.drum.amp.gainPct");
    assert_map(
        &runner,
        [
            Some(("instruments.0.drum.amp.gainPct", "Gain")),
            Some(("instruments.0.drum.amp.velocitySensitivityPct", "Vel")),
            None,
            None,
        ],
    );
}

#[test]
fn sampler_auto_map_excludes_unrendered_filter_and_envelope_controls() {
    let mut runner = instrument_runner("sampler");
    focus(&mut runner, "instruments.0.sample.filter.cutoffHz");
    assert_map(
        &runner,
        [
            Some(("instruments.0.sample.filter.cutoffHz", "Cutoff")),
            Some(("instruments.0.sample.filter.resonance", "Res")),
            None,
            None,
        ],
    );
    assert_overlay(&mut runner, "Sample Filter", ["Cutoff", "Res", "-"]);
    for key in [
        "instruments.0.sample.ampEnv.attackMs",
        "instruments.0.sample.filterEnv.attackMs",
    ] {
        focus(&mut runner, key);
        assert_map(&runner, [None, None, None, None]);
    }
    focus(&mut runner, "instruments.0.sample.amp.gainPct");
    assert_map(
        &runner,
        [
            Some(("instruments.0.sample.amp.gainPct", "Gain")),
            Some(("instruments.0.sample.amp.velocitySensitivityPct", "Vel")),
            None,
            None,
        ],
    );
    focus(&mut runner, "instruments.0.sample.baseVelocity");
    assert_map(
        &runner,
        [
            Some(("instruments.0.sample.selectedSlot", "Slot")),
            Some(("instruments.0.sample.baseVelocity", "Base")),
            Some(("instruments.0.sample.tuneSemis", "Tune")),
            Some(("instruments.0.sample.velocityLevelsEnabled", "Levels")),
        ],
    );
}
