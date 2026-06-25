use super::*;

#[test]
fn native_menu_edit_emits_deferred_auto_save_when_enabled() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.auto_save_default = true;
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
    let _ = runner.send(HostMessage::DeviceInput {
        input: json!({ "type": "encoder_press", "id": "main" }),
    });
    runner.transport = RuntimeTransportState::Playing;
    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": -10, "id": "main" }),
        })
        .unwrap();
    let _ = runner.send(HostMessage::DeviceInput {
        input: json!({ "type": "encoder_turn", "delta": -20, "id": "main" }),
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
fn save_default_result_lights_auto_save_indicator_and_toast_scrolls() {
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
