use super::*;

fn press(runner: &mut NativeRunner) -> Vec<RunnerMessage> {
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap()
}

fn open_action(runner: &mut NativeRunner, key: &str) -> Vec<RunnerMessage> {
    let menu_key = key.replace(':', ".");
    assert!(runner.menu.focus_item_key(&menu_key), "menu key {menu_key}");
    press(runner)
}

fn audio(messages: &[RunnerMessage]) -> Vec<&RuntimeAudioCommand> {
    messages
        .iter()
        .flat_map(|message| match message {
            RunnerMessage::AudioCommands { commands } => commands.iter().collect::<Vec<_>>(),
            _ => Vec::new(),
        })
        .collect()
}

#[test]
fn factory_load_cancel_and_back_do_not_change_patch_or_send_audio() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let _ = runner.messages_with_snapshot().unwrap();
    for (kind, key) in [
        ("fm", "fm.preset:0:bell"),
        ("pluck", "pluck.preset:0:steel"),
        ("sampler", "sample.kit:0:sdbkit"),
        ("drum", "drum.kit:0:heavy"),
        ("synth", "synth.preset:0:lead"),
    ] {
        runner.instruments[0].kind = kind.into();
        runner.menu.rebuild(runner.menu_config());
        let before = runner.config_payload();
        for use_back in [false, true] {
            let opened = open_action(&mut runner, key);
            assert!(audio(&opened).is_empty());
            let dialog = runner.display.confirm_dialog.as_ref().unwrap();
            assert_eq!(dialog.cursor, 0);
            assert_eq!(dialog.options, ["Cancel", "Confirm"]);
            assert!(dialog.lines.join(" ").contains("I1"));
            let closed = runner
                .send(HostMessage::DeviceInput {
                    input: if use_back {
                        json!({"type":"button_a", "pressed":true})
                    } else {
                        json!({"type":"encoder_press", "id":"main"})
                    },
                    request_snapshot: None,
                })
                .unwrap();
            assert!(runner.display.confirm_dialog.is_none());
            assert!(audio(&closed).is_empty());
            assert_eq!(runner.config_payload(), before);
        }
    }
}

#[test]
fn factory_load_confirm_replaces_one_slot_only_and_schedules_autosave() {
    for (kind, key, field, value) in [
        ("fm", "fm.preset:2:bell", "fm", "3"),
        ("pluck", "pluck.preset:2:steel", "pluck", "3200"),
        (
            "sampler",
            "sample.kit:2:synthkit",
            "sample",
            "samples/Drum/kick/synthkit-kick.wav",
        ),
        ("drum", "drum.kit:2:tight", "drum", "260"),
        ("synth", "synth.preset:2:bell", "synth", "sine"),
    ] {
        let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
        runner.auto_save_default = true;
        let _ = runner.messages_with_snapshot().unwrap();
        runner.instruments[2].kind = kind.into();
        runner.instruments[2].name = derive_instrument_name(2, kind);
        runner.instruments[2].volume = 37;
        runner.instruments[2].pan_pos = 3;
        runner.instruments[2].route = "fx_bus_2".into();
        runner.instruments[2].note_behavior = "hold".into();
        let before = runner.instruments.clone();
        runner.menu.rebuild(runner.menu_config());
        let opened = open_action(&mut runner, key);
        assert!(audio(&opened).is_empty());
        let confirmed = confirm_current_dialog(&mut runner);
        let commands = audio(&confirmed);
        assert!(
            matches!(commands.as_slice(), [RuntimeAudioCommand::SetInstrumentSlot { instrument_slot: 2, generation: 1, config }] if config["type"] == kind && config["mixer"]["volume"] == 37),
            "{key}: {commands:?}"
        );
        let current = &runner.instruments[2];
        assert_eq!(current.volume, before[2].volume);
        assert_eq!(current.pan_pos, before[2].pan_pos);
        assert_eq!(current.route, before[2].route);
        assert_eq!(current.note_behavior, before[2].note_behavior);
        assert_eq!(current.name, derive_instrument_name(2, kind));
        assert_eq!(&runner.instruments[..2], &before[..2]);
        assert_eq!(&runner.instruments[3..], &before[3..]);
        assert!(runner.config_dirty);
        assert!(runner.pending.pending_autosave_payload_due_at.is_some());
        let loaded = match field {
            "fm" => current.fm_config["ratio"].as_str().unwrap().to_string(),
            "pluck" => current.pluck_config["decayMs"].to_string(),
            "sample" => current.sample_paths[0].clone().unwrap(),
            "drum" => current.drum_config["voices"][0]["decayMs"].to_string(),
            _ => current.synth_config["osc1"]["waveform"]
                .as_str()
                .unwrap()
                .into(),
        };
        assert_eq!(loaded, value, "{key}");
        runner.make_deferred_menu_apply_due_for_test();
        let saved = runner.flush_deferred_menu_apply().unwrap();
        assert!(saved.iter().any(|message| matches!(message, RunnerMessage::PlatformEffects { effects } if effects.iter().any(|effect| matches!(effect, RuntimePlatformEffect::StoreSaveDefault { mode, .. } if mode.as_deref() == Some("deferred"))))));
    }
}

#[test]
fn custom_names_and_inactive_type_blocks_survive_factory_loads_and_portable_roundtrip() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let slot = &mut runner.instruments[1];
    slot.auto_name = false;
    slot.name = "My Noise".into();
    slot.fm_config["index"] = json!(97);
    slot.pluck_config["brightnessPct"] = json!(1);
    slot.synth_config["osc1"]["levelPct"] = json!(4);
    slot.sample_assignments.push(NativeSampleAssignment {
        x: 3,
        y: 0,
        sample_slot: 7,
        level: Some("high".into()),
    });
    slot.selected_sample_slot = 6;
    slot.sample_tune_semis = -7;
    slot.sample_velocity_levels_enabled = true;
    slot.drum_config["assignments"] = json!([{ "x": 5, "y": 2, "voice": 7, "tuneSemis": -24 }, { "x": 0, "y": 0, "voice": 0, "tuneSemis": 24 }]);
    slot.drum_config["filter"]["resonance"] = json!(65);
    let original = slot.clone();

    for action in [
        "fm.preset:1:soft_keys",
        "pluck.preset:1:nylon",
        "sample.kit:1:distkit",
        "drum.kit:1:heavy",
    ] {
        runner
            .execute_confirmed_action(NativeMenuAction::PlatformEffect(action.into()))
            .unwrap();
    }
    let current = &runner.instruments[1];
    assert_eq!(current.name, "My Noise");
    assert_eq!(current.synth_config, original.synth_config);
    assert_eq!(current.fm_config["ratio"], "1");
    assert_eq!(current.pluck_config["brightnessPct"], 35);
    assert_eq!(current.sample_assignments, original.sample_assignments);
    assert_eq!(current.selected_sample_slot, original.selected_sample_slot);
    assert_eq!(current.sample_tune_semis, original.sample_tune_semis);
    assert_eq!(
        current.sample_velocity_levels_enabled,
        original.sample_velocity_levels_enabled
    );
    assert_eq!(
        current.drum_config["assignments"],
        original.drum_config["assignments"]
    );
    assert_eq!(
        current.drum_config["filter"],
        original.drum_config["filter"]
    );
    assert_eq!(current.drum_config["voices"][0]["tuneSemis"], -4);
    let patch = portable_patch_projection(&runner.config_payload()).unwrap();
    let mut loaded = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    loaded
        .apply_patch_payload_preserving_device(patch.clone())
        .unwrap();
    assert_eq!(loaded.instruments[1], runner.instruments[1]);
    assert_eq!(
        portable_patch_projection(&loaded.config_payload()).unwrap(),
        patch
    );
}

#[test]
fn unknown_catalog_ids_and_invalid_slots_leave_patch_and_audio_unchanged() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let before = runner.config_payload();
    for action in [
        "fm.preset:0:missing",
        "pluck.preset:0:missing",
        "sample.kit:0:missing",
        "drum.kit:0:missing",
        "sample.kit:8:sdbkit",
        "synth.preset:0:missing",
    ] {
        let result = runner
            .execute_confirmed_action(NativeMenuAction::PlatformEffect(action.into()))
            .unwrap();
        assert!(result.is_none());
        assert_eq!(runner.config_payload(), before, "{action}");
        assert!(
            runner
                .display
                .toast
                .as_ref()
                .unwrap()
                .message
                .contains("Unknown")
                || runner
                    .display
                    .toast
                    .as_ref()
                    .unwrap()
                    .message
                    .contains("rejected")
        );
        assert!(!runner.outbox.has_audio_commands());
    }
}
