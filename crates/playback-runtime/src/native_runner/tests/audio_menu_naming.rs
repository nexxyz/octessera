use super::*;

#[test]
fn part_and_bus_names_round_trip_with_auto_name_flags() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.part_names[0] = "lead".into();
    runner.part_auto_names[0] = false;
    runner.fx_buses[0].name = "space".into();
    runner.fx_buses[0].auto_name = false;
    runner.fx_buses[0].slot1_type = "delay".into();
    let payload = runner.config_payload();

    let mut restored = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    restored.apply_config_payload(payload).unwrap();

    assert_eq!(restored.part_names[0], "lead");
    assert!(!restored.part_auto_names[0]);
    assert_eq!(restored.fx_buses[0].name, "space");
    assert!(!restored.fx_buses[0].auto_name);

    restored
        .apply_config_payload(json!({
            "runtimeConfig": {
                "mixer": {
                    "buses": [{ "slot1": { "type": "delay" }, "slot2": { "type": "duck" }, "autoName": true }]
                }
            }
        }))
        .unwrap();
    assert_eq!(restored.fx_buses[0].name, "Delay+Duck");

    restored
        .apply_config_payload(json!({
            "runtimeConfig": {
                "instruments": [{ "type": "sampler", "name": "sampler", "autoName": true }],
                "mixer": { "buses": [{ "slot1": { "type": "duck" }, "slot2": { "type": "none" }, "name": "duck", "autoName": true }] }
            }
        }))
        .unwrap();
    assert_eq!(restored.instruments[0].name, "Sampler");
    assert_eq!(restored.fx_buses[0].name, "Duck");

    restored
        .apply_config_payload(json!({
            "runtimeConfig": {
                "instruments": [{ "type": "sampler", "name": "manual lower", "autoName": false }],
                "mixer": { "buses": [{ "slot1": { "type": "duck" }, "slot2": { "type": "none" }, "name": "side duck", "autoName": false }] }
            }
        }))
        .unwrap();
    assert_eq!(restored.instruments[0].name, "manual lower");
    assert_eq!(restored.fx_buses[0].name, "side duck");
}

#[test]
fn native_text_row_edits_part_name_and_clears_auto_name() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.state.stack = vec![0, 0];
    runner.menu.state.cursor = 2;

    runner.menu.press();
    runner.menu.turn(1);
    let snapshot = runner.menu.snapshot();
    runner.apply_menu_state().unwrap();

    assert!(snapshot.lines.iter().any(|line| line == "    * lifeA"));
    assert!(snapshot.lines.iter().all(|line| !line.contains('@')));
    assert_eq!(runner.part_names[0], "lifeA");
    assert!(!runner.part_auto_names[0]);
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["parts"][0]["name"],
        "lifeA"
    );
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["parts"][0]["autoName"],
        false
    );
}

#[test]
fn lowercase_text_edit_and_manual_instrument_name_round_trip() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner
        .apply_config_payload(json!({
            "runtimeConfig": {
                "instruments": [{ "type": "synth", "name": "lead bass", "autoName": false }]
            }
        }))
        .unwrap();
    assert_eq!(runner.instruments[0].name, "lead bass");
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["instruments"][0]["name"],
        "lead bass"
    );

    runner.menu.state.stack = vec![0, 0];
    runner.menu.state.cursor = 2;
    runner.menu.press();
    runner.menu.turn(27);
    let snapshot = runner.menu.snapshot();
    assert!(snapshot.lines.iter().any(|line| line == "    * lifea"));
}

#[test]
fn factory_payload_uses_display_style_auto_names() {
    let payload = super::super::native_factory_payload();
    let runtime = &payload["runtimeConfig"];
    assert_eq!(runtime["instruments"][0]["name"], "Synth");
    assert_eq!(runtime["instruments"][1]["name"], "drums");
    assert_eq!(runtime["instruments"][1]["autoName"], false);
    assert!(runtime["instruments"]
        .as_array()
        .unwrap()
        .iter()
        .all(|instrument| {
            !instrument["autoName"].as_bool().unwrap_or(false)
                || instrument["name"] != "synth" && instrument["name"] != "sampler"
        }));
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.apply_config_payload(payload).unwrap();
    assert_eq!(runner.fx_buses[0].name, "Delay+Duck");
}

#[test]
fn auto_named_part_renames_when_behavior_changes_to_none() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.part_behavior_ids[1] = "sequencer".into();
    runner.part_names[1] = "sequencer".into();
    runner.part_auto_names[1] = false;
    runner.select_active_part(1).unwrap();
    runner.menu.rebuild(runner.menu_config());

    runner.menu.turn_key("parts.1.autoName", 1);
    runner.apply_menu_state().unwrap();
    assert_eq!(runner.part_names[1], "sequencer");
    assert!(runner.part_auto_names[1]);

    assert!(runner.menu.focus_item_key("behaviorId"));
    runner.menu.state.editing = true;
    runner.menu.turn_key("behaviorId", -99);
    assert_eq!(
        runner.menu.value_for_key("behaviorId").as_deref(),
        Some("none")
    );
    runner.apply_or_schedule_menu_key("behaviorId").unwrap();
    runner.make_deferred_menu_apply_due_for_test();
    runner.flush_deferred_menu_apply().unwrap();

    assert_eq!(runner.part_behavior_ids[1], "none");
    assert_eq!(runner.part_names[1], "none");
    assert!(runner.part_auto_names[1]);
    assert_eq!(
        runner.menu.value_for_key("parts.1.name").as_deref(),
        Some("none")
    );
}

#[test]
fn auto_named_part_renames_after_toggling_auto_name_off_and_on() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.part_behavior_ids[3] = "sequencer".into();
    runner.part_names[3] = "sequencer".into();
    runner.part_auto_names[3] = true;
    runner.select_active_part(3).unwrap();
    runner.menu.rebuild(runner.menu_config());

    assert!(runner.menu.focus_item_key("parts.3.autoName"));
    runner.menu.state.editing = true;
    runner.menu.turn_key("parts.3.autoName", -1);
    runner
        .apply_or_schedule_menu_key("parts.3.autoName")
        .unwrap();
    assert!(!runner.part_auto_names[3]);

    runner.menu.turn_key("parts.3.autoName", 1);
    runner
        .apply_or_schedule_menu_key("parts.3.autoName")
        .unwrap();
    assert!(runner.part_auto_names[3]);
    assert_eq!(runner.part_names[3], "sequencer");

    assert!(runner.menu.focus_item_key("behaviorId"));
    runner.menu.state.editing = true;
    runner.menu.turn_key("behaviorId", -99);
    assert_eq!(
        runner.menu.value_for_key("behaviorId").as_deref(),
        Some("none")
    );
    runner.apply_or_schedule_menu_key("behaviorId").unwrap();
    runner.make_deferred_menu_apply_due_for_test();
    runner.flush_deferred_menu_apply().unwrap();

    assert_eq!(runner.part_behavior_ids[3], "none");
    assert!(runner.part_auto_names[3]);
    assert_eq!(runner.part_names[3], "none");
    assert_eq!(
        runner.menu.value_for_key("parts.3.name").as_deref(),
        Some("none")
    );
}

#[test]
fn button_back_commits_l1_behavior_auto_name_after_manual_auto_name_toggle() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.part_behavior_ids[3] = "sequencer".into();
    runner.part_names[3] = "sequencer".into();
    runner.part_auto_names[3] = true;
    runner.select_active_part(3).unwrap();
    runner.menu.rebuild(runner.menu_config());

    assert!(runner.menu.focus_item_key("parts.3.autoName"));
    runner.menu.state.editing = true;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": -1, "id": "main" }),
        })
        .unwrap();
    assert!(!runner.part_auto_names[3]);
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
        })
        .unwrap();
    assert!(runner.part_auto_names[3]);

    assert!(runner.menu.focus_item_key("behaviorId"));
    runner.menu.state.editing = true;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": -99, "id": "main" }),
        })
        .unwrap();
    assert_eq!(
        runner.menu.value_for_key("behaviorId").as_deref(),
        Some("none")
    );

    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_a", "pressed": true }),
        })
        .unwrap();

    assert_eq!(runner.part_behavior_ids[3], "none");
    assert!(runner.part_auto_names[3]);
    assert_eq!(runner.part_names[3], "none");
    assert_eq!(
        runner.menu.value_for_key("parts.3.name").as_deref(),
        Some("none")
    );
}

#[test]
fn auto_name_updates_when_engine_behavior_already_matches_menu_value() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.part_behavior_ids[3] = "sequencer".into();
    runner.part_names[3] = "sequencer".into();
    runner.part_auto_names[3] = true;
    runner.select_active_part(3).unwrap();
    runner
        .rebuild_engine(platform_core::get_native_behavior("none").unwrap())
        .unwrap();
    runner.behavior = platform_core::get_native_behavior("none").unwrap();
    runner.menu.rebuild(runner.menu_config());

    assert!(runner.menu.focus_item_key("behaviorId"));
    runner.menu.state.editing = true;
    assert_eq!(
        runner.menu.value_for_key("behaviorId").as_deref(),
        Some("none")
    );
    runner.apply_or_schedule_menu_key("behaviorId").unwrap();
    runner.make_deferred_menu_apply_due_for_test();
    runner.flush_deferred_menu_apply().unwrap();

    assert_eq!(runner.part_behavior_ids[3], "none");
    assert_eq!(runner.part_names[3], "none");
    assert_eq!(
        runner.menu.value_for_key("parts.3.name").as_deref(),
        Some("none")
    );
}

#[test]
fn part_four_auto_name_change_is_in_deferred_autosave_payload() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.auto_save_default = true;
    runner.part_behavior_ids[3] = "sequencer".into();
    runner.part_names[3] = "sequencer".into();
    runner.part_auto_names[3] = true;
    runner.select_active_part(3).unwrap();
    runner.menu.rebuild(runner.menu_config());

    assert!(runner.menu.focus_item_key("behaviorId"));
    runner.menu.state.editing = true;
    runner.menu.turn_key("behaviorId", -99);
    runner.apply_or_schedule_menu_key("behaviorId").unwrap();

    let messages = runner.flush_pending_deferred_work_now().unwrap();
    let saved_payload = messages
        .iter()
        .find_map(|message| match message {
            RunnerMessage::PlatformEffects { effects } => {
                effects.iter().find_map(|effect| match effect {
                    RuntimePlatformEffect::StoreSaveDefault { payload, mode }
                        if mode.as_deref() == Some("deferred") =>
                    {
                        Some(payload)
                    }
                    _ => None,
                })
            }
            _ => None,
        })
        .expect("part 4 deferred autosave payload");

    assert_eq!(runner.part_behavior_ids[3], "none");
    assert_eq!(runner.part_names[3], "none");
    assert_eq!(
        saved_payload["runtimeConfig"]["parts"][3]["l1"]["behaviorId"],
        "none"
    );
    assert_eq!(saved_payload["runtimeConfig"]["parts"][3]["name"], "none");
    assert_eq!(saved_payload["runtimeConfig"]["parts"][3]["autoName"], true);
}

#[test]
fn turning_part_auto_name_on_replaces_manual_name() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.part_behavior_ids[1] = "sequencer".into();
    runner.part_names[1] = "manual name".into();
    runner.part_auto_names[1] = false;
    runner.select_active_part(1).unwrap();
    runner.menu.rebuild(runner.menu_config());

    runner.menu.turn_key("parts.1.autoName", 1);
    runner.apply_menu_state().unwrap();

    assert!(runner.part_auto_names[1]);
    assert_eq!(runner.part_names[1], "sequencer");
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["parts"][1]["name"],
        "sequencer"
    );
}

#[test]
fn loading_auto_named_part_ignores_stale_payload_name() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();

    runner
        .apply_config_payload(json!({
            "runtimeConfig": {
                "activePartIndex": 1,
                "parts": [
                    { "l1": { "behaviorId": "life" }, "autoName": true, "name": "life" },
                    { "l1": { "behaviorId": "none" }, "autoName": true, "name": "sequencer" }
                ]
            }
        }))
        .unwrap();

    assert_eq!(runner.part_behavior_ids[1], "none");
    assert!(runner.part_auto_names[1]);
    assert_eq!(runner.part_names[1], "none");
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["parts"][1]["name"],
        "none"
    );
}

#[test]
fn turning_instrument_and_bus_auto_name_on_replaces_manual_names() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].kind = "sampler".into();
    runner.instruments[0].name = "manual inst".into();
    runner.instruments[0].auto_name = false;
    runner.fx_buses[0].slot1_type = "delay".into();
    runner.fx_buses[0].slot2_type = "duck".into();
    runner.fx_buses[0].name = "manual bus".into();
    runner.fx_buses[0].auto_name = false;
    runner.menu.rebuild(runner.menu_config());

    runner.menu.turn_key("instruments.0.autoName", 1);
    runner.menu.turn_key("mixer.buses.0.autoName", 1);
    runner.apply_menu_state().unwrap();

    assert!(runner.instruments[0].auto_name);
    assert_eq!(runner.instruments[0].name, "Sampler");
    assert!(runner.fx_buses[0].auto_name);
    assert_eq!(runner.fx_buses[0].name, "Delay+Duck");
}
