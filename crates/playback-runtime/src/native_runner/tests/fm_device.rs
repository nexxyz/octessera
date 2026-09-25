use super::*;

#[test]
fn fm_device_type_switch_and_numeric_encoder_edits_emit_prepared_slot_and_scalar_commands() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let _ = runner.messages_with_snapshot().unwrap();
    assert!(runner.menu.focus_item_key("instruments.0.type"));
    runner.menu.state.editing = true;
    let turn = |runner: &mut NativeRunner, delta: i32| {
        runner
            .send(HostMessage::DeviceInput {
                input: json!({ "type": "encoder_turn", "delta": delta, "id": "main" }),
                request_snapshot: None,
            })
            .unwrap()
    };
    let switched = turn(&mut runner, 3);
    assert_eq!(runner.instruments[0].kind, "fm");
    assert!(switched.iter().any(|message| matches!(message,
        RunnerMessage::AudioCommands { commands } if commands.iter().any(|command| matches!(command,
            RuntimeAudioCommand::SetInstrumentSlot { instrument_slot: 0, config, .. }
                if config["type"] == "fm" && config["fm"]["index"] == 50
        ))
    )));
    for (key, delta, path, expected) in [
        ("fm.index", 1, "fm.index", 51.0),
        ("fm.indexEnv.attackMs", 1, "fm.indexEnv.attackMs", 5.0),
        ("fm.amp.gainPct", -1, "fm.amp.gainPct", 79.0),
        (
            "fm.filter.cutoffHz",
            1,
            "fm.filter.cutoffHz",
            cutoff_display_to_hz(cutoff_hz_to_display(8000) + 1) as f32,
        ),
        ("fm.filter.resonance", 1, "fm.filter.resonance", 21.0),
        ("fm.ampEnv.sustainPct", 1, "fm.ampEnv.sustainPct", 71.0),
        ("fm.filterEnv.releaseMs", 1, "fm.filterEnv.releaseMs", 185.0),
    ] {
        assert!(
            runner.menu.focus_item_key(&format!("instruments.0.{key}")),
            "{key}"
        );
        runner.menu.state.editing = true;
        let messages = turn(&mut runner, delta);
        assert!(messages.iter().any(|message| matches!(message,
            RunnerMessage::AudioCommands { commands } if commands.iter().any(|command| matches!(command,
                RuntimeAudioCommand::SetFmParam { instrument_slot: 0, path: actual, value, .. }
                    if actual == path && (*value - expected).abs() < f32::EPSILON
            ))
        )), "{key}: {messages:?}");
        assert!(!messages.iter().any(|message| matches!(message,
            RunnerMessage::AudioCommands { commands } if commands.iter().any(|command| matches!(command,
                RuntimeAudioCommand::SetAudioConfig { .. } | RuntimeAudioCommand::SetInstrumentSlot { .. }
            ))
        )));
    }
    for key in ["fm.ratio", "fm.filter.type"] {
        assert!(runner.menu.focus_item_key(&format!("instruments.0.{key}")));
        runner.menu.state.editing = true;
        let messages = turn(&mut runner, 1);
        assert!(messages.iter().any(|message| matches!(message,
            RunnerMessage::AudioCommands { commands } if commands.iter().any(|command| matches!(command,
                RuntimeAudioCommand::SetInstrumentSlot { instrument_slot: 0, config, .. }
                    if config["type"] == "fm"
            ))
        )), "{key}: {messages:?}");
        assert!(!messages.iter().any(|message| matches!(message,
            RunnerMessage::AudioCommands { commands } if commands.iter().any(|command| matches!(command,
                RuntimeAudioCommand::SetAudioConfig { .. }
            ))
        )));
    }
    assert_eq!(runner.audio_config_revision, 0);
    let fm = runner.instruments[0].fm_config.clone();
    assert!(runner.menu.focus_item_key("instruments.0.type"));
    runner.menu.state.editing = true;
    turn(&mut runner, -3);
    assert_eq!(runner.instruments[0].kind, "synth");
    turn(&mut runner, 3);
    assert_eq!(runner.instruments[0].fm_config, fm);
}
