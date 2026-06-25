use super::*;

#[test]
fn synth_gain_edits_into_config_payload() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.state.stack = vec![2, 0, 0, 2, 4];
    runner.menu.state.cursor = 0;

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
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();

    assert_eq!(
        runner.config_payload()["runtimeConfig"]["instruments"][0]["synth"]["amp"]["gainPct"],
        70
    );
}

#[test]
fn sampler_tune_edits_into_config_payload() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].kind = "sampler".into();
    runner.instruments[0].name = "sampler".into();
    runner.menu.rebuild(runner.menu_config());
    runner.menu.state.stack = vec![2, 0, 0, 2];
    runner.menu.state.cursor = 3;

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 7, "id": "main" }),
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();

    assert_eq!(
        runner.config_payload()["runtimeConfig"]["instruments"][0]["sample"]["tuneSemis"],
        7
    );
}

#[test]
fn sampler_extended_params_edit_into_config_payload() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].kind = "sampler".into();
    runner.instruments[0].name = "sampler".into();
    runner.menu.rebuild(runner.menu_config());

    runner.menu.state.stack = vec![2, 0, 0, 2];
    runner.menu.state.cursor = 5;
    runner.menu.state.editing = true;
    runner.menu.turn(-20);
    runner.apply_menu_state().unwrap();

    runner.menu.state.cursor = 6;
    runner.menu.turn(1);
    runner.apply_menu_state().unwrap();

    runner.menu.state.stack = vec![2, 0, 0, 2, 7];
    runner.menu.state.cursor = 0;
    runner.menu.state.editing = true;
    runner.menu.turn(-10);
    runner.apply_menu_state().unwrap();

    runner.menu.state.stack = vec![2, 0, 0, 2, 8];
    runner.menu.state.cursor = 0;
    runner.menu.state.editing = true;
    runner.menu.turn(1);
    runner.apply_menu_state().unwrap();
    runner.menu.state.cursor = 1;
    runner.menu.turn(-10);
    runner.apply_menu_state().unwrap();

    runner.menu.state.stack = vec![2, 0, 0, 2];
    runner.menu.state.cursor = 9;
    runner.menu.state.editing = true;
    runner.menu.turn(-25);
    runner.apply_menu_state().unwrap();

    runner.menu.state.stack = vec![2, 0, 0, 2, 10];
    runner.menu.state.cursor = 0;
    runner.menu.state.editing = true;
    runner.menu.turn(4);
    runner.apply_menu_state().unwrap();

    let sample = &runner.config_payload()["runtimeConfig"]["instruments"][0]["sample"];
    assert_eq!(sample["baseVelocity"], 80);
    assert_eq!(sample["velocityLevelsEnabled"], true);
    assert_eq!(sample["velocityLevels"]["high"], 110);
    assert_eq!(sample["filter"]["type"], "highpass");
    assert_eq!(sample["filter"]["cutoffHz"], 6548);
    assert_eq!(sample["amp"]["velocitySensitivityPct"], 75);
    assert_eq!(sample["ampEnv"]["attackMs"], 25);
}

#[test]
fn fx_bus_slot_type_edits_into_config_payload() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.turn_key("mixer.buses.0.slot1.type", 1);
    runner.apply_menu_state().unwrap();

    assert_eq!(
        runner.config_payload()["runtimeConfig"]["mixer"]["buses"][0]["slot1"]["type"],
        "tremolo"
    );
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["mixer"]["buses"][0]["slot1"]["params"]["rateHz"],
        4.0
    );
}

#[test]
fn global_fx_slot_type_edits_into_config_payload() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.turn_key("mixer.master.slots.0.type", 1);
    runner.apply_menu_state().unwrap();

    assert_eq!(
        runner.config_payload()["runtimeConfig"]["mixer"]["master"]["slots"][0]["type"],
        "vinyl"
    );
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["mixer"]["master"]["slots"][0]["params"]
            ["cracklePct"],
        8
    );
}

#[test]
fn dynamic_fx_type_turn_is_deferred_until_flush() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    assert!(runner.menu.focus_item_key("mixer.buses.0.slot1.type"));
    runner.menu.state.editing = true;

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
        })
        .unwrap();

    assert_eq!(
        runner
            .menu
            .value_for_key("mixer.buses.0.slot1.type")
            .as_deref(),
        Some("tremolo")
    );
    assert_eq!(runner.fx_buses[0].slot1_type, "none");
    assert_eq!(runner.audio_config_revision, 0);

    runner.make_deferred_menu_apply_due_for_test();
    let flushed = runner.flush_deferred_menu_apply().unwrap();

    assert!(!flushed.is_empty());
    assert_eq!(runner.fx_buses[0].slot1_type, "tremolo");
    assert_eq!(runner.audio_config_revision, 0);
    assert!(flushed.iter().any(|message| matches!(
        message,
        RunnerMessage::AudioCommands { commands }
            if commands.iter().any(|command| matches!(
                command,
                RuntimeAudioCommand::SetFxBusSlot { bus_index: 0, slot_index: 0, fx_type, .. }
                    if fx_type == "tremolo"
            ))
    )));
}

#[test]
fn dynamic_fx_slot2_type_turn_is_deferred_until_flush() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    assert!(runner.menu.focus_item_key("mixer.buses.0.slot2.type"));
    runner.menu.state.editing = true;

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
        })
        .unwrap();

    assert_eq!(
        runner
            .menu
            .value_for_key("mixer.buses.0.slot2.type")
            .as_deref(),
        Some("tremolo")
    );
    assert_eq!(runner.fx_buses[0].slot2_type, "none");
    assert_eq!(runner.audio_config_revision, 0);

    runner.make_deferred_menu_apply_due_for_test();
    let flushed = runner.flush_deferred_menu_apply().unwrap();

    assert_eq!(runner.fx_buses[0].slot2_type, "tremolo");
    assert_eq!(runner.audio_config_revision, 0);
    assert!(flushed.iter().any(|message| matches!(
        message,
        RunnerMessage::AudioCommands { commands }
            if commands.iter().any(|command| matches!(
                command,
                RuntimeAudioCommand::SetFxBusSlot { bus_index: 0, slot_index: 1, fx_type, .. }
                    if fx_type == "tremolo"
            ))
    )));
}

#[test]
fn dynamic_global_fx_type_turn_is_deferred_until_flush() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    assert!(runner.menu.focus_item_key("mixer.master.slots.0.type"));
    runner.menu.state.editing = true;

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
        })
        .unwrap();

    assert_eq!(
        runner
            .menu
            .value_for_key("mixer.master.slots.0.type")
            .as_deref(),
        Some("vinyl")
    );
    assert_eq!(runner.global_fx_slots[0], "none");
    assert_eq!(runner.audio_config_revision, 0);

    runner.make_deferred_menu_apply_due_for_test();
    let flushed = runner.flush_deferred_menu_apply().unwrap();

    assert_eq!(runner.global_fx_slots[0], "vinyl");
    assert_eq!(runner.audio_config_revision, 0);
    assert!(flushed.iter().any(|message| matches!(
        message,
        RunnerMessage::AudioCommands { commands }
            if commands.iter().any(|command| matches!(
                command,
                RuntimeAudioCommand::SetGlobalFxSlot { slot_index: 0, fx_type, .. }
                    if fx_type == "vinyl"
            ))
    )));
}

#[test]
fn dynamic_instrument_type_turn_is_deferred_until_flush() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    assert!(runner.menu.focus_item_key("instruments.0.type"));
    runner.menu.state.editing = true;

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
        })
        .unwrap();

    assert_eq!(
        runner.menu.value_for_key("instruments.0.type").as_deref(),
        Some("sampler")
    );
    assert_eq!(runner.instruments[0].kind, "synth");
    assert_eq!(runner.audio_config_revision, 0);

    runner.make_deferred_menu_apply_due_for_test();
    let _ = runner.flush_deferred_menu_apply().unwrap();

    assert_eq!(runner.instruments[0].kind, "sampler");
    assert_eq!(runner.audio_config_revision, 1);
}

#[test]
fn behavior_turns_coalesce_until_flush() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    assert!(runner.menu.focus_item_key("behaviorId"));
    runner.menu.state.editing = true;

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
        })
        .unwrap();

    let selected = runner.menu.selected_behavior().unwrap();
    assert_ne!(selected, runner.behavior.id());
    assert_eq!(runner.behavior.id(), "life");

    runner.make_deferred_menu_apply_due_for_test();
    let _ = runner.flush_deferred_menu_apply().unwrap();

    assert_eq!(runner.behavior.id(), selected);
}

#[test]
fn fx_params_edit_into_config_payload() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner
        .apply_config_payload(json!({
            "runtimeConfig": {
                "mixer": {
                    "buses": [{ "slot1": { "type": "delay", "params": { "timeMs": 250, "feedback": 0.35, "mixPct": 35 } } }],
                    "master": { "slots": [{ "type": "distortion", "params": { "drive": 2.5, "clip": 0.6, "mixPct": 100 } }] }
                }
            }
        }))
        .unwrap();
    runner.menu.rebuild(runner.menu_config());

    runner
        .menu
        .turn_key("mixer.buses.0.slot1.params.feedback", 1);
    runner.menu.turn_key("mixer.master.slots.0.params.clip", 1);
    runner.apply_menu_state().unwrap();

    let payload = runner.config_payload();
    assert_eq!(
        payload["runtimeConfig"]["mixer"]["buses"][0]["slot1"]["params"]["feedback"],
        0.36
    );
    assert_eq!(
        payload["runtimeConfig"]["mixer"]["master"]["slots"][0]["params"]["clip"],
        0.65
    );
}

#[test]
fn invalid_bus_and_global_fx_types_are_sanitized_on_load() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["mixer"]["buses"][0]["slot1"] =
        json!({ "type": "vinyl", "params": {} });
    payload["runtimeConfig"]["mixer"]["master"]["slots"][0] =
        json!({ "type": "delay", "params": {} });

    runner.apply_config_payload(payload).unwrap();

    assert_eq!(runner.fx_buses[0].slot1_type, "none");
    assert_eq!(runner.global_fx_slots[0], "none");
}

#[test]
fn l1_part_config_always_exposes_auto_name() {
    for behavior_id in ["life", "none", "glider"] {
        let mut runner = NativeRunner::new(NativeRunnerConfig {
            behavior_id: behavior_id.into(),
            ..NativeRunnerConfig::default()
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
        assert!(
            lines
                .iter()
                .any(|line| line.as_str().unwrap_or("").contains("Part Name")),
            "{behavior_id} should show Part Name"
        );
        assert!(
            lines
                .iter()
                .any(|line| line.as_str().unwrap_or("").contains("Auto Name")),
            "{behavior_id} should show Auto Name"
        );
    }
}

#[test]
fn behavior_change_updates_active_part_auto_name_label() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
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
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();

    assert_eq!(runner.part_behavior_ids[0], "keys");
    runner.menu.back();
    runner.menu.rebuild(runner.menu_config());
    let snapshot = runner.snapshot().unwrap();
    let lines = snapshot["display"]["lines"].as_array().unwrap();
    assert!(lines
        .iter()
        .any(|line| line.as_str().unwrap_or("") == "> P1: keys"));
}
