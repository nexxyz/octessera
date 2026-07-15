use super::*;

#[test]
pub(crate) fn aux_binding_payload_follows_platform_aux_encoder_count() {
    let runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let payload = runner.config_payload();
    let bindings = payload["runtimeConfig"]["auxBindings"].as_object().unwrap();

    assert!(bindings.contains_key("aux1"));
    assert!(bindings.contains_key("aux2"));
    assert!(bindings.contains_key("aux3"));
    assert!(!bindings.contains_key("aux4"));
}

#[test]
pub(crate) fn aux_auto_map_config_load_disables_automatic_bindings() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["auxAutoMapEnabled"] = json!(false);

    runner.apply_config_payload(payload).unwrap();
    runner.menu.state.stack = synth_stack(&runner, "Filter");
    runner.menu.state.cursor = 1;

    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "id": "aux1", "delta": 1 }),
            request_snapshot: None,
        })
        .unwrap();

    assert_eq!(
        runner.snapshot().unwrap()["settings"]["auxAutoMapEnabled"],
        false
    );
    assert_eq!(
        runner
            .menu
            .number_for_key("instruments.0.synth.filter.cutoffHz"),
        Some(222)
    );
    assert_eq!(runner.toast.as_ref().unwrap().message, "Trn-1: No binding");
}

#[test]
pub(crate) fn auto_map_updates_synth_filter_and_prefixes_selected_row() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.state.stack = synth_stack(&runner, "Filter");
    runner.menu.state.cursor = 1;

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "id": "aux1", "delta": 1 }),
            request_snapshot: None,
        })
        .unwrap();
    let snapshot = snapshot_from(&messages);

    assert_eq!(
        runner
            .menu
            .number_for_key("instruments.0.synth.filter.cutoffHz"),
        Some(223)
    );
    assert!(snapshot["display"]["lines"]
        .as_array()
        .unwrap()
        .iter()
        .any(|line| line.as_str().unwrap_or("").starts_with("> 1-Cutoff")));
    assert!(snapshot["display"]["lines"]
        .as_array()
        .unwrap()
        .iter()
        .any(|line| line.as_str().unwrap_or("").contains("2-Res")));
    assert!(snapshot["display"]["lines"]
        .as_array()
        .unwrap()
        .iter()
        .any(|line| line.as_str().unwrap_or("").contains("3-Env")));
    assert!(snapshot["display"]["lines"]
        .as_array()
        .unwrap()
        .iter()
        .any(|line| line.as_str().unwrap_or("").contains("4-Key")));
}

#[test]
pub(crate) fn auto_map_press_enters_sample_assign_and_prefixes_assign_action() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].kind = "sampler".into();
    runner.menu.rebuild(runner.menu_config());
    runner.menu.state.stack = vec![2, 0, 0, 2];
    runner.menu.state.cursor = 3;

    let opened = runner.messages_with_snapshot().unwrap();
    assert!(snapshot_from(&opened)["display"]["lines"]
        .as_array()
        .unwrap()
        .iter()
        .any(|line| line.as_str().unwrap_or("") == "> 1!Assign"));

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "aux1" }),
            request_snapshot: None,
        })
        .unwrap();

    assert_eq!(runner.sample_assign, Some((0, 0)));
}

#[test]
pub(crate) fn auto_map_is_disabled_in_pulses_and_unbound_toast_uses_short_format() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.state.stack = vec![1];
    runner.menu.state.cursor = 0;

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "id": "aux1", "delta": 1 }),
            request_snapshot: None,
        })
        .unwrap();

    assert_eq!(runner.toast.as_ref().unwrap().message, "Trn-1: No binding");
}

#[test]
pub(crate) fn disabled_auto_map_suppresses_display_prefixes() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.aux_auto_map_enabled = false;
    runner.menu.rebuild(runner.menu_config());
    runner.menu.state.stack = synth_stack(&runner, "Filter");
    runner.menu.state.cursor = 1;

    let messages = runner.messages_with_snapshot().unwrap();
    assert!(snapshot_from(&messages)["display"]["lines"]
        .as_array()
        .unwrap()
        .iter()
        .all(|line| !line.as_str().unwrap_or_default().contains("1-Cutoff")));
}

#[test]
pub(crate) fn custom_aux_binding_still_works_when_auto_map_is_disabled() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.aux_auto_map_enabled = false;
    runner.aux_bindings[0] = Some(NativeAuxBinding {
        turn_key: Some("masterVolume".into()),
        press_action: None,
    });

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "id": "aux1", "delta": -1 }),
            request_snapshot: None,
        })
        .unwrap();

    assert_eq!(runner.ui.master_volume, 72);
}

#[test]
pub(crate) fn custom_aux_binding_overrides_auto_map_when_enabled() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.state.stack = synth_stack(&runner, "Filter");
    runner.menu.state.cursor = 0;
    runner.aux_bindings[0] = Some(NativeAuxBinding {
        turn_key: Some("masterVolume".into()),
        press_action: None,
    });

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "id": "aux1", "delta": -1 }),
            request_snapshot: None,
        })
        .unwrap();

    assert_eq!(runner.ui.master_volume, 72);
    assert_eq!(
        runner
            .menu
            .number_for_key("instruments.0.synth.filter.cutoffHz"),
        Some(222)
    );
}

#[test]
pub(crate) fn custom_binding_is_used_on_worlds_non_mapped_rows_even_when_auto_map_is_enabled() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.state.stack = vec![0, 0];
    runner.menu.state.cursor = 0;
    runner.aux_bindings[0] = Some(NativeAuxBinding {
        turn_key: Some("masterVolume".into()),
        press_action: None,
    });

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "id": "aux1", "delta": -1 }),
            request_snapshot: None,
        })
        .unwrap();

    assert_eq!(runner.ui.master_volume, 72);
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["layers"][0]["worlds"]["stepRate"],
        "1/8"
    );
}

#[test]
pub(crate) fn auto_map_worlds_life_turn_and_press_follow_behavior_context() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let life_items = &runner.menu.root.children[0].children[0].children;
    let interval_cursor = child_index_by_key(
        life_items,
        "layers.0.worlds.behaviorConfig.randomTickInterval",
    );
    runner.menu.state.stack = vec![0, 0];
    runner.menu.state.cursor = interval_cursor;

    let before = runner
        .engine
        .model()
        .unwrap()
        .cells
        .iter()
        .filter(|cell| **cell)
        .count();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "id": "aux1", "delta": 1 }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["layers"][0]["worlds"]["stepRate"],
        "1/4T"
    );

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "id": "aux2", "delta": 1 }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(runner.behavior_config["randomCellsPerTick"], 1);

    let spawn_aux_index = (0..runner.aux_bindings.len())
        .find(|index| {
            runner
                .effective_aux_slot(*index)
                .press
                .as_ref()
                .map(|press| {
                    matches!(
                        press.action,
                        NativeMenuAction::BehaviorAction(ref action) if action == "spawnRandom"
                    )
                })
                .unwrap_or(false)
        })
        .unwrap();

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": format!("aux{}", spawn_aux_index + 1) }),
            request_snapshot: None,
        })
        .unwrap();
    let after = runner
        .engine
        .model()
        .unwrap()
        .cells
        .iter()
        .filter(|cell| **cell)
        .count();

    assert!(after >= before);
    assert_eq!(
        runner.toast.as_ref().unwrap().message,
        format!("Clk-{}: Spawn", spawn_aux_index + 1)
    );
}
