use super::*;

#[test]
pub(crate) fn system_sound_master_volume_edit_via_menu() {
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
pub(crate) fn fn_aux_binds_selected_param_and_aux_turn_edits_it() {
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
pub(crate) fn fn_aux_binds_selected_action_and_aux_press_executes_it() {
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
pub(crate) fn edit_marker_uses_compact_star_prefix() {
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
pub(crate) fn midi_sync_mode_edits_through_menu() {
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
pub(crate) fn system_menu_refresh_list_emits_store_list_effect() {
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
pub(crate) fn system_menu_midi_panic_emits_panic_effect() {
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

#[test]
pub(crate) fn reset_behavior_action_shows_feedback() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();

    runner
        .execute_menu_action(NativeMenuAction::ResetBehavior)
        .unwrap();

    assert_eq!(runner.toast.as_ref().unwrap().message, "Behavior reset");
}
