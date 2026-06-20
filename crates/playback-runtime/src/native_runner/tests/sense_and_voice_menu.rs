use super::*;

#[test]
fn entering_l1_selects_active_part_row() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.active_part_index = 2;
    runner.menu.rebuild(runner.menu_config());

    let entered = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    let snapshot = snapshot_from(&entered);
    assert_eq!(snapshot["display"]["title"], "L1: Life");
    assert_eq!(snapshot["selectedRow"], 2);
}

#[test]
fn entering_l2_selects_active_part_row_after_event_group() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.active_part_index = 2;
    runner.menu.rebuild(runner.menu_config());

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
        })
        .unwrap();
    let entered = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    let snapshot = snapshot_from(&entered);
    assert_eq!(snapshot["display"]["title"], "L2: Sense");
    assert_eq!(snapshot["selectedRow"], 3);
}

#[test]
fn l2_sense_exposes_aux_mappings_and_enterable_part_rows() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
        })
        .unwrap();
    let entered_l2 = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    let lines = snapshot_from(&entered_l2)["display"]["lines"]
        .as_array()
        .unwrap()
        .clone();
    assert!(lines
        .iter()
        .any(|line| line.as_str().unwrap_or("") == "  Aux Mappings"));
    assert!(lines
        .iter()
        .any(|line| line.as_str().unwrap_or("") == "> P1: life"));
    assert!(lines
        .iter()
        .any(|line| line.as_str().unwrap_or("").contains("P1:")));

    let part = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    let snapshot = snapshot_from(&part);
    assert_eq!(snapshot["display"]["title"], "L2: Sense/P1: life");
    let part_lines = snapshot["display"]["lines"].as_array().unwrap().clone();
    assert!(part_lines
        .iter()
        .any(|line| line.as_str().unwrap_or("") == "> Scanning"));
    assert!(part_lines
        .iter()
        .any(|line| line.as_str().unwrap_or("").trim() == "Events"));
}

#[test]
fn l2_sense_scan_mode_edits_into_config_payload() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.state.stack = vec![1, 1, 0];
    runner.menu.state.cursor = 0;

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();

    assert_eq!(
        runner.config_payload()["runtimeConfig"]["parts"][0]["l2"]["scanMode"],
        "scanning"
    );
}

#[test]
fn entering_part_row_updates_active_part_index() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 2, "id": "main" }),
        })
        .unwrap();
    let entered = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();

    assert_eq!(runner.active_part_index, 2);
    assert_eq!(
        snapshot_from(&entered)["display"]["title"],
        "L1: Life/P3: life"
    );
}

#[test]
fn instrument_list_shows_compact_name_labels() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[1].kind = "sampler".into();
    runner.instruments[1].name = "sampler".into();
    runner.menu.rebuild(runner.menu_config());

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 2, "id": "main" }),
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    let entered = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();

    let lines = snapshot_from(&entered)["display"]["lines"]
        .as_array()
        .unwrap()
        .clone();
    assert!(lines
        .iter()
        .any(|line| line.as_str().unwrap_or("").contains("I1: synth")));
}

#[test]
fn snapshot_exposes_platform_sized_instrument_slots() {
    let runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let snapshot = runner.snapshot().unwrap();
    let instruments = snapshot["settings"]["instruments"].as_array().unwrap();

    assert_eq!(instruments.len(), INSTRUMENT_COUNT);
    assert!(instruments
        .iter()
        .all(|instrument| instrument["type"] == "synth"));
}

#[test]
fn voice_menu_visibility_follows_instrument_type() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].kind = "sampler".into();
    runner.instruments[0].name = "sampler".into();
    runner.menu.rebuild(runner.menu_config());

    let _ = runner.send(HostMessage::DeviceInput {
        input: json!({ "type": "encoder_turn", "delta": 2, "id": "main" }),
    });
    let _ = runner.send(HostMessage::DeviceInput {
        input: json!({ "type": "encoder_press", "id": "main" }),
    });
    let _ = runner.send(HostMessage::DeviceInput {
        input: json!({ "type": "encoder_press", "id": "main" }),
    });
    let sampler = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
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
fn midi_instrument_params_edit_through_menu() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].kind = "midi".into();
    runner.instruments[0].name = "midi".into();
    runner.menu.rebuild(runner.menu_config());
    runner.menu.state.stack = vec![2, 0, 0, 2];
    runner.menu.state.cursor = 1;

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 4, "id": "main" }),
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();

    runner.menu.state.cursor = 2;

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": -10, "id": "main" }),
        })
        .unwrap();
    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
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
fn instrument_clone_and_reset_actions_update_slots() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].kind = "sampler".into();
    runner.instruments[0].name = "kit".into();
    runner.instruments[0].auto_name = false;

    runner
        .execute_menu_action(NativeMenuAction::ResetInstrument { index: 1 })
        .unwrap();

    assert_eq!(runner.instruments[1].kind, "none");
    assert_eq!(runner.instruments[1].name, "none");
    assert_eq!(runner.instruments[1].midi_channel, 2);

    runner
        .execute_menu_action(NativeMenuAction::CloneInstrument { index: 0 })
        .unwrap();

    assert_eq!(runner.instruments[1].kind, "sampler");
    assert_eq!(runner.instruments[1].name, "sampler");
    assert!(runner.instruments[1].auto_name);
    assert!(!runner.instruments[1].midi_enabled);
    assert_eq!(runner.instruments[1].midi_channel, 2);
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["instruments"][1]["type"],
        "sampler"
    );
}
