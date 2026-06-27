use super::*;

#[test]
fn sense_value_lanes_load_into_runner_and_menu_curve_edits_apply() {
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

    runner.menu.rebuild(runner.menu_config());
    runner.menu.turn_key("parts.0.l2.x.velocity.curve", -1);
    runner.apply_menu_state().unwrap();
    assert_eq!(runner.sense_parts[0].x_velocity.curve, "linear");
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["parts"][0]["l2"]["x"]["velocity"]["curve"],
        "linear"
    );
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
fn dance_xy_overlay_marks_physical_touch_with_inverted_modulation() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.active_dance_mode = "xy".into();
    runner.xy_invert_x = true;
    runner.xy_invert_y = true;
    runner.xy_release = "sample-hold".into();

    let pressed = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 1, "y": 6 }),
        })
        .unwrap();
    assert!((runner.xy_touch.x - 6.0 / 7.0).abs() < 0.0001);
    assert!((runner.xy_touch.y - 1.0 / 7.0).abs() < 0.0001);
    let snapshot = snapshot_from(&pressed);
    let cells = led_cells(&snapshot);
    assert_eq!(
        cells[display_index(1, 6)],
        json!({ "r": 255, "g": 255, "b": 255 })
    );

    let released = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_release", "x": 1, "y": 6 }),
        })
        .unwrap();
    assert!(!runner.xy_touch.active);
    let snapshot = snapshot_from(&released);
    let cells = led_cells(&snapshot);
    assert_eq!(
        cells[display_index(1, 6)],
        json!({ "r": 80, "g": 80, "b": 80 })
    );
}

#[test]
fn dance_xy_reset_center_overlay_returns_to_center() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.active_dance_mode = "xy".into();
    runner.xy_release = "reset-center".into();

    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 7, "y": 0 }),
        })
        .unwrap();
    let released = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_release", "x": 7, "y": 0 }),
        })
        .unwrap();
    let snapshot = snapshot_from(&released);
    let cells = led_cells(&snapshot);

    assert_eq!(runner.xy_touch.x, 0.5);
    assert_eq!(runner.xy_touch.y, 0.5);
    assert_eq!(
        cells[display_index(4, 4)],
        json!({ "r": 48, "g": 48, "b": 48 })
    );
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
fn voice_stealing_param_mod_emits_full_audio_config_command() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let _ = runner.messages_with_snapshot().unwrap();
    runner.param_mods[0].x[0] = Some(NativeParamBinding {
        key: "sound.voiceStealingMode".into(),
        label: Some("Steal".into()),
        kind: "enum".into(),
        min: None,
        max: None,
        step: None,
        options: vec!["auto-balanced".into(), "auto-hard".into()],
        invert: false,
    });
    let intents = vec![CellTriggerIntent {
        x: 7,
        y: 0,
        degree: 0,
        kind: platform_core::CellTriggerKind::Activate,
    }];

    runner.apply_runtime_modulation(&intents, 0);
    let messages = runner.messages_with_snapshot().unwrap();

    let config = messages
        .iter()
        .find_map(|message| match message {
            RunnerMessage::AudioCommands { commands } => commands.iter().find_map(|command| {
                if let RuntimeAudioCommand::SetAudioConfig { config, .. } = command {
                    Some(config)
                } else {
                    None
                }
            }),
            _ => None,
        })
        .expect("full audio config command");
    assert_eq!(config["voiceStealingMode"], "auto-hard");
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
    let cells = led_cells(&snapshot);

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
