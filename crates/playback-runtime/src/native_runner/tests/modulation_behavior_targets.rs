use super::*;

#[test]
pub(crate) fn behavior_target_picker_uses_each_layers_behavior_and_prunes_none() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.layer_behavior_ids[0] = "none".into();
    runner.layer_behavior_ids[2] = "brain".into();
    runner.layer_behavior_configs[2] = json!({ "randomSeedCells": 4 });

    let config = runner.menu_config();

    assert!(config.behavior_target_items[0].is_empty());
    assert!(contains_key_recursive(
        &config.behavior_target_items[2],
        "layers.2.algorithmStep"
    ));
    assert!(contains_key_recursive(
        &config.behavior_target_items[2],
        "layers.2.worlds.behaviorConfig.randomSeedCells"
    ));
    assert!(!contains_key_recursive(
        &config.behavior_target_items[2],
        "layers.0.worlds.behaviorConfig.randomCellsPerTick"
    ));
}

#[test]
pub(crate) fn build_layer_menu_uses_selected_layers_behavior_params() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.layer_behavior_ids[2] = "brain".into();
    runner.layer_names[2] = "brain".into();
    runner.layer_behavior_configs[2] = json!({ "randomSeedCells": 4 });
    runner.menu.rebuild(runner.menu_config());

    let _ = runner.menu.press();
    runner.menu.state.cursor = 2;
    let _ = runner.menu.press();
    let snapshot = runner.menu.snapshot();

    assert_eq!(snapshot.path, "/B/L3: brain");
    assert!(snapshot
        .lines
        .iter()
        .any(|line| line.contains("Fire Threshold")));
    assert!(snapshot
        .lines
        .iter()
        .any(|line| line.contains("Seed Interval")));
    assert!(!snapshot
        .lines
        .iter()
        .any(|line| line.contains("Spawn Interval")));
}

#[test]
pub(crate) fn aux_turn_generated_per_layer_behavior_targets_updates_stored_config() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.layer_behavior_ids[2] = "brain".into();
    runner.layer_behavior_configs[2] = json!({ "randomSeedCells": 4 });
    runner.transport.layer_algorithm_step_pulses[2] = 24;
    runner.aux_bindings[0] = Some(NativeAuxBinding {
        turn_key: Some("layers.2.algorithmStep".into()),
        press_action: None,
    });
    runner.aux_bindings[1] = Some(NativeAuxBinding {
        turn_key: Some("layers.2.worlds.behaviorConfig.randomSeedCells".into()),
        press_action: None,
    });

    runner.handle_aux_turn(0, 1).unwrap();
    runner.handle_aux_turn(1, 1).unwrap();

    assert_eq!(runner.transport.layer_algorithm_step_pulses[2], 32);
    assert_eq!(runner.layer_behavior_configs[2]["randomSeedCells"], 5);
    assert!(runner.config_dirty);
    assert!(runner.pending.pending_autosave_payload_due_at.is_some());
}

#[test]
pub(crate) fn keyed_layer_algorithm_step_rebases_held_discrete_source() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "life".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    let key = "algorithmStep";
    runner.active_sparks_mode = "xy".into();
    runner.xy_touch = NativeXyTouch {
        x: 1.0,
        y: 0.5,
        display_x: 1.0,
        display_y: 0.5,
        active: true,
    };
    runner.xy_x_binding = Some(NativeParamBinding {
        key: key.into(),
        label: Some("Step Rate".into()),
        kind: "enum".into(),
        min: None,
        max: None,
        step: None,
        user_min: None,
        user_max: None,
        options: crate::timing_units::NOTE_UNIT_OPTIONS
            .iter()
            .copied()
            .map(String::from)
            .collect(),
        invert: false,
    });
    runner.refresh_xy_runtime_sources();
    runner.process_dirty_modulation_step(false).unwrap();
    let held_step = crate::timing_units::note_unit_to_pulses("1/1");
    assert_eq!(runner.transport.algorithm_step_pulses, held_step);

    assert!(runner.menu.focus_item_key(key));
    runner.menu.state.editing = true;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    let rebased_base =
        crate::timing_units::note_unit_to_pulses(&runner.menu.value_for_key(key).unwrap());
    assert_eq!(runner.transport.algorithm_step_pulses, held_step);

    runner.set_param_binding_target("xy:x", None);
    assert_eq!(runner.transport.algorithm_step_pulses, rebased_base);
}

#[test]
pub(crate) fn unchanged_modulation_avoids_state_serialization_rebuild_and_autosave() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.select_layer_behavior(1, "brain").unwrap();
    runner.layer_behavior_configs[1] = json!({ "randomSeedCells": 4 });
    runner.xy_touch = NativeXyTouch {
        x: 0.5,
        y: 0.2,
        display_x: 0.5,
        display_y: 0.2,
        active: true,
    };
    runner.xy_y_binding = Some(NativeParamBinding {
        key: "layers.1.worlds.behaviorConfig.randomSeedCells".into(),
        label: Some("Spawn Count".into()),
        kind: "number".into(),
        min: Some(0.0),
        max: Some(20.0),
        step: Some(1.0),
        user_min: None,
        user_max: None,
        options: vec![],
        invert: false,
    });
    runner.config_dirty = false;
    runner.pending.pending_autosave_payload_due_at = None;
    runner.behavior_state_serialization_calls.set(0);
    runner.layer_behavior_rebuilds = 0;
    runner.fast_autosave_marks = 0;

    for _ in 0..8 {
        runner.apply_runtime_modulation(&[], 0);
    }

    assert_eq!(runner.layer_behavior_configs[1]["randomSeedCells"], 4);
    assert!(!runner.config_dirty);
    assert!(runner.pending.pending_autosave_payload_due_at.is_none());
    assert_eq!(runner.behavior_state_serialization_calls.get(), 0);
    assert_eq!(runner.layer_behavior_rebuilds, 0);
    assert_eq!(runner.fast_autosave_marks, 0);
}

#[test]
pub(crate) fn per_layer_step_rate_xy_binding_round_trips_from_payload() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["xy"]["x"] = json!({
        "key": "layers.2.algorithmStep",
        "label": "Step Rate",
        "kind": "enum",
        "options": ["1/32T", "1/32", "1/16T", "1/16", "1/8T", "1/8", "1/4T", "1/4", "1/2T", "1/2", "1/1T", "1/1"]
    });

    runner.apply_config_payload(payload).unwrap();

    assert_eq!(
        runner.xy_x_binding.as_ref().unwrap().key,
        "layers.2.algorithmStep"
    );
}

#[test]
pub(crate) fn stale_bindings_to_none_behavior_layer_do_not_mutate_hidden_values() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.layer_behavior_ids[2] = "none".into();
    runner.transport.layer_algorithm_step_pulses[2] = 24;
    runner.layer_behavior_configs[2] = json!({ "randomSeedCells": 4 });
    runner.xy_touch = NativeXyTouch {
        x: 1.0,
        y: 1.0,
        display_x: 1.0,
        display_y: 1.0,
        active: true,
    };
    runner.xy_x_binding = Some(NativeParamBinding {
        key: "layers.2.algorithmStep".into(),
        label: Some("Step Rate".into()),
        kind: "enum".into(),
        min: None,
        max: None,
        step: None,
        user_min: None,
        user_max: None,
        options: crate::timing_units::NOTE_UNIT_OPTIONS
            .iter()
            .copied()
            .map(String::from)
            .collect(),
        invert: false,
    });
    runner.xy_y_binding = Some(NativeParamBinding {
        key: "layers.2.worlds.behaviorConfig.randomSeedCells".into(),
        label: Some("Spawn Count".into()),
        kind: "number".into(),
        min: Some(0.0),
        max: Some(20.0),
        step: Some(1.0),
        user_min: None,
        user_max: None,
        options: vec![],
        invert: false,
    });

    runner.apply_runtime_modulation(&[], 0);

    assert_eq!(runner.transport.layer_algorithm_step_pulses[2], 24);
    assert_eq!(runner.layer_behavior_configs[2]["randomSeedCells"], 4);
    assert!(!runner.config_dirty);
}

pub(crate) fn contains_key_recursive(
    items: &[crate::native_menu::NativeMenuItem],
    key: &str,
) -> bool {
    items
        .iter()
        .any(|item| item.key.as_deref() == Some(key) || contains_key_recursive(&item.children, key))
}
