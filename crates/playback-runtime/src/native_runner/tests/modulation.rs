use super::*;

#[test]
fn sense_value_lanes_round_trip_in_payload() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["parts"][0]["l2"]["x"]["velocity"] = json!({
        "enabled": true,
        "from": 12,
        "to": 99,
        "gridOffset": 2,
        "curve": "curve"
    });
    payload["runtimeConfig"]["parts"][0]["l2"]["y"]["filterResonance"] = json!({
        "enabled": true,
        "from": 3,
        "to": 77,
        "gridOffset": -1,
        "curve": "linear"
    });

    runner.apply_config_payload(payload).unwrap();

    assert!(runner.sense_parts[0].x_velocity.enabled);
    assert_eq!(runner.sense_parts[0].x_velocity.from, 12);
    assert_eq!(runner.sense_parts[0].x_velocity.to, 99);
    assert_eq!(runner.sense_parts[0].x_velocity.grid_offset, 2);
    assert_eq!(runner.sense_parts[0].x_velocity.curve, "curve");
    assert!(runner.sense_parts[0].y_filter_resonance.enabled);
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["parts"][0]["l2"]["x"]["velocity"]["to"],
        99
    );
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["parts"][0]["l2"]["x"]["velocity"]["curve"],
        "curve"
    );

    runner.menu.rebuild(runner.menu_config());
    runner.menu.turn_key("parts.0.l2.x.velocity.curve", -1);
    runner.apply_menu_state().unwrap();
    assert_eq!(runner.sense_parts[0].x_velocity.curve, "linear");
}

#[test]
fn assignment_mode_wins_over_fn_part_navigation_and_autosaves() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.auto_save_default = true;
    runner.instruments[0].kind = "sampler".into();
    runner.sample_assign = Some((0, 1));
    runner.ui.fn_held = true;

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 0, "y": 3 }),
        })
        .unwrap();

    assert_eq!(runner.active_part_index, 0);
    assert!(runner.instruments[0]
        .sample_assignments
        .iter()
        .any(|assignment| assignment.x == 0 && assignment.y == 3 && assignment.sample_slot == 1));
    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if matches!(
                effects.as_slice(),
                [RuntimePlatformEffect::StoreSaveDefault { mode, .. }]
                    if mode.as_deref() == Some("deferred")
            )
    )));
}

#[test]
fn dance_mix_grid_edit_autosaves_persistent_volume_change() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.auto_save_default = true;
    runner.active_dance_mode = "mix".into();

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 0, "y": 1 }),
        })
        .unwrap();

    assert_eq!(runner.instruments[0].volume, 14);
    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if matches!(
                effects.as_slice(),
                [RuntimePlatformEffect::StoreSaveDefault { mode, .. }]
                    if mode.as_deref() == Some("deferred")
            )
    )));
}

#[test]
fn dance_xy_touch_persists_and_release_behavior_matches_config() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.active_dance_mode = "xy".into();
    runner.xy_invert_x = true;
    runner.xy_release = "reset-center".into();

    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 0, "y": 7 }),
        })
        .unwrap();
    assert_eq!(runner.xy_touch.x, 1.0);
    assert_eq!(runner.xy_touch.y, 1.0);
    assert!(runner.xy_touch.active);

    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_release", "x": 0, "y": 7 }),
        })
        .unwrap();
    assert_eq!(runner.xy_touch.x, 0.5);
    assert_eq!(runner.xy_touch.y, 0.5);
    assert!(!runner.xy_touch.active);

    let payload = runner.config_payload();
    assert_eq!(payload["runtimeConfig"]["xyRelease"], "reset-center");
    assert_eq!(payload["runtimeConfig"]["parts"][0]["xy"]["xInvert"], true);

    let mut restored = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    restored.apply_config_payload(payload).unwrap();
    assert_eq!(restored.xy_release, "reset-center");
    assert!(restored.xy_invert_x);
}

#[test]
fn param_mod_binding_updates_native_runtime_config() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.param_mods[0].x[0] = Some(NativeParamBinding {
        key: "instruments.0.mixer.volume".into(),
        label: Some("Volume".into()),
        kind: "number".into(),
        min: Some(0.0),
        max: Some(100.0),
        step: Some(1.0),
        options: vec![],
        invert: false,
    });
    let intents = vec![CellTriggerIntent {
        x: 7,
        y: 0,
        degree: 0,
        kind: platform_core::CellTriggerKind::Activate,
    }];

    runner.apply_runtime_modulation(&intents, 0);

    assert_eq!(runner.instruments[0].volume, 100);
    assert!(runner.config_dirty);
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["parts"][0]["paramMods"]["x"][0]["key"],
        "instruments.0.mixer.volume"
    );
}

#[test]
fn shift_grid_param_mod_mapping_cycles_slots() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let binding = NativeParamBinding {
        key: "instruments.0.mixer.volume".into(),
        label: Some("Volume".into()),
        kind: "number".into(),
        min: Some(0.0),
        max: Some(100.0),
        step: Some(1.0),
        options: vec![],
        invert: false,
    };

    assert!(runner.apply_param_mod_mapping(0, 0, binding.clone()));
    assert_eq!(runner.param_mods[0].x[0].as_ref().unwrap().key, binding.key);
    assert_eq!(runner.param_mods[0].y[0].as_ref().unwrap().key, binding.key);
    assert!(!runner.param_mods[0].x[0].as_ref().unwrap().invert);
    assert!(runner.config_dirty);

    assert!(runner.apply_param_mod_mapping(0, 0, binding.clone()));
    assert!(runner.param_mods[0].x[0].as_ref().unwrap().invert);
    assert!(runner.param_mods[0].y[0].as_ref().unwrap().invert);

    assert!(runner.apply_param_mod_mapping(0, 0, binding.clone()));
    assert!(runner.param_mods[0].x[0].is_none());
    assert!(runner.param_mods[0].y[0].is_none());

    assert!(runner.apply_param_mod_mapping(2, 1, binding.clone()));
    assert_eq!(runner.param_mods[0].x[1].as_ref().unwrap().key, binding.key);
    assert!(runner.param_mods[0].y[1].is_none());

    assert!(runner.apply_param_mod_mapping(1, 4, binding.clone()));
    assert_eq!(runner.param_mods[0].y[1].as_ref().unwrap().key, binding.key);

    assert!(runner.apply_param_mod_mapping(1, 1, binding));
    assert!(runner.param_mods[0].x[1].as_ref().unwrap().invert);
    assert!(runner.param_mods[0].y[1].as_ref().unwrap().invert);
}

#[test]
fn shift_grid_param_mod_overlay_marks_lanes_and_combined_cells() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let binding = NativeParamBinding {
        key: "instruments.0.mixer.volume".into(),
        label: Some("Volume".into()),
        kind: "number".into(),
        min: Some(0.0),
        max: Some(100.0),
        step: Some(1.0),
        options: vec![],
        invert: false,
    };
    let mut inverted = binding.clone();
    inverted.invert = true;
    runner.param_mods[0].x[0] = Some(binding);
    runner.param_mods[0].y[1] = Some(inverted);

    runner.menu.turn(2);
    runner.menu.press();
    runner.menu.press();
    runner.menu.press();
    runner.menu.turn(3);
    runner.menu.press();
    runner.menu.turn(1);
    runner.ui.shift_held = true;

    let snapshot = runner.snapshot().unwrap();
    let cells = snapshot["leds"]["cells"].as_array().unwrap();

    assert_eq!(
        cells[display_index(3, 0)],
        json!({ "r": 0, "g": 255, "b": 120 })
    );
    assert_eq!(
        cells[display_index(1, 3)],
        json!({ "r": 255, "g": 0, "b": 90 })
    );
    assert_eq!(
        cells[display_index(3, 1)],
        json!({ "r": 18, "g": 18, "b": 24 })
    );
    assert_eq!(
        cells[display_index(0, 0)],
        json!({ "r": 255, "g": 255, "b": 255 })
    );
    assert_eq!(
        cells[display_index(1, 1)],
        json!({ "r": 255, "g": 255, "b": 255 })
    );
}

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
        runner.aux_bindings[0]
            .as_ref()
            .and_then(|binding| binding.press_action.as_ref()),
        Some(NativeMenuAction::PlatformEffect(action)) if action == "sample.assign:0:0"
    ));
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
    assert!(matches!(
        aux.press_action.as_ref(),
        Some(NativeMenuAction::BehaviorAction(action)) if action == "seedRandom"
    ));
}

#[test]
fn dance_xy_binding_updates_native_runtime_config() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.xy_touch = NativeXyTouch {
        x: 1.0,
        y: 0.5,
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
fn invalid_aux_and_xy_bindings_are_dropped_on_load() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["auxBindings"] = json!({
        "aux1": { "turnKey": "../../bad", "pressAction": null },
        "aux2": { "turnKey": "sound.noteLengthMs", "pressAction": null }
    });
    payload["runtimeConfig"]["parts"][0]["xy"]["x"] = json!({
        "key": "unknown.path",
        "kind": "number"
    });
    payload["runtimeConfig"]["parts"][0]["xy"]["y"] = json!({
        "key": "instruments.0.mixer.volume",
        "kind": "number",
        "min": 0,
        "max": 100,
        "step": 1
    });

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
    payload["runtimeConfig"]["instruments"][0]["midiEngine"] = json!({
        "channel": 7,
        "velocity": 66,
        "durationMs": 444
    });
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
