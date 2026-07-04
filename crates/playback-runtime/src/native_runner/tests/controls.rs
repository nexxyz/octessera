use super::*;

#[test]
pub(crate) fn controls_action_opens_help_without_platform_effects() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.state.stack = vec![5];
    runner.menu.state.cursor = 6;

    let before_path = runner.menu.current_focus_path();
    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();

    assert_eq!(runner.menu.current_focus_path(), before_path);
    assert!(messages
        .iter()
        .all(|message| !matches!(message, RunnerMessage::PlatformEffects { .. })));
    let snapshot = snapshot_from(&messages);
    assert_eq!(snapshot["display"]["title"], "Help: Basic Help");
    assert!(snapshot["display"]["lines"]
        .as_array()
        .unwrap()
        .iter()
        .all(|line| line.as_str().unwrap_or_default().chars().count() <= 18));
}

#[test]
pub(crate) fn controls_help_popup_turns_without_effects() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.state.stack = vec![5];
    runner.menu.state.cursor = 6;
    runner.open_controls_help();

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
        })
        .unwrap();

    assert!(messages
        .iter()
        .all(|message| !matches!(message, RunnerMessage::PlatformEffects { .. })));
    assert_eq!(
        snapshot_from(&messages)["display"]["title"],
        "Help: Basic Help"
    );
}

#[test]
pub(crate) fn contextual_help_does_not_change_static_navigation_memory() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.state.stack = vec![5, 3];
    runner.menu.state.cursor = 2;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_a", "pressed": true }),
        })
        .unwrap();
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    assert_eq!(runner.menu.current_label(), Some("Velocity Scale"));
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_a", "pressed": true }),
        })
        .unwrap();

    runner.menu.state.stack = vec![5, 3];
    runner.menu.state.cursor = 2;
    runner.ui.combined_modifier_held = true;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    runner.ui.combined_modifier_held = false;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_a", "pressed": true }),
        })
        .unwrap();

    runner.menu.state.stack = vec![5];
    runner.menu.state.cursor = 3;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    assert_eq!(runner.menu.current_label(), Some("Velocity Scale"));
}
