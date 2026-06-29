use super::*;

#[test]
fn behavior_target_picker_uses_each_parts_behavior_and_prunes_none() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.part_behavior_ids[0] = "none".into();
    runner.part_behavior_ids[2] = "brain".into();
    runner.part_behavior_configs[2] = json!({ "randomSeedCells": 4 });

    let config = runner.menu_config();

    assert!(config.behavior_target_items[0].is_empty());
    assert!(contains_key_recursive(
        &config.behavior_target_items[2],
        "parts.2.algorithmStep"
    ));
    assert!(contains_key_recursive(
        &config.behavior_target_items[2],
        "parts.2.l1.behaviorConfig.randomSeedCells"
    ));
    assert!(!contains_key_recursive(
        &config.behavior_target_items[2],
        "parts.0.l1.behaviorConfig.randomCellsPerTick"
    ));
}

#[test]
fn aux_turn_generated_per_part_behavior_targets_updates_stored_config() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.part_behavior_ids[2] = "brain".into();
    runner.part_behavior_configs[2] = json!({ "randomSeedCells": 4 });
    runner.part_algorithm_step_pulses[2] = 24;
    runner.aux_bindings[0] = Some(NativeAuxBinding {
        turn_key: Some("parts.2.algorithmStep".into()),
        press_action: None,
    });
    runner.aux_bindings[1] = Some(NativeAuxBinding {
        turn_key: Some("parts.2.l1.behaviorConfig.randomSeedCells".into()),
        press_action: None,
    });

    runner.handle_aux_turn(0, 1).unwrap();
    runner.handle_aux_turn(1, 1).unwrap();

    assert_eq!(runner.part_algorithm_step_pulses[2], 48);
    assert_eq!(runner.part_behavior_configs[2]["randomSeedCells"], 5);
    assert!(runner.config_dirty);
}

#[test]
fn per_part_step_rate_xy_binding_round_trips_from_payload() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["parts"][0]["xy"]["x"] = json!({
        "key": "parts.2.algorithmStep",
        "label": "Step Rate",
        "kind": "enum",
        "options": ["1/16", "1/8", "1/4", "1/2", "1/1"]
    });

    runner.apply_config_payload(payload).unwrap();

    assert_eq!(
        runner.xy_x_binding.as_ref().unwrap().key,
        "parts.2.algorithmStep"
    );
}

#[test]
fn stale_bindings_to_none_behavior_part_do_not_mutate_hidden_values() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.part_behavior_ids[2] = "none".into();
    runner.part_algorithm_step_pulses[2] = 24;
    runner.part_behavior_configs[2] = json!({ "randomSeedCells": 4 });
    runner.xy_touch = NativeXyTouch {
        x: 1.0,
        y: 1.0,
        display_x: 1.0,
        display_y: 1.0,
        active: true,
    };
    runner.xy_x_binding = Some(NativeParamBinding {
        key: "parts.2.algorithmStep".into(),
        label: Some("Step Rate".into()),
        kind: "enum".into(),
        min: None,
        max: None,
        step: None,
        options: vec!["1/16", "1/8", "1/4", "1/2", "1/1"]
            .into_iter()
            .map(String::from)
            .collect(),
        invert: false,
    });
    runner.xy_y_binding = Some(NativeParamBinding {
        key: "parts.2.l1.behaviorConfig.randomSeedCells".into(),
        label: Some("Spawn Count".into()),
        kind: "number".into(),
        min: Some(0.0),
        max: Some(20.0),
        step: Some(1.0),
        options: vec![],
        invert: false,
    });

    runner.apply_runtime_modulation(&[], 0);

    assert_eq!(runner.part_algorithm_step_pulses[2], 24);
    assert_eq!(runner.part_behavior_configs[2]["randomSeedCells"], 4);
    assert!(!runner.config_dirty);
}

fn contains_key_recursive(items: &[crate::native_menu::NativeMenuItem], key: &str) -> bool {
    items
        .iter()
        .any(|item| item.key.as_deref() == Some(key) || contains_key_recursive(&item.children, key))
}
