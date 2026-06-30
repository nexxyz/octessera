use super::*;

#[test]
pub(crate) fn dance_page_menu_edits_selected_and_active_mode() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();

    for _ in 0..3 {
        let _ = runner.send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
        });
    }
    let _ = runner.send(HostMessage::DeviceInput {
        input: json!({ "type": "encoder_press", "id": "main" }),
    });
    let _ = runner.send(HostMessage::DeviceInput {
        input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
    });
    let edit = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    assert_eq!(snapshot_from(&edit)["display"]["title"], "L4: Dance");
    assert_eq!(snapshot_from(&edit)["display"]["editing"], true);

    for mode in ["pan", "fx", "trigger-gate", "xy", "xy"] {
        let changed = runner
            .send(HostMessage::DeviceInput {
                input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
            })
            .unwrap();
        assert_eq!(snapshot_from(&changed)["danceMode"], mode);
        assert_eq!(snapshot_from(&changed)["activeDanceMode"], mode);
    }

    for mode in ["trigger-gate", "fx", "pan", "mix", "mix"] {
        let changed = runner
            .send(HostMessage::DeviceInput {
                input: json!({ "type": "encoder_turn", "delta": -1, "id": "main" }),
            })
            .unwrap();
        assert_eq!(snapshot_from(&changed)["danceMode"], mode);
        assert_eq!(snapshot_from(&changed)["activeDanceMode"], mode);
    }
}

#[test]
pub(crate) fn dance_page_fast_path_applies_immediately_without_deferred_flush() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    assert!(runner.menu.focus_item_key("danceMode"));
    runner.menu.state.stack = vec![3];
    runner.menu.state.editing = true;
    let changed = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
        })
        .unwrap();
    assert_eq!(snapshot_from(&changed)["danceMode"], "pan");
    assert_eq!(snapshot_from(&changed)["activeDanceMode"], "pan");
    assert_eq!(runner.dance_mode, "pan");
    assert_eq!(runner.active_dance_mode, "pan");
    assert!(runner.flush_deferred_menu_apply().unwrap().is_empty());
}

#[test]
pub(crate) fn dance_mode_edits_outside_dance_page_do_not_activate_overlay() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.dance_mode = "mix".into();
    runner.active_dance_mode = "none".into();
    runner.menu.rebuild(runner.menu_config());

    assert!(runner.menu.turn_key("danceMode", 1));
    runner.menu.state.stack = vec![0];
    runner.apply_or_schedule_menu_key("danceMode").unwrap();

    assert_eq!(runner.dance_mode, "pan");
    assert_eq!(runner.active_dance_mode, "none");
}

#[test]
pub(crate) fn fn_grid_context_changes_show_oled_toasts() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.part_names[2] = "rain".into();

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_fn", "pressed": true }),
        })
        .unwrap();
    let part = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 0, "y": 2 }),
        })
        .unwrap();
    assert_eq!(snapshot_from(&part)["display"]["toast"], "Part: P3 rain");

    let dance = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 7, "y": 2 }),
        })
        .unwrap();
    assert_eq!(snapshot_from(&dance)["display"]["toast"], "Dance: fx");
}

#[test]
pub(crate) fn dance_fx_type_turn_is_deferred_until_flush() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.dance_mode = "fx".into();
    runner.active_dance_mode = "fx".into();
    runner.menu.rebuild(runner.menu_config());
    assert!(runner.menu.focus_item_key("dance.fx.type"));
    runner.menu.state.stack = vec![3];
    runner.menu.state.editing = true;

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
        })
        .unwrap();

    assert_eq!(
        runner.menu.value_for_key("dance.fx.type").as_deref(),
        Some("stutter")
    );
    assert_eq!(runner.dance_fx_selected["fxType"], "none");
    runner.make_deferred_menu_apply_due_for_test();
    let flushed = runner.flush_deferred_menu_apply().unwrap();
    assert!(!flushed.is_empty());
    assert_eq!(runner.dance_fx_selected["fxType"], "stutter");
}

#[test]
pub(crate) fn dance_fx_none_exposes_type_without_effect_params() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.dance_mode = "fx".into();
    runner.active_dance_mode = "fx".into();
    runner.dance_fx_selected["fxType"] = json!("none");
    runner.menu.rebuild(runner.menu_config());

    assert_eq!(
        runner.menu.value_for_key("dance.fx.type").as_deref(),
        Some("none")
    );
    assert!(runner
        .menu
        .value_for_key("dance.fx.params.rateHz")
        .is_none());
    assert!(runner
        .menu
        .value_for_key("dance.fx.params.depthPct")
        .is_none());
}

#[test]
pub(crate) fn changing_dance_page_rebuilds_visible_menu_rows_for_selected_page() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();

    for _ in 0..3 {
        let _ = runner.send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
        });
    }
    let _ = runner.send(HostMessage::DeviceInput {
        input: json!({ "type": "encoder_press", "id": "main" }),
    });
    let _ = runner.send(HostMessage::DeviceInput {
        input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
    });
    let _ = runner.send(HostMessage::DeviceInput {
        input: json!({ "type": "encoder_press", "id": "main" }),
    });

    let fx = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 2, "id": "main" }),
        })
        .unwrap();
    let fx_snapshot = snapshot_from(&fx);
    let fx_lines = fx_snapshot["display"]["lines"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|line| line.as_str())
        .collect::<Vec<_>>();
    assert!(fx_lines.iter().any(|line| line.contains("FX Type")));
    assert!(fx_lines.iter().any(|line| line.contains("Target")));

    let xy = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 2, "id": "main" }),
        })
        .unwrap();
    let xy_snapshot = snapshot_from(&xy);
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
pub(crate) fn entering_dance_menu_activates_selected_page_and_overlay() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.dance_mode = "pan".into();
    runner.active_dance_mode = "none".into();
    runner.menu.rebuild(runner.menu_config());

    for _ in 0..3 {
        let _ = runner.send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
        });
    }
    let entered = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();

    assert_eq!(runner.active_dance_mode, "pan");
    let snapshot = snapshot_from(&entered);
    assert_eq!(snapshot["display"]["title"], "L4: Dance");
    assert_eq!(snapshot["danceMode"], "pan");
    assert_eq!(snapshot["activeDanceMode"], "pan");

    let cells = led_cells(&snapshot);
    let left = cells[3].as_object().unwrap();
    let right = cells[4].as_object().unwrap();
    assert!(left["r"].as_i64().unwrap() > 100 && left["g"].as_i64().unwrap() > 100);
    assert!(right["r"].as_i64().unwrap() > 100 && right["g"].as_i64().unwrap() > 100);
}
