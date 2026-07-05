use super::*;

#[test]
pub(crate) fn native_menu_edit_emits_deferred_auto_save_when_enabled() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.auto_save_default = true;
    for _ in 0..5 {
        let _ = runner.send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 2, "id": "main" }),
            request_snapshot: None,
        });
    }
    let _ = runner.send(HostMessage::DeviceInput {
        input: json!({ "type": "encoder_press", "id": "main" }),
        request_snapshot: None,
    });
    let _ = runner.send(HostMessage::DeviceInput {
        input: json!({ "type": "encoder_turn", "delta": 3, "id": "main" }),
        request_snapshot: None,
    });
    let _ = runner.send(HostMessage::DeviceInput {
        input: json!({ "type": "encoder_press", "id": "main" }),
        request_snapshot: None,
    });
    let _ = runner.send(HostMessage::DeviceInput {
        input: json!({ "type": "encoder_press", "id": "main" }),
        request_snapshot: None,
    });
    runner.transport = RuntimeTransportState::Playing;
    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": -10, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    let _ = runner.send(HostMessage::DeviceInput {
        input: json!({ "type": "encoder_turn", "delta": -20, "id": "main" }),
        request_snapshot: None,
    });

    assert!(!messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if matches!(
                effects.as_slice(),
                [RuntimePlatformEffect::StoreSaveDefault { mode, .. }]
                    if mode.as_deref() == Some("deferred")
            )
    )));

    runner.make_deferred_menu_apply_due_for_test();
    let messages = runner.flush_deferred_menu_apply().unwrap();
    let saved_payload = messages
        .iter()
        .find_map(|message| match message {
            RunnerMessage::PlatformEffects { effects } => {
                effects.iter().find_map(|effect| match effect {
                    RuntimePlatformEffect::StoreSaveDefault { payload, mode }
                        if mode.as_deref() == Some("deferred") =>
                    {
                        Some(payload)
                    }
                    _ => None,
                })
            }
            _ => None,
        })
        .expect("deferred save payload");
    assert_eq!(
        saved_payload["runtimeConfig"]["masterVolume"],
        runner.ui.master_volume
    );
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
pub(crate) fn save_default_result_lights_auto_save_indicator_and_toast_scrolls() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.oled_mode = NativeOledMode::Normal;
    runner.oled_splash_text.clear();
    runner.oled_splash_until = None;
    runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::SaveDefaultResult {
                ok: true,
                is_auto: Some(true),
            },
        })
        .unwrap();
    let snapshot = runner.snapshot().unwrap();
    assert_eq!(snapshot["settings"]["autoSaveFlash"], "flash");
    assert!(snapshot["display"]["toast"]
        .as_str()
        .unwrap()
        .contains("Saved"));

    runner.set_toast_for_test(
        "This is a very long toaster message that must scroll across the reserved status row",
    );
    let first = runner.snapshot().unwrap()["display"]["toast"].clone();
    runner.advance_toast_for_test();
    let second = runner.snapshot().unwrap()["display"]["toast"].clone();
    assert_ne!(first, second);
}

#[test]
pub(crate) fn text_edit_turns_are_deferred_until_flush_or_exit() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.auto_save_default = true;
    runner.menu.state.stack = vec![0, 0];
    runner.menu.state.cursor = 2;
    runner.menu.press();

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 27, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();

    assert_eq!(runner.part_names[0], "life");
    assert!(!messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects.iter().any(|effect| matches!(
                effect,
                RuntimePlatformEffect::StoreSaveDefault { .. }
            ))
    )));

    runner.make_deferred_menu_apply_due_for_test();
    runner.flush_deferred_menu_apply().unwrap();

    assert_eq!(runner.part_names[0], "lifea");
    assert!(!runner.part_auto_names[0]);
}

#[test]
pub(crate) fn deferred_text_edit_survives_leaving_name_row_before_flush() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.state.stack = vec![0, 0];
    runner.menu.state.cursor = 2;
    runner.menu.press();

    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 27, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    runner.menu.back();
    runner.menu.back();

    runner.make_deferred_menu_apply_due_for_test();
    runner.flush_deferred_menu_apply().unwrap();

    assert_eq!(runner.part_names[0], "lifea");
    assert!(!runner.part_auto_names[0]);
}

#[test]
pub(crate) fn deferred_instrument_name_edit_survives_leaving_row() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();

    assert!(runner.menu.focus_item_key("instruments.0.name"));
    runner.menu.state.editing = true;
    runner.menu.turn_key("instruments.0.name", 27);
    runner
        .apply_or_schedule_menu_key("instruments.0.name")
        .unwrap();
    runner.menu.back();

    assert_eq!(runner.instruments[0].name, "Synth");

    runner.make_deferred_menu_apply_due_for_test();
    runner.flush_deferred_menu_apply().unwrap();

    assert_eq!(runner.instruments[0].name, "tynth");
    assert!(!runner.instruments[0].auto_name);
}

#[test]
pub(crate) fn deferred_bus_name_edit_survives_leaving_row() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.fx_buses[0].slot1_type = "delay".into();
    runner.fx_buses[0].slot2_type = "duck".into();
    runner.fx_buses[0].name = "Delay+Duck".into();
    runner.menu.rebuild(runner.menu_config());

    assert!(runner.menu.focus_item_key("mixer.buses.0.name"));
    runner.menu.state.editing = true;
    runner.menu.turn_key("mixer.buses.0.name", 27);
    runner
        .apply_or_schedule_menu_key("mixer.buses.0.name")
        .unwrap();
    runner.menu.back();

    assert_eq!(runner.fx_buses[0].name, "Delay+Duck");

    runner.make_deferred_menu_apply_due_for_test();
    runner.flush_deferred_menu_apply().unwrap();

    assert_eq!(runner.fx_buses[0].name, "eelay+Duck");
    assert!(!runner.fx_buses[0].auto_name);
}
