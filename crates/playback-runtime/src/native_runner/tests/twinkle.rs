use super::*;

fn twinkle_runner() -> NativeRunner {
    NativeRunner::new(NativeRunnerConfig {
        behavior_id: "twinkle".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap()
}

fn state_cells(runner: &NativeRunner) -> Vec<bool> {
    runner.engine.serialized_state().unwrap()["cells"]
        .as_array()
        .unwrap()
        .iter()
        .map(|cell| cell.as_bool().unwrap())
        .collect()
}

#[test]
pub(crate) fn twinkle_menu_exposes_native_defaults_and_actions() {
    let runner = twinkle_runner();
    let prefix = "layers.0.worlds.behaviorConfig";
    assert_eq!(
        runner.menu.number_for_key(&format!("{prefix}.density")),
        Some(3)
    );
    assert_eq!(
        runner
            .menu
            .number_for_key(&format!("{prefix}.birthChancePct")),
        Some(70)
    );
    assert_eq!(
        runner
            .menu
            .number_for_key(&format!("{prefix}.fadeChancePct")),
        Some(35)
    );
    assert_eq!(
        runner.menu.number_for_key(&format!("{prefix}.starLife")),
        Some(8)
    );
    assert_eq!(
        runner
            .menu
            .number_for_key(&format!("{prefix}.clusterBiasPct")),
        Some(40)
    );
    assert_eq!(
        runner.menu.number_for_key(&format!("{prefix}.seed")),
        Some(1)
    );
    assert!(matches!(
        runner.menu.item_for_key(&format!("{prefix}.reseedStars")).unwrap().value,
        crate::native_menu::NativeMenuValue::Action(
            crate::native_menu::NativeMenuAction::BehaviorAction(ref action)
        ) if action == "reseedStars"
    ));
    assert!(runner
        .menu
        .item_for_key(&format!("{prefix}.clearStars"))
        .is_some());
    assert_eq!(
        runner.menu.item_for_key("behaviorId").unwrap().label,
        "Behavior: twinkle"
    );
}

#[test]
pub(crate) fn twinkle_layer_selection_and_keyed_edit_stay_targeted() {
    let mut runner = twinkle_runner();
    runner.select_layer_behavior(1, "twinkle").unwrap();
    assert_eq!(runner.layer_behavior_ids[1], "twinkle");
    assert!(runner
        .menu
        .item_for_key("layers.1.worlds.behaviorConfig.density")
        .is_some());

    let active_before = runner.engine.serialized_state().unwrap();
    let key = "layers.1.worlds.behaviorConfig.density";
    assert!(runner.menu.focus_item_key(key));
    runner.menu.state.editing = true;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({"type": "encoder_turn", "id": "main", "delta": 1}),
            request_snapshot: None,
        })
        .unwrap();

    assert_eq!(runner.engine.serialized_state().unwrap(), active_before);
    assert_eq!(runner.layer_behavior_configs[1]["density"], 4);
    assert!(runner.config_dirty);
}

#[test]
pub(crate) fn twinkle_actions_reset_aux_and_autosave_round_trip() {
    let mut runner = twinkle_runner();
    runner
        .execute_menu_action(crate::native_menu::NativeMenuAction::BehaviorAction(
            "clearStars".into(),
        ))
        .unwrap();
    assert_eq!(state_cells(&runner).iter().filter(|cell| **cell).count(), 0);
    assert!(runner.pending.pending_autosave_payload_due_at.is_some());

    runner.config_dirty = false;
    runner
        .menu
        .focus_item_key("layers.0.worlds.behaviorConfig.density");
    let slot = runner.effective_aux_slot(2);
    assert!(slot.turn.is_none());
    assert!(matches!(
        slot.press.map(|press| press.action),
        Some(crate::native_menu::NativeMenuAction::BehaviorAction(action))
            if action == "reseedStars"
    ));
    runner.handle_aux_press(2).unwrap();
    assert_eq!(state_cells(&runner).iter().filter(|cell| **cell).count(), 3);
    assert!(runner.config_dirty);

    runner
        .execute_menu_action(crate::native_menu::NativeMenuAction::ResetBehavior)
        .unwrap();
    assert_eq!(state_cells(&runner).iter().filter(|cell| **cell).count(), 3);
    let payload = runner.config_payload();
    let expected = runner.engine.serialized_state().unwrap();
    let mut restored = twinkle_runner();
    restored.apply_config_payload(payload).unwrap();
    assert_eq!(restored.engine.serialized_state().unwrap(), expected);
}

#[test]
pub(crate) fn twinkle_help_round_trip_resolves_menu_keys_and_actions() {
    let runner = twinkle_runner();
    let targets = runner.menu.help_targets();
    for key in [
        "key:layers.*.worlds.behaviorConfig.density",
        "key:layers.*.worlds.behaviorConfig.seed",
        "action:behavior_action:reseedStars",
        "action:behavior_action:clearStars",
        "action:behavior_select:twinkle",
    ] {
        let target = targets.iter().find(|target| target.key == key).unwrap();
        let help = crate::native_help::resolve_native_help(target).unwrap();
        assert!(!help.title.is_empty());
        assert!(!help.detail.is_empty());
    }
}
