use super::*;

#[test]
pub(crate) fn play_page_menu_edits_selected_and_active_mode() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();

    for (mode, key) in [
        ("mix", "play.page.mix"),
        ("pan", "play.page.pan"),
        ("fx", "play.page.fx"),
        ("trigger-gate", "play.page.trigger-gate"),
        ("transpose", "play.page.transpose"),
        ("xy", "play.page.xy"),
    ] {
        assert!(runner.menu.focus_item_key(key));
        runner.apply_or_schedule_menu_key(key).unwrap();
        let snapshot = runner.snapshot().unwrap();
        assert_eq!(snapshot["display"]["title"], "/Play");
        assert_eq!(snapshot["playMode"], mode);
        assert_eq!(snapshot["activePlayMode"], mode);
    }
}

#[test]
pub(crate) fn play_transpose_page_round_trips_through_runtime_config() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    assert!(runner.menu.focus_item_key("play.page.transpose"));
    runner
        .apply_or_schedule_menu_key("play.page.transpose")
        .unwrap();
    let payload = runner.config_payload();

    assert_eq!(runner.play_mode, "transpose");
    assert_eq!(payload["runtimeConfig"]["playMode"], "transpose");
    assert!(payload["runtimeConfig"].get("playTranspose").is_none());

    let mut restored = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    restored.apply_config_payload(payload).unwrap();
    assert_eq!(restored.play_mode, "transpose");
    assert_eq!(restored.active_play_mode, "none");
}

#[test]
pub(crate) fn play_page_fast_path_applies_immediately_without_deferred_flush() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    assert!(runner.menu.focus_item_key("play.page.pan"));
    let changed = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(snapshot_from(&changed)["playMode"], "pan");
    assert_eq!(snapshot_from(&changed)["activePlayMode"], "pan");
    assert_eq!(runner.play_mode, "pan");
    assert_eq!(runner.active_play_mode, "pan");
    assert!(runner.flush_deferred_menu_apply().unwrap().is_empty());
}

#[test]
pub(crate) fn parameterless_play_pages_select_without_entering_empty_page() {
    for (key, mode) in [
        ("play.page.mix", "mix"),
        ("play.page.pan", "pan"),
        ("play.page.trigger-gate", "trigger-gate"),
        ("play.page.transpose", "transpose"),
    ] {
        let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
        assert!(runner.menu.focus_item_key(key));

        let messages = runner
            .send(HostMessage::DeviceInput {
                input: json!({ "type": "encoder_press", "id": "main" }),
                request_snapshot: None,
            })
            .unwrap();

        assert_eq!(runner.play_mode, mode);
        assert_eq!(runner.active_play_mode, mode);
        assert_eq!(runner.menu.state.stack, vec![3]);
        assert_eq!(snapshot_from(&messages)["display"]["title"], "/Play");
    }
}

#[test]
pub(crate) fn parameterized_play_pages_remain_enterable() {
    for (key, expected_label) in [
        ("play.page.fx", "FX Type"),
        ("play.page.xy", "X Axis: (none)"),
    ] {
        let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
        assert!(runner.menu.focus_item_key(key));

        runner
            .send(HostMessage::DeviceInput {
                input: json!({ "type": "encoder_press", "id": "main" }),
                request_snapshot: None,
            })
            .unwrap();

        assert_eq!(runner.menu.current_label(), Some(expected_label));
        assert_eq!(runner.menu.state.stack.len(), 2);
    }
}

#[test]
pub(crate) fn play_mode_edits_outside_play_page_do_not_activate_overlay() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.play_mode = "mix".into();
    runner.active_play_mode = "none".into();
    runner.menu.rebuild(runner.menu_config());

    assert!(runner.menu.focus_item_key("play.page.pan"));
    runner.menu.state.stack = vec![0];
    runner.apply_or_schedule_menu_key("play.page.pan").unwrap();

    assert_eq!(runner.play_mode, "pan");
    assert_eq!(runner.active_play_mode, "none");
}

#[test]
pub(crate) fn fn_grid_context_changes_show_oled_toasts() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.layer_names[2] = "rain".into();

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_fn", "pressed": true }),
            request_snapshot: None,
        })
        .unwrap();
    let layer = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 0, "y": 2 }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(snapshot_from(&layer)["display"]["toast"], "Layer: L3 rain");

    let play = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 7, "y": 2 }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(snapshot_from(&play)["display"]["toast"], "Play: fx");
}

#[test]
pub(crate) fn play_fx_type_turn_updates_params_immediately() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.play_mode = "fx".into();
    runner.active_play_mode = "fx".into();
    runner.menu.rebuild(runner.menu_config());
    assert!(runner.menu.focus_item_key("play.fx.type"));
    runner.menu.state.editing = true;

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();

    assert_eq!(
        runner.menu.value_for_key("play.fx.type").as_deref(),
        Some("stutter")
    );
    assert_eq!(runner.play_fx_selected["fxType"], "stutter");
    assert!(runner
        .menu
        .number_for_key("play.fx.params.rateHz")
        .is_some());
    assert!(runner
        .menu
        .number_for_key("play.fx.params.depthPct")
        .is_some());
    assert!(runner.flush_deferred_menu_apply().unwrap().is_empty());
}

#[test]
pub(crate) fn pitch_play_fx_mapping_normalizes_complete_params() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.play_fx_selected = json!({
        "fxType": "pitch_shift",
        "targetKey": "master",
        "params": {}
    });
    runner.menu.rebuild(runner.menu_config());

    assert!(runner.apply_play_fx_menu_state());
    let params = runner.play_fx_selected["params"].as_object().unwrap();
    let mut keys = params.keys().cloned().collect::<Vec<_>>();
    keys.sort();
    assert_eq!(
        keys,
        vec!["cents", "mixPct", "semitones", "slideInMs", "slideOutMs"]
    );
    assert_eq!(params["semitones"], json!(0));
    assert_eq!(params["cents"], json!(0));
    assert_eq!(params["slideInMs"], json!(120));
    assert_eq!(params["slideOutMs"], json!(180));
    assert_eq!(params["mixPct"], json!(100));
}

#[test]
pub(crate) fn play_fx_none_exposes_type_without_effect_params() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.play_mode = "fx".into();
    runner.active_play_mode = "fx".into();
    runner.play_fx_selected["fxType"] = json!("none");
    runner.menu.rebuild(runner.menu_config());

    assert_eq!(
        runner.menu.value_for_key("play.fx.type").as_deref(),
        Some("none")
    );
    assert!(runner.menu.value_for_key("play.fx.params.rateHz").is_none());
    assert!(runner
        .menu
        .value_for_key("play.fx.params.depthPct")
        .is_none());
}

#[test]
pub(crate) fn changing_play_page_uses_static_visible_menu_rows() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    assert!(runner.menu.focus_item_key("play.page.fx"));
    runner.apply_or_schedule_menu_key("play.page.fx").unwrap();
    runner.menu.state.stack = vec![3, 2];
    runner.menu.state.cursor = 0;
    let fx_snapshot = runner.snapshot().unwrap();
    let fx_lines = fx_snapshot["display"]["lines"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|line| line.as_str())
        .collect::<Vec<_>>();
    assert!(fx_lines.iter().any(|line| line.contains("FX Type")));
    assert!(fx_lines.iter().any(|line| line.contains("Target")));

    assert!(runner.menu.focus_item_key("play.page.xy"));
    runner.apply_or_schedule_menu_key("play.page.xy").unwrap();
    runner.menu.state.stack = vec![3, 5];
    runner.menu.state.cursor = 0;
    let xy_snapshot = runner.snapshot().unwrap();
    let xy_lines = xy_snapshot["display"]["lines"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|line| line.as_str())
        .collect::<Vec<_>>();
    assert!(!xy_lines.iter().any(|line| line.contains("FX Type")));
    assert!(xy_lines.iter().any(|line| line.contains("X Axis")));
}

#[test]
pub(crate) fn entering_play_menu_activates_selected_page_and_overlay() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.play_mode = "pan".into();
    runner.active_play_mode = "none".into();
    runner.menu.rebuild(runner.menu_config());

    for _ in 0..3 {
        let _ = runner.send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
            request_snapshot: None,
        });
    }
    let entered = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();

    assert_eq!(runner.active_play_mode, "pan");
    let snapshot = snapshot_from(&entered);
    assert_eq!(snapshot["display"]["title"], "/Play");
    assert_eq!(snapshot["playMode"], "pan");
    assert_eq!(snapshot["activePlayMode"], "pan");

    let cells = led_cells(&snapshot);
    let left = cells[3].as_object().unwrap();
    let right = cells[4].as_object().unwrap();
    assert!(left["r"].as_i64().unwrap() > 100 && left["g"].as_i64().unwrap() > 100);
    assert!(right["r"].as_i64().unwrap() > 100 && right["g"].as_i64().unwrap() > 100);
}
