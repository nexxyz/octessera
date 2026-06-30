use super::*;

#[test]
pub(crate) fn button_back_commits_l1_behavior_auto_name_after_manual_auto_name_toggle() {
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
pub(crate) fn exact_desktop_flow_renames_p4_after_auto_name_toggle_and_behavior_change() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.part_behavior_ids[3] = "sequencer".into();
    runner.part_names[3] = "sequencer".into();
    runner.part_auto_names[3] = true;
    runner.menu.rebuild(runner.menu_config());

    send_encoder_press(&mut runner);
    send_encoder_turn(&mut runner, 1);
    send_encoder_turn(&mut runner, 1);
    send_encoder_turn(&mut runner, 1);
    send_encoder_press(&mut runner);
    send_encoder_turn(&mut runner, 1);
    send_encoder_press(&mut runner);
    send_encoder_turn(&mut runner, -1);
    send_encoder_press(&mut runner);
    send_encoder_press(&mut runner);
    send_encoder_turn(&mut runner, 1);
    send_encoder_press(&mut runner);
    send_encoder_turn(&mut runner, -1);
    send_encoder_press(&mut runner);
    send_encoder_turn(&mut runner, -1);
    send_encoder_turn(&mut runner, -1);
    send_encoder_press(&mut runner);
    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_a", "pressed": true }),
        })
        .unwrap();
    let snapshot = snapshot_from(&messages);

    assert_eq!(runner.part_behavior_ids[3], "none");
    assert_eq!(runner.part_names[3], "none");
    assert_eq!(snapshot["display"]["title"], "L1: Life");
    assert!(snapshot["display"]["lines"]
        .as_array()
        .unwrap()
        .iter()
        .any(|line| line.as_str().is_some_and(|line| line.contains("P4: none"))));
}

pub(crate) fn send_encoder_turn(runner: &mut NativeRunner, delta: i32) -> Vec<RunnerMessage> {
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": delta, "id": "main" }),
        })
        .unwrap()
}

pub(crate) fn send_encoder_press(runner: &mut NativeRunner) -> Vec<RunnerMessage> {
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap()
}

#[test]
pub(crate) fn auto_name_updates_when_engine_behavior_already_matches_menu_value() {
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
pub(crate) fn part_four_auto_name_change_is_in_deferred_autosave_payload() {
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
pub(crate) fn turning_part_auto_name_on_replaces_manual_name() {
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
pub(crate) fn loading_auto_named_part_ignores_stale_payload_name() {
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
