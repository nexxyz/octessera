use super::*;

#[test]
pub(crate) fn changing_behavior_keeps_menu_location() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();

    let level1 = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    assert_eq!(snapshot_from(&level1)["display"]["title"], "L1: Life");

    let part = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    assert_eq!(
        snapshot_from(&part)["display"]["title"],
        "L1: Life/P1: life"
    );

    let edit_behavior = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    assert_eq!(
        snapshot_from(&edit_behavior)["display"]["title"],
        "L1: Life/P1: life"
    );
    assert_eq!(snapshot_from(&edit_behavior)["display"]["editing"], true);

    let changed = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 2, "id": "main" }),
        })
        .unwrap();
    let changed_snapshot = snapshot_from(&changed);
    assert_eq!(changed_snapshot["display"]["title"], "L1: Life/P1: keys");
    assert_eq!(changed_snapshot["display"]["editing"], true);
    assert_eq!(changed_snapshot["activeBehavior"], "keys");

    runner.make_deferred_menu_apply_due_for_test();
    let _ = runner.flush_deferred_menu_apply().unwrap();
    let flushed = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    let snapshot = snapshot_from(&flushed);
    assert_eq!(snapshot["display"]["title"], "L1: Life/P1: keys");
    assert_eq!(snapshot["display"]["editing"], false);
    assert_eq!(snapshot["activeBehavior"], "keys");
}

#[test]
pub(crate) fn button_a_release_does_not_navigate_back() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.state.stack = vec![0];
    runner.menu.state.cursor = 2;

    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_a", "pressed": false }),
        })
        .unwrap();

    assert_eq!(runner.menu.state.stack, vec![0]);
    assert_eq!(runner.menu.state.cursor, 2);
}

#[test]
pub(crate) fn startup_splash_closes_into_help_toast() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.oled_mode = NativeOledMode::Splash;
    runner.oled_splash_text = super::OLED_STARTUP_SPLASH_KEY.into();
    runner.oled_splash_until = Some(Instant::now() + Duration::from_secs(1));

    let messages = runner.messages_with_snapshot().unwrap();
    let display = &snapshot_from(&messages)["display"];
    assert_eq!(display["splash"], "startup");

    runner.oled_splash_until = Some(Instant::now() - Duration::from_millis(1));
    let messages = runner.messages_with_snapshot().unwrap();
    let display = &snapshot_from(&messages)["display"];
    assert_eq!(display["splash"], "");
    assert_eq!(display["off"], false);
    assert_eq!(display["toast"], "Help: Sh+Fn+Enter");
}

#[test]
pub(crate) fn startup_splash_blocks_input_until_timeout() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.oled_mode = NativeOledMode::Splash;
    runner.oled_splash_text = super::OLED_STARTUP_SPLASH_KEY.into();
    runner.oled_splash_until = Some(Instant::now() + Duration::from_secs(1));
    let _ = runner.messages_with_snapshot().unwrap();

    let blocked = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
        })
        .unwrap();

    assert_eq!(runner.menu.state.cursor, 0);
    assert_eq!(snapshot_from(&blocked)["display"]["splash"], "startup");

    runner.oled_splash_until = Some(Instant::now() - Duration::from_millis(1));
    let unblocked = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
        })
        .unwrap();

    assert_eq!(runner.menu.state.cursor, 1);
    assert_eq!(snapshot_from(&unblocked)["display"]["splash"], "");
}

#[test]
pub(crate) fn screen_sleep_splashes_then_turns_oled_off_and_wake_input_shows_wakeup_screen() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.oled_mode = NativeOledMode::Normal;
    runner.oled_splash_text.clear();
    runner.oled_splash_until = None;
    runner.ui.screen_sleep_seconds = 1;
    runner.last_interaction_at = Instant::now() - Duration::from_secs(2);

    let messages = runner.messages_with_snapshot().unwrap();
    let display = &snapshot_from(&messages)["display"];
    assert_eq!(display["splash"], "sleep");
    assert_eq!(display["off"], false);

    runner.oled_splash_until = Some(Instant::now() - Duration::from_millis(1));
    let messages = runner.messages_with_snapshot().unwrap();
    let display = &snapshot_from(&messages)["display"];
    assert_eq!(display["off"], true);

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
        })
        .unwrap();
    let display = &snapshot_from(&messages)["display"];
    assert_eq!(display["off"], false);
    assert_eq!(display["splash"], "");
    assert_eq!(runner.menu.state.cursor, 0);

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
        })
        .unwrap();
    let display = &snapshot_from(&messages)["display"];
    assert_eq!(display["off"], false);
    assert_eq!(display["splash"], "");
    assert_eq!(runner.menu.state.cursor, 1);
}
