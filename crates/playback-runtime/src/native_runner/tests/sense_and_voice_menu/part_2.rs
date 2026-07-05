use super::*;

#[test]
pub(crate) fn snapshot_exposes_platform_sized_instrument_slots() {
    let runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let snapshot = runner.snapshot().unwrap();
    let instruments = snapshot["settings"]["instruments"].as_array().unwrap();

    assert_eq!(instruments.len(), INSTRUMENT_COUNT);
    assert!(instruments
        .iter()
        .all(|instrument| instrument["type"] == "synth"));
}

#[test]
pub(crate) fn voice_menu_visibility_follows_instrument_type() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].kind = "sampler".into();
    runner.instruments[0].name = "sampler".into();
    runner.menu.rebuild(runner.menu_config());

    let _ = runner.send(HostMessage::DeviceInput {
        input: json!({ "type": "encoder_turn", "delta": 2, "id": "main" }),
        request_snapshot: None,
    });
    let _ = runner.send(HostMessage::DeviceInput {
        input: json!({ "type": "encoder_press", "id": "main" }),
        request_snapshot: None,
    });
    let _ = runner.send(HostMessage::DeviceInput {
        input: json!({ "type": "encoder_press", "id": "main" }),
        request_snapshot: None,
    });
    let sampler = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    let sampler_lines = snapshot_from(&sampler)["display"]["lines"]
        .as_array()
        .unwrap()
        .clone();
    assert!(sampler_lines
        .iter()
        .any(|line| line.as_str().unwrap_or("").trim() == "Sampler"));
    assert!(!sampler_lines
        .iter()
        .any(|line| line.as_str().unwrap_or("").trim() == "Synth"));

    runner.instruments[0].kind = "midi".into();
    runner.instruments[0].name = "midi".into();
    runner.menu.rebuild(runner.menu_config());
    let midi = runner.snapshot().unwrap();
    let midi_lines = midi["display"]["lines"].as_array().unwrap();
    assert!(midi_lines
        .iter()
        .any(|line| line.as_str().unwrap_or("").trim() == "MIDI"));
    assert!(!midi_lines
        .iter()
        .any(|line| line.as_str().unwrap_or("").trim() == "Mixer"));
}

#[test]
pub(crate) fn midi_instrument_params_edit_through_menu() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].kind = "midi".into();
    runner.instruments[0].name = "midi".into();
    runner.menu.rebuild(runner.menu_config());
    runner.menu.state.stack = vec![2, 0, 0, 2];
    runner.menu.state.cursor = 1;

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 4, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();

    runner.menu.state.cursor = 2;

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": -10, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    assert!(messages
        .iter()
        .any(|message| matches!(message, RunnerMessage::Snapshot { .. })));
    let snapshot = runner.snapshot().unwrap();

    assert_eq!(snapshot["settings"]["instruments"][0]["midi"]["channel"], 5);
    assert_eq!(
        snapshot["settings"]["instruments"][0]["midi"]["velocity"],
        90
    );
}

#[test]
pub(crate) fn instrument_clone_and_reset_actions_update_slots() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].kind = "sampler".into();
    runner.instruments[0].name = "kit".into();
    runner.instruments[0].auto_name = false;
    runner.instruments[1] = NativeInstrumentSlot::reset(1);

    runner
        .execute_menu_action(NativeMenuAction::CloneInstrument { index: 0 })
        .unwrap();
    assert_eq!(
        runner.confirm_dialog.as_ref().unwrap().title,
        "Confirm Clone"
    );
    confirm_current_dialog(&mut runner);

    assert_eq!(runner.toast.as_ref().unwrap().message, "Cloned to I2");
    assert_eq!(runner.instruments[1].kind, "sampler");
    assert_eq!(runner.instruments[1].name, "Sampler");
    assert!(runner.instruments[1].auto_name);
    assert!(!runner.instruments[1].midi_enabled);
    assert_eq!(runner.instruments[1].midi_channel, 2);
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["instruments"][1]["type"],
        "sampler"
    );

    runner
        .execute_menu_action(NativeMenuAction::ResetInstrument { index: 1 })
        .unwrap();
    assert_eq!(
        runner.confirm_dialog.as_ref().unwrap().title,
        "Confirm Reset"
    );
    confirm_current_dialog(&mut runner);

    assert_eq!(runner.toast.as_ref().unwrap().message, "Reset I2");
    assert_eq!(runner.instruments[1].kind, "none");
    assert_eq!(runner.instruments[1].name, "None");
    assert_eq!(runner.instruments[1].midi_channel, 2);
}
