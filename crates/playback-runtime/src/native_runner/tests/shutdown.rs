use super::*;

#[test]
fn system_menu_shutdown_emits_shutdown_effect_and_splash() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.oled_mode = NativeOledMode::Normal;
    runner.oled_splash_text.clear();
    runner.oled_splash_until = None;
    runner.menu.state.stack = vec![5];
    runner.menu.state.cursor = 4;

    let opened = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
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
    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects == &vec![RuntimePlatformEffect::Shutdown]
    )));
}
