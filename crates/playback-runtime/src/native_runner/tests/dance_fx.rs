use super::*;

#[test]
pub(crate) fn dance_fx_grid_press_and_release_emit_audio_commands() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.active_dance_mode = "fx".into();
    runner.dance_fx_assignments.push(NativeDanceFxAssignment {
        x: 2,
        y: 3,
        config: json!({
            "fxType": "stutter",
            "targetKey": "master",
            "params": { "rateHz": 12, "depthPct": 70 }
        }),
    });

    let start = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 2, "y": 3 }),
            request_snapshot: None,
        })
        .unwrap();
    assert!(start.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if matches!(effects.as_slice(), [RuntimePlatformEffect::AudioCommand { command: RuntimeAudioCommand::MomentaryFxStart { id, fx_type, params, .. } }] if id == "momentary-fx:2:3" && fx_type == "stutter" && params.get("rateHz") == Some(&json!(12)))
    )));

    let stop = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_release", "x": 2, "y": 3 }),
            request_snapshot: None,
        })
        .unwrap();
    assert!(stop.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects == &vec![RuntimePlatformEffect::AudioCommand { command: RuntimeAudioCommand::MomentaryFxStop { id: "momentary-fx:2:3".into() } }]
    )));
}

#[test]
pub(crate) fn dance_fx_same_config_assignment_toggles_cell_clear() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let config = json!({ "fxType": "stutter", "targetKey": "master", "params": { "rateHz": 8 } });
    runner.dance_fx_assign = Some(config.clone());
    runner.handle_dance_fx_assignment_grid_press(1, 2);
    assert_eq!(runner.dance_fx_assignments.len(), 1);

    runner.dance_fx_assign = Some(config);
    runner.handle_dance_fx_assignment_grid_press(1, 2);
    assert!(runner.dance_fx_assignments.is_empty());
}

#[test]
pub(crate) fn dance_fx_press_blocks_same_type_and_limits_concurrency() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.active_dance_mode = "fx".into();
    for (x, fx_type) in ["stutter", "freeze", "filter_sweep", "pitch_shift"]
        .iter()
        .enumerate()
    {
        runner.dance_fx_assignments.push(NativeDanceFxAssignment {
            x,
            y: 0,
            config: json!({ "fxType": fx_type, "targetKey": "master", "params": {} }),
        });
    }
    runner.dance_fx_assignments.push(NativeDanceFxAssignment {
        x: 4,
        y: 0,
        config: json!({ "fxType": "freeze", "targetKey": "master", "params": {} }),
    });
    runner.dance_fx_assignments.push(NativeDanceFxAssignment {
        x: 5,
        y: 0,
        config: json!({ "fxType": "glitch", "targetKey": "master", "params": {} }),
    });

    for x in 0..2 {
        assert!(!runner.dance_fx_press_effects(x, 0).is_empty());
    }
    assert_eq!(runner.active_dance_fx.len(), 2);

    let same_type = runner.dance_fx_press_effects(4, 0);
    assert!(same_type.is_empty());
    assert_eq!(runner.active_dance_fx.len(), 2);

    let limited = runner.dance_fx_press_effects(5, 0);
    assert!(limited.is_empty());
    assert_eq!(runner.active_dance_fx.len(), 2);
}

#[test]
pub(crate) fn dance_fx_overlay_marks_active_and_limited_cells() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.active_dance_mode = "fx".into();
    runner.dance_fx_assignments.push(NativeDanceFxAssignment {
        x: 0,
        y: 0,
        config: json!({ "fxType": "stutter", "targetKey": "master", "params": {} }),
    });
    runner.dance_fx_assignments.push(NativeDanceFxAssignment {
        x: 1,
        y: 0,
        config: json!({ "fxType": "stutter", "targetKey": "master", "params": {} }),
    });
    runner.active_dance_fx = vec![
        ("momentary-fx:0:0".into(), "stutter".into()),
        ("momentary-fx:2:0".into(), "freeze".into()),
    ];

    let snapshot = runner.snapshot().unwrap();
    let cells = led_cells(&snapshot);
    let active = &cells[display_index(0, 0)];
    let limited = &cells[display_index(1, 0)];

    let active_brightness = active["r"].as_i64().unwrap()
        + active["g"].as_i64().unwrap()
        + active["b"].as_i64().unwrap();
    let limited_brightness = limited["r"].as_i64().unwrap()
        + limited["g"].as_i64().unwrap()
        + limited["b"].as_i64().unwrap();
    assert!(active_brightness > limited_brightness);
}

#[test]
pub(crate) fn dance_fx_map_to_grid_stores_config_and_payload_round_trips() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.dance_fx_selected = json!({
        "fxType": "pitch_shift",
        "targetKey": "fx_bus_1",
        "params": { "semitones": 7, "cents": 12, "mixPct": 65 }
    });
    let _ = runner
        .execute_menu_action(crate::native_menu::NativeMenuAction::PlatformEffect(
            "dance.fx.map".into(),
        ))
        .unwrap();
    assert!(runner.dance_fx_assign.is_some());
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 4, "y": 5 }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(runner.dance_fx_assignments.len(), 1);
    assert_eq!(
        runner.dance_fx_assignments[0].config["fxType"],
        "pitch_shift"
    );

    let payload = runner.config_payload();
    let mut loaded = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    loaded.apply_config_payload(payload).unwrap();
    assert_eq!(loaded.dance_fx_assignments.len(), 1);
    assert_eq!(loaded.dance_fx_assignments[0].x, 4);
    assert_eq!(
        loaded.dance_fx_assignments[0].config["params"]["semitones"],
        7
    );
}
