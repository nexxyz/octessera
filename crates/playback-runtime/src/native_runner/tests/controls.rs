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
pub(crate) fn system_info_action_opens_loading_popup_and_requests_info() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    assert!(runner.menu.focus_item_key("system.info"));

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();

    assert!(matches!(
        messages.iter().find(|message| matches!(
            message,
            RunnerMessage::PlatformEffects { .. }
        )),
        Some(RunnerMessage::PlatformEffects { effects })
            if effects == &vec![RuntimePlatformEffect::SystemInfoRequest]
    ));
    let snapshot = snapshot_from(&messages);
    assert_eq!(snapshot["display"]["title"], "System Info");
    assert_eq!(snapshot["display"]["lines"][0], "Loading info...");
}

#[test]
pub(crate) fn system_info_popup_formats_scrolls_and_dismisses_natively() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.open_system_info();
    runner
        .apply_store_result(RuntimeStoreResult::Identified {
            result: Box::new(RuntimeStoreResult::SystemInfoResult {
                info: RuntimeSystemInfo {
                    os: "Linux".into(),
                    os_version: "A very long operating system version".into(),
                    octessera_version: "0.7.0".into(),
                    primary_ip: Some("192.168.1.100".into()),
                    primary_mac: Some("aa:bb:cc:dd:ee:ff".into()),
                    hostname: "octessera-pi".into(),
                    board_profile: "raspberry-pi-zero-2w".into(),
                },
            }),
            request_id: "platform-1".into(),
            revision: None,
        })
        .unwrap();

    let initial = runner.messages_with_snapshot().unwrap();
    assert!(snapshot_from(&initial)["display"]["lines"]
        .as_array()
        .unwrap()
        .iter()
        .all(|line| line.as_str().unwrap().chars().count() <= 18));
    let scrolled = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "id": "main", "delta": 1 }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(runner.display.system_info_modal.as_ref().unwrap().scroll, 1);
    assert!(snapshot_from(&scrolled)["display"]["lines"]
        .as_array()
        .unwrap()
        .iter()
        .all(|line| line.as_str().unwrap().chars().count() <= 18));

    let closed = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    assert!(runner.display.system_info_modal.is_none());
    assert_eq!(snapshot_from(&closed)["display"]["title"], "MENU");
}

#[test]
pub(crate) fn system_info_popup_distinguishes_unavailable_state() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.open_system_info();
    runner
        .apply_store_result(RuntimeStoreResult::SystemInfoError {
            error: RuntimeSystemInfoError::unavailable("network details unavailable"),
        })
        .unwrap();
    let snapshot = snapshot_from(&runner.messages_with_snapshot().unwrap());
    assert_eq!(snapshot["display"]["lines"][0], "Unavailable: netwo");
}

#[test]
pub(crate) fn physical_owner_fast_handlers_apply_voice_limit_and_sparks_xy_keys() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.modulation_process_calls = 0;

    turn_menu_key_physical(&mut runner, "sound.voiceStealingMode", 1);
    assert_eq!(runner.voice_stealing_mode, "auto-hard");
    assert_eq!(runner.audio_config_revision, 1);

    turn_menu_key_physical(&mut runner, "sparks.xy.release", 1);
    assert_eq!(runner.xy_release, "reset-center");

    runner.instruments[0].volume = 50;
    runner.xy_touch = NativeXyTouch {
        x: 0.25,
        y: 0.5,
        display_x: 0.25,
        display_y: 0.5,
        active: true,
    };
    runner.xy_x_binding = Some(NativeParamBinding {
        key: "instruments.0.mixer.volume".into(),
        label: Some("Volume".into()),
        kind: "number".into(),
        min: Some(0.0),
        max: Some(100.0),
        step: Some(1.0),
        user_min: None,
        user_max: None,
        options: vec![],
        invert: false,
    });
    runner.refresh_xy_runtime_sources();
    runner.process_dirty_modulation_step(false).unwrap();
    assert_eq!(runner.instruments[0].volume, 25);

    turn_menu_key_physical(&mut runner, "sparks.xy.invertX", 1);
    assert!(runner.xy_invert_x);
    assert_eq!(runner.instruments[0].volume, 75);
    turn_menu_key_physical(&mut runner, "sparks.xy.invertY", 1);
    assert!(runner.xy_invert_y);
}

fn turn_menu_key_physical(runner: &mut NativeRunner, key: &str, delta: i8) {
    assert!(runner.menu.focus_item_key(key));
    runner.menu.state.editing = true;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": delta, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
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
