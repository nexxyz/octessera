use super::*;

#[test]
fn menu_binding_actions_update_param_xy_and_aux_targets() {
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
fn representative_selector_actions_update_sense_aux_and_dance_xy_bindings() {
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

fn find_item_by_key<'a>(
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

fn collect_set_binding_keys(
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
fn generated_selector_trees_expose_representative_bindings() {
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
fn behavior_target_picker_uses_per_part_behavior_rows_and_hides_none_parts() {
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
fn behavior_change_remaps_behavior_param_mods_and_aux_bindings() {
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

#[test]
fn dance_xy_binding_updates_native_runtime_config() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.xy_touch = NativeXyTouch {
        x: 1.0,
        y: 0.5,
        display_x: 1.0,
        display_y: 0.5,
        active: true,
    };
    runner.xy_x_binding = Some(NativeParamBinding {
        key: "sound.velocityScalePct".into(),
        label: Some("Velocity Scale".into()),
        kind: "number".into(),
        min: Some(0.0),
        max: Some(200.0),
        step: Some(1.0),
        options: vec![],
        invert: true,
    });

    runner.apply_runtime_modulation(&[], 0);

    assert_eq!(runner.global_sound.velocity_scale_pct, 200);
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["parts"][0]["xy"]["x"]["key"],
        "sound.velocityScalePct"
    );
}

#[test]
fn xy_mapping_execute_action_keeps_menu_on_xy_axis_picker() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.dance_mode = "xy".into();
    runner.menu.rebuild(runner.menu_config());
    assert!(runner.menu.focus_item_key("xy:x"));

    runner
        .execute_menu_action(NativeMenuAction::SetParamBinding {
            target: "xy:x".into(),
            binding: NativeParamBindingSpec {
                key: "instruments.0.synth.filter.cutoffHz".into(),
                label: Some("Cutoff".into()),
                kind: "number".into(),
                min: Some(0),
                max: Some(255),
                step: Some(1),
                options: vec![],
                invert: false,
            },
        })
        .unwrap();

    assert_eq!(
        runner.xy_x_binding.as_ref().unwrap().key,
        "instruments.0.synth.filter.cutoffHz"
    );
    assert_eq!(
        runner.menu.current_focus_path(),
        "Menu > L4: Dance > X Axis: Cutoff"
    );
    let snapshot = runner.snapshot().unwrap();
    assert_eq!(snapshot["display"]["title"], "L4: Dance");
}

#[test]
fn xy_binding_can_drive_sense_fx_bus_and_global_fx_params() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.xy_touch = NativeXyTouch {
        x: 1.0,
        y: 1.0,
        display_x: 1.0,
        display_y: 1.0,
        active: true,
    };
    runner.fx_buses[0].slot1_type = "delay".into();
    runner.fx_buses[0].slot1_params = json!({ "feedback": 0.35, "timeMs": 250, "mixPct": 35 });
    runner.global_fx_slots[0] = "vinyl".into();
    runner.global_fx_params[0] =
        json!({ "cracklePct": 8, "saturationPct": 15, "warpDepthPct": 5, "mixPct": 100 });
    runner.xy_x_binding = Some(NativeParamBinding {
        key: "parts.0.l2.x.pitch.steps".into(),
        label: Some("Steps".into()),
        kind: "number".into(),
        min: Some(-16.0),
        max: Some(16.0),
        step: Some(1.0),
        options: vec![],
        invert: false,
    });
    runner.xy_y_binding = Some(NativeParamBinding {
        key: "mixer.buses.0.slot1.params.feedback".into(),
        label: Some("Feedback".into()),
        kind: "number".into(),
        min: Some(0.0),
        max: Some(98.0),
        step: Some(1.0),
        options: vec![],
        invert: false,
    });

    runner.apply_runtime_modulation(&[], 0);

    assert_eq!(runner.sense_parts[0].x_pitch_steps, 16);
    assert_eq!(runner.fx_buses[0].slot1_params["feedback"], json!(0.98));

    runner.xy_x_binding = Some(NativeParamBinding {
        key: "mixer.master.slots.0.params.cracklePct".into(),
        label: Some("Crackle".into()),
        kind: "number".into(),
        min: Some(0.0),
        max: Some(100.0),
        step: Some(1.0),
        options: vec![],
        invert: false,
    });
    runner.xy_y_binding = Some(NativeParamBinding {
        key: "dance.fx.params.rateHz".into(),
        label: Some("Rate Hz".into()),
        kind: "number".into(),
        min: Some(1.0),
        max: Some(32.0),
        step: Some(1.0),
        options: vec![],
        invert: false,
    });
    runner.dance_fx_selected = json!({ "fxType": "stutter", "targetKey": "master", "params": { "rateHz": 8, "depthPct": 100 } });

    runner.apply_runtime_modulation(&[], 0);

    assert_eq!(runner.global_fx_params[0]["cracklePct"], json!(100));
    assert_eq!(runner.dance_fx_selected["params"]["rateHz"], json!(32.0));
}

#[test]
fn invalid_aux_and_xy_bindings_are_dropped_on_load() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["auxBindings"] = json!({ "aux1": { "turnKey": "../../bad", "pressAction": null }, "aux2": { "turnKey": "sound.noteLengthMs", "pressAction": null } });
    payload["runtimeConfig"]["parts"][0]["xy"]["x"] =
        json!({ "key": "unknown.path", "kind": "number" });
    payload["runtimeConfig"]["parts"][0]["xy"]["y"] = json!({ "key": "instruments.0.mixer.volume", "kind": "number", "min": 0, "max": 100, "step": 1 });

    runner.apply_config_payload(payload).unwrap();

    assert!(runner.aux_bindings[0].is_none());
    assert_eq!(
        runner.aux_bindings[1].as_ref().unwrap().turn_key.as_deref(),
        Some("sound.noteLengthMs")
    );
    assert!(runner.xy_x_binding.is_none());
    assert_eq!(
        runner.xy_y_binding.as_ref().unwrap().key,
        "instruments.0.mixer.volume"
    );
}

#[test]
fn config_payload_includes_complete_sample_and_fx_param_shapes() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut payload = runner.config_payload();

    assert_eq!(
        payload["runtimeConfig"]["instruments"][0]["sample"]["baseVelocity"],
        100
    );
    assert_eq!(
        payload["runtimeConfig"]["instruments"][0]["midiEngine"]["velocity"],
        100
    );
    assert_eq!(
        payload["runtimeConfig"]["instruments"][0]["midiEngine"]["channel"],
        1
    );
    assert!(payload["runtimeConfig"]["instruments"][0]["sample"]["ampEnv"].is_object());
    assert!(payload["runtimeConfig"]["instruments"][0]["sample"]["filter"].is_object());
    assert!(payload["runtimeConfig"]["instruments"][0]["sample"]["filterEnv"].is_object());
    assert!(payload["runtimeConfig"]["mixer"]["buses"][0]["slot1"]["params"].is_object());
    assert!(payload["runtimeConfig"]["mixer"]["master"]["slots"][0]["params"].is_object());

    payload["runtimeConfig"]["instruments"][0]["sample"]["baseVelocity"] = json!(72);
    payload["runtimeConfig"]["instruments"][0]["sample"]["ampEnv"] = json!({ "attackMs": 11 });
    payload["runtimeConfig"]["instruments"][0]["sample"]["filter"] =
        json!({ "type": "highpass", "cutoffHz": 1200 });
    payload["runtimeConfig"]["instruments"][0]["sample"]["filterEnv"] = json!({ "releaseMs": 222 });
    payload["runtimeConfig"]["instruments"][0]["midiEngine"] =
        json!({ "channel": 7, "velocity": 66, "durationMs": 444 });
    payload["runtimeConfig"]["mixer"]["buses"][0]["slot1"] =
        json!({ "type": "delay", "params": { "timeMs": 333, "feedback": 0.42, "mixPct": 44 } });
    payload["runtimeConfig"]["mixer"]["master"]["slots"][0] =
        json!({ "type": "distortion", "params": { "drive": 3.5, "clip": 0.75, "mixPct": 88 } });
    runner.apply_config_payload(payload).unwrap();
    assert_eq!(runner.instruments[0].sample_base_velocity, 72);
    assert_eq!(runner.instruments[0].sample_amp_env["attackMs"], 11);
    assert_eq!(runner.instruments[0].sample_filter["type"], "highpass");
    assert_eq!(runner.instruments[0].sample_filter_env["releaseMs"], 222);
    assert_eq!(runner.instruments[0].midi_velocity, 66);
    assert_eq!(runner.instruments[0].midi_channel, 7);
    assert_eq!(runner.instruments[0].midi_duration_ms, 444);
    assert_eq!(runner.fx_buses[0].slot1_params["feedback"], 0.42);
    assert_eq!(runner.global_fx_params[0]["drive"], 3.5);
    let round_trip = runner.config_payload();
    assert_eq!(
        round_trip["runtimeConfig"]["mixer"]["buses"][0]["slot1"]["params"]["timeMs"],
        333
    );
    assert_eq!(
        round_trip["runtimeConfig"]["mixer"]["master"]["slots"][0]["params"]["clip"],
        0.75
    );
}
