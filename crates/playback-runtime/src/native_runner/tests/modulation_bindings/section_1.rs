use super::*;
use crate::native_menu::PlatformAction;

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
        user_min: None,
        user_max: None,
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
        runner.layers[0].param_mods.x[0].as_ref().unwrap().key,
        "instruments.0.mixer.volume"
    );
    assert_eq!(
        runner.xy.x_binding.as_ref().unwrap().key,
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
                PlatformAction::SampleAssign {
                    instrument_slot: 0,
                    sample_slot: 0,
                },
            ))),
        })
        .unwrap();
    assert!(matches!(
        runner.aux_bindings[0]
            .as_ref()
            .and_then(|binding| binding.press_action.as_ref()),
        Some(NativeMenuAction::PlatformEffect(
            PlatformAction::SampleAssign {
                instrument_slot: 0,
                sample_slot: 0
            }
        ))
    ));
}

#[test]
pub(crate) fn representative_selector_actions_update_link_aux_and_play_xy_bindings() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let binding = NativeParamBindingSpec {
        key: "mixer.buses.0.slot1.params.mixPct".into(),
        label: Some("Mix".into()),
        kind: "number".into(),
        min: Some(0),
        max: Some(100),
        step: Some(1),
        user_min: None,
        user_max: None,
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

    assert_eq!(
        runner.layers[0].param_mods.x[0].as_ref().unwrap().key,
        binding.key
    );
    assert_eq!(
        runner.aux_bindings[1]
            .as_ref()
            .and_then(|binding| binding.turn_key.as_deref()),
        Some(binding.key.as_str())
    );
    assert_eq!(runner.xy.y_binding.as_ref().unwrap().key, binding.key);
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
    runner.play_mode = PlayMode::Xy;
    runner.menu.rebuild(runner.menu_config());

    for (target, expected_key) in [
        ("param:0:x:0", "sound.noteLengthMs"),
        ("aux:0:turn", "sound.noteLengthMs"),
        ("xy:x", "sound.noteLengthMs"),
    ] {
        let picker = runner.menu.item_for_key(target).expect("picker target");
        let mut keys = Vec::new();
        collect_set_binding_keys(&picker, target, &mut keys);
        assert!(
            keys.iter().any(|key| key == expected_key),
            "{target} should expose {expected_key}, got {keys:?}"
        );
    }

    let picker = runner
        .menu
        .item_for_key("param:0:x:0")
        .expect("sense x picker");
    let group_labels = picker
        .children
        .iter()
        .filter(|child| matches!(child.value, crate::native_menu::NativeMenuValue::Group))
        .map(|child| child.label.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        group_labels,
        vec!["Build", "Link", "Shape", "Play", "System"]
    );
}

#[test]
pub(crate) fn behavior_target_picker_uses_per_layer_behavior_rows_and_hides_none_layers() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.layers[1].behavior_id = "life".into();
    runner.layers[1].name = "life".into();
    runner.layers[2].behavior_id = "none".into();
    runner.layers[2].name = "none".into();
    runner.menu.rebuild(runner.menu_config());

    let picker = runner
        .menu
        .item_for_key("param:0:x:0")
        .expect("sense x picker");
    let behavior_group = picker
        .children
        .iter()
        .find(|child| child.label == "Build")
        .expect("Build target group");

    assert!(behavior_group
        .children
        .iter()
        .any(|child| child.label == "L1: life"));
    assert!(!behavior_group
        .children
        .iter()
        .any(|child| child.label.contains("none")));
}

#[test]
pub(crate) fn behavior_change_remaps_behavior_param_mods_and_aux_bindings() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.layers[0].param_mods.x[0] = Some(NativeParamBinding {
        key: "layers.0.build.behaviorConfig.randomTickInterval".into(),
        label: Some("Spawn Interval".into()),
        kind: "number".into(),
        min: Some(1.0),
        max: Some(20.0),
        step: Some(1.0),
        user_min: None,
        user_max: None,
        options: vec![],
        invert: false,
    });
    runner.layers[0].param_mods.y[0] = Some(NativeParamBinding {
        key: "layers.0.build.behaviorConfig.randomCellsPerTick".into(),
        label: Some("Spawn Count".into()),
        kind: "number".into(),
        min: Some(0.0),
        max: Some(20.0),
        step: Some(1.0),
        user_min: None,
        user_max: None,
        options: vec![],
        invert: true,
    });
    runner.aux_bindings[0] = Some(NativeAuxBinding {
        turn_key: Some("behaviorConfig.life.randomTickInterval".into()),
        press_action: Some(NativeMenuAction::LayerBehaviorAction {
            layer_index: 0,
            action_type: "spawnRandom".into(),
        }),
    });

    runner.remap_bindings_for_behavior_change("brain", 0);

    let x_binding = runner.layers[0].param_mods.x[0].as_ref().unwrap();
    assert_eq!(x_binding.key, "layers.0.build.behaviorConfig.seedInterval");
    assert_eq!(x_binding.label.as_deref(), Some("Seed Interval"));
    assert!(!x_binding.invert);

    let y_binding = runner.layers[0].param_mods.y[0].as_ref().unwrap();
    assert_eq!(
        y_binding.key,
        "layers.0.build.behaviorConfig.randomSeedCells"
    );
    assert_eq!(y_binding.label.as_deref(), Some("Spawn Count"));
    assert!(y_binding.invert);

    let aux = runner.aux_bindings[0].as_ref().unwrap();
    assert_eq!(
        aux.turn_key.as_deref(),
        Some("behaviorConfig.brain.seedInterval")
    );
    assert!(
        matches!(aux.press_action.as_ref(), Some(NativeMenuAction::LayerBehaviorAction { action_type, .. }) if action_type == "spawnRandom"),
        "a bound click keeps its action; on brain it is not active"
    );
}
