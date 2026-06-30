use super::*;

#[test]
pub(crate) fn menu_binding_actions_update_param_xy_and_aux_targets() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let binding = NativeParamBindingSpec {
        key: "instruments.0.mixer.volume".into(),
        label: Some("Volume".into()),
        kind: "number".into(),
        min: Some(0),
        max: Some(100),
        step: Some(1),
        options: vec![],
        invert: false,
    };

    runner
        .execute_menu_action(NativeMenuAction::SetParamBinding {
            target: "param:0:x:0".into(),
            binding: binding.clone(),
        })
        .unwrap();
    runner
        .execute_menu_action(NativeMenuAction::SetParamBinding {
            target: "xy:x".into(),
            binding: binding.clone(),
        })
        .unwrap();
    runner
        .execute_menu_action(NativeMenuAction::SetParamBinding {
            target: "aux:0:turn".into(),
            binding,
        })
        .unwrap();

    assert_eq!(
        runner.param_mods[0].x[0].as_ref().unwrap().key,
        "instruments.0.mixer.volume"
    );
    assert_eq!(
        runner.xy_x_binding.as_ref().unwrap().key,
        "instruments.0.mixer.volume"
    );
    assert_eq!(
        runner.aux_bindings[0]
            .as_ref()
            .and_then(|binding| binding.turn_key.as_deref()),
        Some("instruments.0.mixer.volume")
    );

    runner
        .execute_menu_action(NativeMenuAction::SetAuxClick {
            index: 0,
            action: Some(Box::new(NativeMenuAction::PlatformEffect(
                "sample.assign:0:0".into(),
            ))),
        })
        .unwrap();
    assert!(matches!(
        runner.aux_bindings[0].as_ref().and_then(|binding| binding.press_action.as_ref()),
        Some(NativeMenuAction::PlatformEffect(action)) if action == "sample.assign:0:0"
    ));
}

#[test]
pub(crate) fn representative_selector_actions_update_sense_aux_and_dance_xy_bindings() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let binding = NativeParamBindingSpec {
        key: "mixer.buses.0.slot1.params.mixPct".into(),
        label: Some("Mix".into()),
        kind: "number".into(),
        min: Some(0),
        max: Some(100),
        step: Some(1),
        options: vec![],
        invert: false,
    };

    for target in ["param:0:x:0", "aux:1:turn", "xy:y"] {
        runner
            .execute_menu_action(NativeMenuAction::SetParamBinding {
                target: target.into(),
                binding: binding.clone(),
            })
            .unwrap();
    }

    assert_eq!(runner.param_mods[0].x[0].as_ref().unwrap().key, binding.key);
    assert_eq!(
        runner.aux_bindings[1]
            .as_ref()
            .and_then(|binding| binding.turn_key.as_deref()),
        Some(binding.key.as_str())
    );
    assert_eq!(runner.xy_y_binding.as_ref().unwrap().key, binding.key);
}

pub(crate) fn find_item_by_key<'a>(
    item: &'a crate::native_menu::NativeMenuItem,
    key: &str,
) -> Option<&'a crate::native_menu::NativeMenuItem> {
    if item.key.as_deref() == Some(key) {
        return Some(item);
    }
    item.children
        .iter()
        .find_map(|child| find_item_by_key(child, key))
}

pub(crate) fn collect_set_binding_keys(
    item: &crate::native_menu::NativeMenuItem,
    target: &str,
    keys: &mut Vec<String>,
) {
    if let crate::native_menu::NativeMenuValue::Action(NativeMenuAction::SetParamBinding {
        target: action_target,
        binding,
    }) = &item.value
    {
        if action_target == target {
            keys.push(binding.key.clone());
        }
    }
    for child in &item.children {
        collect_set_binding_keys(child, target, keys);
    }
}

#[test]
pub(crate) fn generated_selector_trees_expose_representative_bindings() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.dance_mode = "xy".into();
    runner.menu.rebuild(runner.menu_config());

    for (target, expected_key) in [
        ("param:0:x:0", "sound.noteLengthMs"),
        ("aux:0:turn", "sound.noteLengthMs"),
        ("xy:x", "sound.noteLengthMs"),
    ] {
        let picker = find_item_by_key(&runner.menu.root, target).expect("picker target");
        let mut keys = Vec::new();
        collect_set_binding_keys(picker, target, &mut keys);
        assert!(
            keys.iter().any(|key| key == expected_key),
            "{target} should expose {expected_key}, got {keys:?}"
        );
    }

    let picker = find_item_by_key(&runner.menu.root, "param:0:x:0").expect("sense x picker");
    let group_labels = picker
        .children
        .iter()
        .filter(|child| matches!(child.value, crate::native_menu::NativeMenuValue::Group))
        .map(|child| child.label.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        group_labels,
        vec!["L1: Life", "L2: Sense", "L3: Voice", "L4: Dance", "System"]
    );
}

#[test]
pub(crate) fn behavior_target_picker_uses_per_part_behavior_rows_and_hides_none_parts() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.part_behavior_ids[1] = "life".into();
    runner.part_names[1] = "life".into();
    runner.part_behavior_ids[2] = "none".into();
    runner.part_names[2] = "none".into();
    runner.menu.rebuild(runner.menu_config());

    let picker = find_item_by_key(&runner.menu.root, "param:0:x:0").expect("sense x picker");
    let behavior_group = picker
        .children
        .iter()
        .find(|child| child.label == "L1: Life")
        .expect("L1 target group");

    assert!(behavior_group
        .children
        .iter()
        .any(|child| child.label == "P1: life"));
    assert!(!behavior_group
        .children
        .iter()
        .any(|child| child.label.contains("none")));
}

#[test]
pub(crate) fn behavior_change_remaps_behavior_param_mods_and_aux_bindings() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.param_mods[0].x[0] = Some(NativeParamBinding {
        key: "parts.0.l1.behaviorConfig.randomTickInterval".into(),
        label: Some("Spawn Interval".into()),
        kind: "number".into(),
        min: Some(1.0),
        max: Some(20.0),
        step: Some(1.0),
        options: vec![],
        invert: false,
    });
    runner.param_mods[0].y[0] = Some(NativeParamBinding {
        key: "parts.0.l1.behaviorConfig.randomCellsPerTick".into(),
        label: Some("Spawn Count".into()),
        kind: "number".into(),
        min: Some(0.0),
        max: Some(20.0),
        step: Some(1.0),
        options: vec![],
        invert: true,
    });
    runner.aux_bindings[0] = Some(NativeAuxBinding {
        turn_key: Some("behaviorConfig.life.randomTickInterval".into()),
        press_action: Some(NativeMenuAction::BehaviorAction("spawnRandom".into())),
    });

    runner.remap_bindings_for_behavior_change("life", "brain", 0);

    let x_binding = runner.param_mods[0].x[0].as_ref().unwrap();
    assert_eq!(x_binding.key, "parts.0.l1.behaviorConfig.seedInterval");
    assert_eq!(x_binding.label.as_deref(), Some("Seed Interval"));
    assert!(!x_binding.invert);

    let y_binding = runner.param_mods[0].y[0].as_ref().unwrap();
    assert_eq!(y_binding.key, "parts.0.l1.behaviorConfig.randomSeedCells");
    assert_eq!(y_binding.label.as_deref(), Some("Spawn Count"));
    assert!(y_binding.invert);

    let aux = runner.aux_bindings[0].as_ref().unwrap();
    assert_eq!(
        aux.turn_key.as_deref(),
        Some("behaviorConfig.brain.seedInterval")
    );
    assert!(
        matches!(aux.press_action.as_ref(), Some(NativeMenuAction::BehaviorAction(action)) if action == "seedRandom")
    );
}
