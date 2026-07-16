use super::*;

#[test]
pub(crate) fn assignment_mode_wins_over_fn_layer_navigation_and_autosaves() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.auto_save_default = true;
    runner.instruments[0].kind = "sampler".into();
    runner.sample_assign = Some((0, 1));
    runner.ui.fn_held = true;

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 0, "y": 3 }),
            request_snapshot: None,
        })
        .unwrap();

    assert_eq!(runner.active_layer_index, 0);
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
pub(crate) fn sparks_mix_grid_edit_autosaves_persistent_volume_change() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.auto_save_default = true;
    runner.active_sparks_mode = "mix".into();

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 0, "y": 1 }),
            request_snapshot: None,
        })
        .unwrap();

    assert_eq!(runner.instruments[0].volume, 14);
    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::AudioCommands { commands }
            if commands.iter().any(|command| matches!(
                command,
                RuntimeAudioCommand::SetInstrumentMixer {
                    instrument_slot: 0,
                    volume_pct: Some(volume),
                    pan_pos: None,
                } if (*volume - 14.0).abs() < f32::EPSILON
            ))
    )));
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
pub(crate) fn sparks_xy_touch_persists_and_release_behavior_matches_config() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.active_sparks_mode = "xy".into();
    runner.xy_invert_x = true;
    runner.xy_release = "reset-center".into();

    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 0, "y": 7 }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(runner.xy_touch.x, 1.0);
    assert_eq!(runner.xy_touch.y, 1.0);
    assert!(runner.xy_touch.active);

    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_release", "x": 0, "y": 7 }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(runner.xy_touch.x, 0.5);
    assert_eq!(runner.xy_touch.y, 0.5);
    assert!(!runner.xy_touch.active);

    let payload = runner.config_payload();
    assert_eq!(payload["runtimeConfig"]["xyRelease"], "reset-center");
    assert_eq!(payload["runtimeConfig"]["layers"][0]["xy"]["xInvert"], true);

    let mut restored = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    restored.apply_config_payload(payload).unwrap();
    assert_eq!(restored.xy_release, "reset-center");
    assert!(restored.xy_invert_x);
}

#[test]
pub(crate) fn sparks_xy_overlay_marks_physical_touch_with_inverted_modulation() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.active_sparks_mode = "xy".into();
    runner.xy_invert_x = true;
    runner.xy_invert_y = true;
    runner.xy_release = "sample-hold".into();

    let pressed = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 1, "y": 6 }),
            request_snapshot: None,
        })
        .unwrap();
    assert!((runner.xy_touch.x - 6.0 / 7.0).abs() < 0.0001);
    assert!((runner.xy_touch.y - 1.0 / 7.0).abs() < 0.0001);
    let snapshot = snapshot_from(&pressed);
    let cells = led_cells(&snapshot);
    assert_eq!(
        cells[display_index(1, 6)],
        led_rgb(platform_core::palette::WHITE)
    );

    let released = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_release", "x": 1, "y": 6 }),
            request_snapshot: None,
        })
        .unwrap();
    assert!(!runner.xy_touch.active);
    let snapshot = snapshot_from(&released);
    let cells = led_cells(&snapshot);
    assert_eq!(
        cells[display_index(1, 6)],
        led_rgb(dim_rgb(platform_core::palette::GRAY, 2))
    );
}

#[test]
pub(crate) fn sparks_xy_reset_center_overlay_returns_to_center() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.active_sparks_mode = "xy".into();
    runner.xy_release = "reset-center".into();

    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 7, "y": 0 }),
            request_snapshot: None,
        })
        .unwrap();
    let released = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_release", "x": 7, "y": 0 }),
            request_snapshot: None,
        })
        .unwrap();
    let snapshot = snapshot_from(&released);
    let cells = led_cells(&snapshot);

    assert_eq!(runner.xy_touch.x, 0.5);
    assert_eq!(runner.xy_touch.y, 0.5);
    assert_eq!(
        cells[display_index(4, 4)],
        led_rgb(dim_rgb(platform_core::palette::GRAY, 4))
    );
}

#[test]
pub(crate) fn param_mod_binding_updates_native_runtime_config() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.param_mods[0].x[0] = Some(NativeParamBinding {
        key: "instruments.0.mixer.volume".into(),
        label: Some("Volume".into()),
        kind: "number".into(),
        min: Some(0.0),
        max: Some(100.0),
        step: Some(1.0),
        user_min: None,
        user_max: None,
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
        runner.config_payload()["runtimeConfig"]["layers"][0]["paramMods"]["x"][0]["key"],
        "instruments.0.mixer.volume"
    );
}
