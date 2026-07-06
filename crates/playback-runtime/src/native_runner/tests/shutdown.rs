use super::*;

#[test]
pub(crate) fn system_menu_shutdown_emits_shutdown_effect_and_splash() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.oled_mode = NativeOledMode::Normal;
    runner.oled_splash_text.clear();
    runner.oled_splash_until = None;
    assert!(runner.menu.focus_item_key("system.shutdown"));

    let opened = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(
        snapshot_from(&opened)["display"]["title"],
        "Confirm Shutdown"
    );

    let messages = confirm_current_dialog(&mut runner);
    let display = &snapshot_from(&messages)["display"];
    assert_eq!(display["splash"], "shutdown");
    assert!(display["toast"]
        .as_str()
        .is_some_and(|toast| toast.contains("shutting down")));
    assert_recovery_save_then_effect(&messages, RuntimePlatformEffect::Shutdown);
}

#[test]
pub(crate) fn system_menu_reboot_emits_reboot_effect_and_shutdown_splash() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.oled_mode = NativeOledMode::Normal;
    runner.oled_splash_text.clear();
    runner.oled_splash_until = None;
    assert!(runner.menu.focus_item_key("system.reboot"));

    let opened = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(snapshot_from(&opened)["display"]["title"], "Confirm Reboot");

    let messages = confirm_current_dialog(&mut runner);
    let display = &snapshot_from(&messages)["display"];
    assert_eq!(display["splash"], "shutdown");
    assert!(display["toast"]
        .as_str()
        .is_some_and(|toast| toast.contains("rebooting")));
    assert_recovery_save_then_effect(&messages, RuntimePlatformEffect::Reboot);
}

fn assert_recovery_save_then_effect(messages: &[RunnerMessage], expected: RuntimePlatformEffect) {
    let effects = messages
        .iter()
        .flat_map(|message| match message {
            RunnerMessage::PlatformEffects { effects } => effects.as_slice(),
            _ => &[],
        })
        .collect::<Vec<_>>();
    assert!(
        matches!(effects.as_slice(), [RuntimePlatformEffect::StoreSaveRecovery { payload }, effect] if payload["runtimeConfig"].is_object() && **effect == expected)
    );
}
