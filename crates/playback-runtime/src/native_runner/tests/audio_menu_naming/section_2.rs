use super::*;

#[test]
pub(crate) fn button_back_commits_build_behavior_auto_name_after_manual_auto_name_toggle() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.layers[3].behavior_id = "sequencer".into();
    runner.layers[3].name = "sequencer".into();
    runner.layers[3].auto_name = true;
    runner.select_active_layer(3).unwrap();
    runner.menu.rebuild(runner.menu_config());

    assert!(runner.menu.focus_item_key("layers.3.autoName"));
    runner.menu.state.editing = true;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": -1, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    assert!(!runner.layers[3].auto_name);
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    assert!(runner.layers[3].auto_name);

    select_behavior(&mut runner, "none");

    assert_eq!(runner.layers[3].behavior_id, "none");
    assert!(runner.layers[3].auto_name);
    assert_eq!(runner.layers[3].name, "none");
    assert_eq!(
        runner.menu.value_for_key("layers.3.name").as_deref(),
        Some("none")
    );
}

#[test]
pub(crate) fn exact_desktop_flow_renames_p4_after_auto_name_toggle_and_behavior_change() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.layers[3].behavior_id = "sequencer".into();
    runner.layers[3].name = "sequencer".into();
    runner.layers[3].auto_name = true;
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
    select_behavior(&mut runner, "none");
    let messages = runner.messages_with_snapshot().unwrap();
    let snapshot = snapshot_from(&messages);

    assert_eq!(runner.layers[3].behavior_id, "none");
    assert_eq!(runner.layers[3].name, "none");
    assert_eq!(snapshot["display"]["title"], "/B/L4: none");
    assert_eq!(runner.layers[3].name, "none");
}

pub(crate) fn send_encoder_turn(runner: &mut NativeRunner, delta: i32) -> Vec<RunnerMessage> {
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": delta, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap()
}

pub(crate) fn send_encoder_press(runner: &mut NativeRunner) -> Vec<RunnerMessage> {
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap()
}

#[test]
pub(crate) fn auto_name_updates_when_engine_behavior_already_matches_menu_value() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.layers[3].behavior_id = "sequencer".into();
    runner.layers[3].name = "sequencer".into();
    runner.layers[3].auto_name = true;
    runner.select_active_layer(3).unwrap();
    runner
        .rebuild_engine(platform_core::get_native_behavior("none").unwrap())
        .unwrap();
    runner.menu.rebuild(runner.menu_config());

    select_behavior(&mut runner, "none");

    assert_eq!(runner.layers[3].behavior_id, "none");
    assert_eq!(runner.layers[3].name, "none");
    assert_eq!(
        runner.menu.value_for_key("layers.3.name").as_deref(),
        Some("none")
    );
}

#[test]
pub(crate) fn layer_four_auto_name_change_is_in_deferred_autosave_payload() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.auto_save_default = true;
    runner.layers[3].behavior_id = "sequencer".into();
    runner.layers[3].name = "sequencer".into();
    runner.layers[3].auto_name = true;
    runner.select_active_layer(3).unwrap();
    runner.menu.rebuild(runner.menu_config());

    select_behavior(&mut runner, "none");

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
        .expect("layer 4 deferred autosave payload");

    assert_eq!(runner.layers[3].behavior_id, "none");
    assert_eq!(runner.layers[3].name, "none");
    assert_eq!(
        saved_payload["runtimeConfig"]["layers"][3]["build"]["behaviorId"],
        "none"
    );
    assert_eq!(saved_payload["runtimeConfig"]["layers"][3]["name"], "none");
    assert_eq!(
        saved_payload["runtimeConfig"]["layers"][3]["autoName"],
        true
    );
}

#[test]
pub(crate) fn turning_layer_auto_name_on_replaces_manual_name() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.layers[1].behavior_id = "sequencer".into();
    runner.layers[1].name = "manual name".into();
    runner.layers[1].auto_name = false;
    runner.select_active_layer(1).unwrap();
    runner.menu.rebuild(runner.menu_config());

    runner.edit_menu_key("layers.1.autoName", 1).unwrap();

    assert!(runner.layers[1].auto_name);
    assert_eq!(runner.layers[1].name, "sequencer");
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["layers"][1]["name"],
        "sequencer"
    );
}

#[test]
pub(crate) fn loading_auto_named_layer_ignores_stale_payload_name() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();

    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["activeLayerIndex"] = json!(1);
    payload["runtimeConfig"]["layers"][1]["build"] = json!({ "behaviorId": "none" });
    payload["runtimeConfig"]["layers"][1]["autoName"] = json!(true);
    payload["runtimeConfig"]["layers"][1]["name"] = json!("sequencer");
    runner.apply_config_payload(payload).unwrap();

    assert_eq!(runner.layers[1].behavior_id, "none");
    assert!(runner.layers[1].auto_name);
    assert_eq!(runner.layers[1].name, "none");
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["layers"][1]["name"],
        "none"
    );
}
