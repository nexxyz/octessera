use super::*;

#[test]
pub(crate) fn controls_action_opens_help_without_platform_effects() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    assert!(runner.menu.focus_item_key("system.controlsHelp"));

    let before_path = runner.menu.current_focus_path();
    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
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
    runner.menu.state.cursor = 5;
    runner.open_controls_help();

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
            request_snapshot: None,
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
            request_snapshot: None,
        })
        .unwrap();
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(runner.menu.current_label(), Some("Velocity Scale"));
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_a", "pressed": true }),
            request_snapshot: None,
        })
        .unwrap();

    runner.menu.state.stack = vec![5, 3];
    runner.menu.state.cursor = 2;
    runner.display.ui.combined_modifier_held = true;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    runner.display.ui.combined_modifier_held = false;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_a", "pressed": true }),
            request_snapshot: None,
        })
        .unwrap();

    runner.menu.state.stack = vec![5];
    runner.menu.state.cursor = 3;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(runner.menu.current_label(), Some("Velocity Scale"));
}

#[test]
pub(crate) fn entering_keyed_behavior_category_only_enters_group() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    assert!(runner.menu.focus_item_key("behavior.category.play"));
    let behavior_before = runner.behavior.id().to_string();

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();

    assert_eq!(runner.behavior.id(), behavior_before);
    assert_eq!(runner.menu.current_label(), Some(".."));
    assert!(messages.iter().all(|message| !matches!(
        message,
        RunnerMessage::PlatformEffects { .. }
            | RunnerMessage::MusicalEvents { .. }
            | RunnerMessage::MidiEvents { .. }
    )));
}

#[test]
pub(crate) fn behavior_category_and_leaf_help_targets_are_keyed() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    assert!(runner.menu.focus_item_key("behavior.category.play"));
    let category_target = runner.menu.current_help_target().unwrap();
    assert_eq!(category_target.key, "key:behavior.category.play");
    assert_eq!(category_target.kind, "group");

    runner.menu.press();
    while runner.menu.current_label() != Some("keys") {
        runner.menu.turn(1);
    }
    let leaf_target = runner.menu.current_help_target().unwrap();
    assert_eq!(leaf_target.key, "action:behavior_select:keys");
    assert_eq!(leaf_target.kind, "action");
}

#[test]
pub(crate) fn non_active_layer_behavior_selector_has_help() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();

    assert!(runner.menu.focus_item_key("layers.1.behaviorId"));
    let target = runner.menu.current_help_target().unwrap();
    assert_eq!(target.key, "key:layers.*.behaviorId");
    assert_eq!(target.kind, "group");

    let entry = crate::native_help::resolve_native_help_entry(&target).unwrap();
    assert_eq!(entry.key, "key:layers.*.behaviorId");
}
