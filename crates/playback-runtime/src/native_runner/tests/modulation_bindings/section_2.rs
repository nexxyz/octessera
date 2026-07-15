use super::*;

#[test]
pub(crate) fn sparks_xy_binding_updates_native_runtime_config() {
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
        user_min: None,
        user_max: None,
        options: vec![],
        invert: true,
    });

    runner.apply_runtime_modulation(&[], 0);

    assert_eq!(runner.global_sound.velocity_scale_pct, 200);
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["layers"][0]["xy"]["x"]["key"],
        "sound.velocityScalePct"
    );
}

#[test]
pub(crate) fn xy_mapping_execute_action_keeps_menu_on_xy_axis_picker() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.sparks_mode = "xy".into();
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
                user_min: None,
                user_max: None,
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
        "Menu > Play > XY > X Axis: Cutoff"
    );
    let snapshot = runner.snapshot().unwrap();
    assert_eq!(snapshot["display"]["title"], "/P/XY");
}

#[test]
pub(crate) fn xy_binding_can_drive_pulses_fx_bus_and_global_fx_params() {
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
        key: "layers.0.pulses.x.pitch.steps".into(),
        label: Some("Steps".into()),
        kind: "number".into(),
        min: Some(-16.0),
        max: Some(16.0),
        step: Some(1.0),
        user_min: None,
        user_max: None,
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
        user_min: None,
        user_max: None,
        options: vec![],
        invert: false,
    });

    runner.apply_runtime_modulation(&[], 0);

    assert_eq!(runner.pulses_layers[0].x_pitch_steps, 16);
    assert_eq!(runner.fx_buses[0].slot1_params["feedback"], json!(0.98));

    runner.xy_x_binding = Some(NativeParamBinding {
        key: "mixer.master.slots.0.params.cracklePct".into(),
        label: Some("Crackle".into()),
        kind: "number".into(),
        min: Some(0.0),
        max: Some(100.0),
        step: Some(1.0),
        user_min: None,
        user_max: None,
        options: vec![],
        invert: false,
    });
    runner.xy_y_binding = Some(NativeParamBinding {
        key: "sparks.fx.params.rateHz".into(),
        label: Some("Rate Hz".into()),
        kind: "number".into(),
        min: Some(1.0),
        max: Some(32.0),
        step: Some(1.0),
        user_min: None,
        user_max: None,
        options: vec![],
        invert: false,
    });
    runner.sparks_fx_selected = json!({ "fxType": "stutter", "targetKey": "master", "params": { "rateHz": 8, "depthPct": 100 } });

    runner.apply_runtime_modulation(&[], 0);

    assert_eq!(runner.global_fx_params[0]["cracklePct"], json!(100));
    assert_eq!(runner.sparks_fx_selected["params"]["rateHz"], json!(32.0));
}

#[test]
pub(crate) fn invalid_aux_and_xy_bindings_are_dropped_on_load() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["auxBindings"] = json!({ "aux1": { "turnKey": "../../bad", "pressAction": null }, "aux2": { "turnKey": "sound.noteLengthMs", "pressAction": null } });
    payload["runtimeConfig"]["layers"][0]["xy"]["x"] =
        json!({ "key": "unknown.path", "kind": "number" });
    payload["runtimeConfig"]["layers"][0]["xy"]["y"] = json!({ "key": "instruments.0.mixer.volume", "kind": "number", "min": 0, "max": 100, "step": 1 });

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
pub(crate) fn numeric_binding_user_range_maps_values_and_round_trips() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.xy_touch = NativeXyTouch {
        x: 0.5,
        y: 0.0,
        display_x: 0.5,
        display_y: 0.0,
        active: true,
    };
    runner.xy_x_binding = Some(NativeParamBinding {
        key: "instruments.0.mixer.volume".into(),
        label: Some("Volume".into()),
        kind: "number".into(),
        min: Some(0.0),
        max: Some(100.0),
        step: Some(1.0),
        user_min: Some(20.0),
        user_max: Some(80.0),
        options: vec![],
        invert: false,
    });

    runner.apply_runtime_modulation(&[], 0);

    assert_eq!(runner.instruments[0].volume, 50);
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["layers"][0]["xy"]["x"]["userMin"],
        20.0
    );
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["layers"][0]["xy"]["x"]["userMax"],
        80.0
    );
}

#[test]
pub(crate) fn custom_range_invert_equal_and_partial_ranges_are_sanitized() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.xy_invert_x = true;
    runner.xy_touch = NativeXyTouch {
        x: 0.25,
        y: 0.0,
        display_x: 0.25,
        display_y: 0.0,
        active: true,
    };
    runner
        .apply_config_payload(json!({
            "runtimeConfig": {
                "layers": [{
                    "xy": {
                        "x": {
                            "key": "instruments.0.mixer.volume",
                            "label": "Volume",
                            "kind": "number",
                            "min": 0,
                            "max": 100,
                            "step": 1,
                            "userMin": 90,
                            "userMax": 10,
                            "invert": true
                        }
                    }
                }]
            }
        }))
        .unwrap();

    let binding = runner.xy_x_binding.as_ref().unwrap();
    assert_eq!(binding.user_min, Some(10.0));
    assert_eq!(binding.user_max, Some(90.0));
    runner.apply_runtime_modulation(&[], 0);
    assert_eq!(runner.instruments[0].volume, 70);

    runner.xy_x_binding.as_mut().unwrap().user_min = Some(42.0);
    runner.xy_x_binding.as_mut().unwrap().user_max = Some(42.0);
    runner.apply_runtime_modulation(&[], 0);
    assert_eq!(runner.instruments[0].volume, 42);

    runner.xy_invert_x = false;
    runner.xy_x_binding.as_mut().unwrap().user_min = None;
    runner.xy_x_binding.as_mut().unwrap().user_max = Some(60.0);
    runner.apply_runtime_modulation(&[], 0);
    assert_eq!(runner.instruments[0].volume, 15);
}

#[test]
pub(crate) fn enum_and_bool_bindings_drop_user_ranges_on_load() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner
        .apply_config_payload(json!({
            "runtimeConfig": {
                "layers": [{
                    "xy": {
                        "x": {
                            "key": "sound.voiceStealingMode",
                            "label": "Steal",
                            "kind": "enum",
                            "options": ["auto-balanced", "auto-hard"],
                            "userMin": 10,
                            "userMax": 20
                        }
                    }
                }]
            }
        }))
        .unwrap();

    let binding = runner.xy_x_binding.as_ref().unwrap();
    assert_eq!(binding.kind, "enum");
    assert_eq!(binding.user_min, None);
    assert_eq!(binding.user_max, None);
}

#[test]
pub(crate) fn range_rows_edit_xy_and_param_mod_bindings_only() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.sparks_mode = "xy".into();
    runner.xy_x_binding = Some(NativeParamBinding {
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
    runner.param_mods[0].x[0] = Some(NativeParamBinding {
        key: "instruments.0.mixer.panPos".into(),
        label: Some("Pan Pos".into()),
        kind: "number".into(),
        min: Some(0.0),
        max: Some(32.0),
        step: Some(1.0),
        user_min: None,
        user_max: None,
        options: vec![],
        invert: false,
    });
    runner.menu.rebuild(runner.menu_config());

    assert!(runner.menu.focus_item_key("xy:x.rangeMin"));
    runner.menu.state.editing = true;
    runner.menu.turn(25);
    runner.apply_menu_state().unwrap();
    assert_eq!(runner.xy_x_binding.as_ref().unwrap().user_min, Some(25.0));
    assert_eq!(runner.param_mods[0].x[0].as_ref().unwrap().user_min, None);

    assert!(runner.menu.focus_item_key("param:0:x:0.rangeMax"));
    runner.menu.state.editing = true;
    runner.menu.turn(-8);
    runner.apply_menu_state().unwrap();
    assert_eq!(
        runner.param_mods[0].x[0].as_ref().unwrap().user_max,
        Some(24.0)
    );
    assert_eq!(runner.xy_x_binding.as_ref().unwrap().user_max, None);
}

#[test]
pub(crate) fn aux_numeric_binding_picker_hides_range_rows() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner
        .execute_menu_action(NativeMenuAction::SetParamBinding {
            target: "aux:0:turn".into(),
            binding: NativeParamBindingSpec {
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
            },
        })
        .unwrap();
    runner.menu.rebuild(runner.menu_config());

    assert!(!runner.menu.focus_item_key("aux:0:turn.rangeMin"));
    assert!(!runner.menu.focus_item_key("aux:0:turn.rangeMax"));
}

#[test]
pub(crate) fn instrument_filter_aux_and_xy_bindings_survive_config_load() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["auxBindings"] = json!({
        "aux1": { "turnKey": "instruments.1.sample.filter.cutoffHz", "pressAction": null }
    });
    payload["runtimeConfig"]["layers"][0]["xy"]["x"] = json!({
        "key": "instruments.0.synth.filter.cutoffHz",
        "label": "Cutoff",
        "kind": "number",
        "min": 0,
        "max": 255,
        "step": 1
    });
    payload["runtimeConfig"]["layers"][0]["xy"]["y"] = json!({
        "key": "instruments.0.synth.filter.resonance",
        "label": "Res",
        "kind": "number",
        "min": 0,
        "max": 255,
        "step": 1
    });

    runner.apply_config_payload(payload).unwrap();

    assert_eq!(
        runner.aux_bindings[0].as_ref().unwrap().turn_key.as_deref(),
        Some("instruments.1.sample.filter.cutoffHz")
    );
    assert_eq!(
        runner.xy_x_binding.as_ref().unwrap().key,
        "instruments.0.synth.filter.cutoffHz"
    );
    assert_eq!(
        runner.xy_y_binding.as_ref().unwrap().key,
        "instruments.0.synth.filter.resonance"
    );
}

#[test]
pub(crate) fn config_payload_includes_complete_sample_and_fx_param_shapes() {
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
