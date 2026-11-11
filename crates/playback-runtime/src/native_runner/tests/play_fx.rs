use super::*;

#[test]
pub(crate) fn play_fx_grid_press_and_release_emit_audio_commands() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.active_play_mode = ActivePlayMode::Page(PlayMode::Fx);
    runner.play_fx_assignments.push(NativePlayFxAssignment {
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
    let start_index = start
        .iter()
        .position(|message| {
            matches!(
                message,
                RunnerMessage::PlatformEffects { effects }
                    if effects.iter().any(|effect| matches!(
                        effect,
                        RuntimePlatformEffect::AudioCommand {
                            command: RuntimeAudioCommand::MomentaryFxStart { .. }
                        }
                    ))
            )
        })
        .unwrap();
    assert!(start
        .iter()
        .take(start_index)
        .any(|message| matches!(message, RunnerMessage::AudioCommands { .. })));
    assert!(start
        .iter()
        .skip(start_index + 1)
        .all(|message| !matches!(message, RunnerMessage::AudioCommands { .. })));

    let stop = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_release", "x": 2, "y": 3 }),
            request_snapshot: None,
        })
        .unwrap();
    assert!(stop.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects == &vec![RuntimePlatformEffect::AudioCommand { command: RuntimeAudioCommand::MomentaryFxStop { id: "momentary-fx:2:3".into(), epoch: 1 } }]
    )));
}

#[test]
pub(crate) fn play_fx_same_config_assignment_toggles_tile_clear() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let config = json!({ "fxType": "stutter", "targetKey": "master", "params": { "rateHz": 8 } });
    runner.grid_assign = Some(GridAssign::PlayFx(config.clone()));
    runner.handle_play_fx_assignment_grid_press(1, 2);
    assert_eq!(runner.play_fx_assignments.len(), 1);

    runner.grid_assign = Some(GridAssign::PlayFx(config));
    runner.handle_play_fx_assignment_grid_press(1, 2);
    assert!(runner.play_fx_assignments.is_empty());
}

#[test]
pub(crate) fn play_fx_press_blocks_same_type_and_limits_concurrency() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.active_play_mode = ActivePlayMode::Page(PlayMode::Fx);
    for (x, fx_type) in ["stutter", "freeze", "filter_sweep", "pitch_shift"]
        .iter()
        .enumerate()
    {
        runner.play_fx_assignments.push(NativePlayFxAssignment {
            x,
            y: 0,
            config: json!({ "fxType": fx_type, "targetKey": "master", "params": {} }),
        });
    }
    runner.play_fx_assignments.push(NativePlayFxAssignment {
        x: 4,
        y: 0,
        config: json!({ "fxType": "freeze", "targetKey": "master", "params": {} }),
    });
    runner.play_fx_assignments.push(NativePlayFxAssignment {
        x: 5,
        y: 0,
        config: json!({ "fxType": "glitch", "targetKey": "master", "params": {} }),
    });

    for x in 0..2 {
        assert!(!runner.play_fx_press_effects(x, 0).is_empty());
    }
    assert_eq!(runner.active_play_fx.len(), 2);

    let same_type = runner.play_fx_press_effects(4, 0);
    assert!(same_type.is_empty());
    assert_eq!(runner.active_play_fx.len(), 2);

    let limited = runner.play_fx_press_effects(5, 0);
    assert!(limited.is_empty());
    assert_eq!(runner.active_play_fx.len(), 2);
}

#[test]
pub(crate) fn play_fx_overlay_marks_active_and_limited_tiles() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.active_play_mode = ActivePlayMode::Page(PlayMode::Fx);
    runner.play_fx_assignments.push(NativePlayFxAssignment {
        x: 0,
        y: 0,
        config: json!({ "fxType": "stutter", "targetKey": "master", "params": {} }),
    });
    runner.play_fx_assignments.push(NativePlayFxAssignment {
        x: 1,
        y: 0,
        config: json!({ "fxType": "stutter", "targetKey": "master", "params": {} }),
    });
    runner.active_play_fx = vec![
        ("momentary-fx:0:0".into(), "stutter".into()),
        ("momentary-fx:2:0".into(), "freeze".into()),
    ];

    let snapshot = runner.snapshot().unwrap();
    let tiles = led_tiles(&snapshot);
    let active = &tiles[display_index(0, 0)];
    let limited = &tiles[display_index(1, 0)];

    let active_brightness = active["r"].as_i64().unwrap()
        + active["g"].as_i64().unwrap()
        + active["b"].as_i64().unwrap();
    let limited_brightness = limited["r"].as_i64().unwrap()
        + limited["g"].as_i64().unwrap()
        + limited["b"].as_i64().unwrap();
    assert!(active_brightness > limited_brightness);
}

#[test]
pub(crate) fn play_fx_overlay_uses_canonical_fx_palette() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.active_play_mode = ActivePlayMode::Page(PlayMode::Fx);
    for (x, fx_type) in ["stutter", "freeze", "filter_sweep", "pitch_shift"]
        .into_iter()
        .enumerate()
    {
        runner.play_fx_assignments.push(NativePlayFxAssignment {
            x,
            y: 0,
            config: json!({ "fxType": fx_type, "targetKey": "master", "params": {} }),
        });
    }

    let snapshot = runner.snapshot().unwrap();
    let tiles = led_tiles(&snapshot);

    assert_eq!(
        tiles[display_index(0, 0)],
        led_rgb(platform_core::palette::YELLOW)
    );
    assert_eq!(
        tiles[display_index(1, 0)],
        led_rgb(platform_core::palette::BLUE)
    );
    assert_eq!(
        tiles[display_index(2, 0)],
        led_rgb(platform_core::palette::GREEN)
    );
    assert_eq!(
        tiles[display_index(3, 0)],
        led_rgb(platform_core::palette::MAGENTA)
    );
}

#[test]
pub(crate) fn play_fx_map_to_grid_stores_config_and_payload_round_trips() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.play_fx_selected = json!({
        "fxType": "pitch_shift",
        "targetKey": "fx_bus_1",
        "params": { "semitones": 7, "cents": 12, "slideInMs": 120, "slideOutMs": 180, "mixPct": 65 }
    });
    let _ = runner
        .execute_menu_action(crate::native_menu::NativeMenuAction::PlatformEffect(
            "play.fx.map".into(),
        ))
        .unwrap();
    assert!(matches!(runner.grid_assign, Some(GridAssign::PlayFx(_))));
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 4, "y": 5 }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(runner.play_fx_assignments.len(), 1);
    assert_eq!(
        runner.play_fx_assignments[0].config["fxType"],
        "pitch_shift"
    );

    let payload = runner.config_payload();
    let mut loaded = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    loaded.apply_config_payload(payload).unwrap();
    assert_eq!(loaded.play_fx_assignments.len(), 1);
    assert_eq!(loaded.play_fx_assignments[0].x, 4);
    assert_eq!(
        loaded.play_fx_assignments[0].config["params"]["semitones"],
        7
    );
    assert_eq!(
        loaded.play_fx_assignments[0].config["params"]["slideInMs"],
        120
    );
    assert_eq!(
        loaded.play_fx_assignments[0].config["params"]["slideOutMs"],
        180
    );
}

#[test]
pub(crate) fn a_slow_clock_drift_retimes_a_held_dub_throw_once_it_adds_up() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.transport.sync_source = SyncSource::External;
    runner.active_play_mode = ActivePlayMode::Page(PlayMode::Fx);
    runner.play_fx_assignments.push(NativePlayFxAssignment {
        x: 0,
        y: 0,
        config: json!({ "fxType": "dub_throw", "targetKey": "master", "params": { "delayNote": 3 } }),
    });
    let mut at = std::time::Instant::now();
    runner.observe_external_clock_tempo_at(1, at);
    let mut bar = |runner: &mut NativeRunner, bpm: f64| {
        let pulse = std::time::Duration::from_secs_f64(60.0 / bpm / 24.0);
        for _ in 0..96 {
            at += pulse;
            runner.observe_external_clock_tempo_at(1, at);
        }
        runner
            .outbox
            .drain_audio_commands()
            .iter()
            .filter(|command| matches!(command, RuntimeAudioCommand::MomentaryFxUpdate { .. }))
            .count()
    };
    assert_eq!(bar(&mut runner, 120.0), 0);
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 0, "y": 0 }),
            request_snapshot: None,
        })
        .unwrap();

    assert_eq!(bar(&mut runner, 120.7), 0, "0.6 % is jitter");
    assert_eq!(bar(&mut runner, 121.4), 1, "1.2 % off the timed tempo");
    assert_eq!(bar(&mut runner, 122.1), 0, "0.6 % off the retimed tempo");
}

#[test]
pub(crate) fn play_fx_targets_on_fx_bus_3_and_4_round_trip() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["playFx"] = json!({
        "selected": { "fxType": "stutter", "targetKey": "fx_bus_4", "params": {} },
        "assignments": [
            { "x": 1, "y": 2, "config": { "fxType": "crush", "targetKey": "fx_bus_3", "params": {} } }
        ]
    });
    runner.apply_config_payload(payload).unwrap();

    let saved = runner.config_payload();
    let mut fresh = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    fresh.apply_config_payload(saved).unwrap();
    assert_eq!(fresh.play_fx_selected["targetKey"], "fx_bus_4");
    assert_eq!(fresh.play_fx_assignments[0].config["targetKey"], "fx_bus_3");
}
