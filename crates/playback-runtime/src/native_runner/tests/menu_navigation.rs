use super::*;

#[test]
fn cursor_only_navigation_does_not_apply_menu_values() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.state.stack = vec![5, 3];
    runner.ui.master_volume = 12;

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
        })
        .unwrap();

    assert_eq!(runner.ui.master_volume, 12);
    assert_eq!(snapshot_from(&messages)["settings"]["masterVolume"], 12);
}

#[test]
fn cursor_only_navigation_does_not_apply_group_browsing_side_effects() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.state.stack = vec![5];
    runner.menu.state.cursor = 0;
    runner.sync_source = SyncSource::External;

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
        })
        .unwrap();

    assert_eq!(runner.sync_source, SyncSource::External);
    assert_eq!(
        snapshot_from(&messages)["settings"]["midi"]["syncMode"],
        "external"
    );
}

#[test]
fn entering_group_does_not_apply_group_side_effects() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.state.stack = vec![5];
    runner.menu.state.cursor = 4;
    runner.sync_source = SyncSource::External;

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();

    assert_eq!(runner.menu.state.stack, vec![5, 4]);
    assert_eq!(runner.sync_source, SyncSource::External);
    assert_eq!(
        snapshot_from(&messages)["settings"]["midi"]["syncMode"],
        "external"
    );
}

#[test]
fn transport_pulse_snapshot_is_explicit() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let _ = runner.messages_with_snapshot().unwrap();

    let without_snapshot = runner
        .send(HostMessage::TransportPulseStep {
            pulses: 0,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: None,
        })
        .unwrap();
    assert!(!without_snapshot
        .iter()
        .any(|message| matches!(message, RunnerMessage::Snapshot { .. })));

    let with_snapshot = runner
        .send(HostMessage::TransportPulseStep {
            pulses: 0,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(true),
        })
        .unwrap();
    assert!(with_snapshot
        .iter()
        .any(|message| matches!(message, RunnerMessage::Snapshot { .. })));
    assert!(snapshot_from(&with_snapshot)["settings"]["instruments"].is_array());
}

#[test]
fn transport_pulse_snapshot_clears_startup_splash_after_timeout() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.oled_mode = NativeOledMode::Splash;
    runner.oled_splash_text = super::OLED_STARTUP_SPLASH_KEY.into();
    runner.oled_splash_until = Some(Instant::now() - Duration::from_millis(1));

    let messages = runner
        .send(HostMessage::TransportPulseStep {
            pulses: 0,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(true),
        })
        .unwrap();

    let display = &snapshot_from(&messages)["display"];
    assert_eq!(display["splash"], "");
    assert_eq!(display["toast"], "Help: Sh+Fn+Enter");
}

#[test]
fn behavior_config_number_param_edit_via_menu() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "life".into(),
        behavior_config: json!({ "randomCellsPerTick": 12, "randomTickInterval": 1 }),
        ..NativeRunnerConfig::default()
    })
    .unwrap();

    runner.menu.state.stack = vec![0, 0];
    runner.menu.state.cursor = 4;
    runner.menu.state.editing = true;
    let _ = runner.send(HostMessage::DeviceInput {
        input: json!({ "type": "encoder_turn", "delta": -1, "id": "main" }),
    });
    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();

    assert_eq!(runner.behavior_config["randomCellsPerTick"], 11);
    let snapshot = snapshot_from(&messages);
    assert_eq!(snapshot["display"]["editing"], false);
    assert_eq!(snapshot["display"]["title"], "L1: Life/P1: life");
}

#[test]
fn behavior_config_second_number_param_edit_via_menu() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "life".into(),
        behavior_config: json!({ "randomCellsPerTick": 0, "randomTickInterval": 1 }),
        ..NativeRunnerConfig::default()
    })
    .unwrap();

    runner.menu.state.stack = vec![0, 0];
    runner.menu.state.cursor = 5;
    runner.menu.state.editing = true;
    let _ = runner.send(HostMessage::DeviceInput {
        input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
    });

    assert_eq!(runner.behavior_config["randomTickInterval"], 2);
}

#[test]
fn behavior_config_enum_param_edits_via_menu() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "keys".into(),
        behavior_config: json!({ "quantize": "immediate" }),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.menu.state.stack = vec![0, 0];
    runner.menu.state.cursor = 4;
    runner.menu.state.editing = true;

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
        })
        .unwrap();

    assert_eq!(runner.behavior_config["quantize"], "step");
}

#[test]
fn bool_menu_items_edit_like_two_option_enums() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();

    for _ in 0..5 {
        let _ = runner.send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 4, "id": "main" }),
        });
    }
    let _ = runner.send(HostMessage::DeviceInput {
        input: json!({ "type": "encoder_press", "id": "main" }),
    });
    let _ = runner.send(HostMessage::DeviceInput {
        input: json!({ "type": "encoder_turn", "delta": 4, "id": "main" }),
    });
    let _ = runner.send(HostMessage::DeviceInput {
        input: json!({ "type": "encoder_press", "id": "main" }),
    });

    assert!(!runner.midi_enabled);

    let enter = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    assert_eq!(snapshot_from(&enter)["display"]["editing"], true);
    assert!(!runner.midi_enabled);

    let _ = runner.send(HostMessage::DeviceInput {
        input: json!({ "type": "encoder_turn", "delta": 3, "id": "main" }),
    });
    assert!(runner.midi_enabled);

    let _ = runner.send(HostMessage::DeviceInput {
        input: json!({ "type": "encoder_turn", "delta": -1, "id": "main" }),
    });
    assert!(!runner.midi_enabled);

    let exit = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    assert_eq!(snapshot_from(&exit)["display"]["editing"], false);
}

#[test]
fn system_sound_master_volume_edit_via_menu() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();

    for _ in 0..5 {
        let _ = runner.send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 2, "id": "main" }),
        });
    }
    let _ = runner.send(HostMessage::DeviceInput {
        input: json!({ "type": "encoder_press", "id": "main" }),
    });
    let _ = runner.send(HostMessage::DeviceInput {
        input: json!({ "type": "encoder_turn", "delta": 3, "id": "main" }),
    });
    let _ = runner.send(HostMessage::DeviceInput {
        input: json!({ "type": "encoder_press", "id": "main" }),
    });

    let edit = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    assert_eq!(snapshot_from(&edit)["display"]["editing"], true);

    let _ = runner.send(HostMessage::DeviceInput {
        input: json!({ "type": "encoder_turn", "delta": 26, "id": "main" }),
    });
    assert_eq!(runner.ui.master_volume, 99);

    let exit = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    assert_eq!(snapshot_from(&exit)["display"]["editing"], false);
    assert_eq!(snapshot_from(&exit)["display"]["title"], "SYS/Sound");
}

#[test]
fn fn_aux_binds_selected_param_and_aux_turn_edits_it() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.state.stack = vec![5, 3];
    runner.menu.state.cursor = 0;

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_fn", "pressed": true }),
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "aux1" }),
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_fn", "pressed": false }),
        })
        .unwrap();
    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "id": "aux1", "delta": -10 }),
        })
        .unwrap();

    assert_eq!(snapshot_from(&messages)["settings"]["masterVolume"], 63);
    assert!(snapshot_from(&messages)["display"]["toast"]
        .as_str()
        .unwrap_or("")
        .contains("T1:"));
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["auxBindings"]["aux1"]["turnKey"],
        "masterVolume"
    );
}

#[test]
fn fn_aux_binds_selected_action_and_aux_press_executes_it() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.state.stack = vec![5, 4];
    runner.menu.state.cursor = 1;

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_fn", "pressed": true }),
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "aux1" }),
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_fn", "pressed": false }),
        })
        .unwrap();
    let opened = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "aux1" }),
        })
        .unwrap();
    assert_eq!(snapshot_from(&opened)["display"]["title"], "Confirm MIDI");
    let messages = confirm_current_dialog(&mut runner);

    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects == &vec![RuntimePlatformEffect::MidiPanic]
    )));
}

#[test]
fn edit_marker_uses_compact_star_prefix() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();

    for _ in 0..5 {
        let _ = runner.send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
        });
    }
    let _ = runner.send(HostMessage::DeviceInput {
        input: json!({ "type": "encoder_press", "id": "main" }),
    });
    let _ = runner.send(HostMessage::DeviceInput {
        input: json!({ "type": "encoder_turn", "delta": 3, "id": "main" }),
    });
    let _ = runner.send(HostMessage::DeviceInput {
        input: json!({ "type": "encoder_press", "id": "main" }),
    });

    let edit = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    let snapshot = snapshot_from(&edit);
    let lines = snapshot["display"]["lines"].as_array().unwrap();
    assert!(lines.windows(2).any(|pair| {
        pair[0].as_str().unwrap_or("") == "> Master Vol:"
            && pair[1].as_str().unwrap_or("").starts_with("    * ")
    }));
}

#[test]
fn midi_sync_mode_edits_through_menu() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();

    for _ in 0..5 {
        let _ = runner.send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 4, "id": "main" }),
        });
    }
    let _ = runner.send(HostMessage::DeviceInput {
        input: json!({ "type": "encoder_press", "id": "main" }),
    });
    let _ = runner.send(HostMessage::DeviceInput {
        input: json!({ "type": "encoder_turn", "delta": 4, "id": "main" }),
    });
    let _ = runner.send(HostMessage::DeviceInput {
        input: json!({ "type": "encoder_press", "id": "main" }),
    });
    let _ = runner.send(HostMessage::DeviceInput {
        input: json!({ "type": "encoder_turn", "delta": 4, "id": "main" }),
    });

    assert_eq!(runner.sync_source, SyncSource::Internal);

    let _ = runner.send(HostMessage::DeviceInput {
        input: json!({ "type": "encoder_press", "id": "main" }),
    });
    let _ = runner.send(HostMessage::DeviceInput {
        input: json!({ "type": "encoder_press", "id": "main" }),
    });
    let changed = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
        })
        .unwrap();

    assert_eq!(runner.sync_source, SyncSource::External);
    assert_eq!(
        snapshot_from(&changed)["settings"]["midi"]["syncMode"],
        "external"
    );
}

#[test]
fn system_menu_refresh_list_emits_store_list_effect() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.state.stack = vec![5, 0, 0];
    runner.menu.state.cursor = 5;

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();

    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects == &vec![RuntimePlatformEffect::StoreListPresets]
    )));
}

#[test]
fn system_menu_midi_panic_emits_panic_effect() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.state.stack = vec![5, 4];
    runner.menu.state.cursor = 1;

    let opened = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    assert_eq!(snapshot_from(&opened)["display"]["title"], "Confirm MIDI");
    let messages = confirm_current_dialog(&mut runner);

    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects == &vec![RuntimePlatformEffect::MidiPanic]
    )));
}
