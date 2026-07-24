use super::*;

#[test]
pub(crate) fn looper_menu_exposes_punch_length_and_clear() {
    let runner = looper_runner();
    let worlds_items = &runner.menu.root.children[0].children[0].children;
    assert!(worlds_items.iter().any(|item| item.key.as_deref()
        == Some("layers.0.worlds.behaviorConfig.toggleMode")
        && item.label == "Punch In/Out"));
    assert!(!worlds_items
        .iter()
        .any(|item| item.key.as_deref() == Some("layers.0.worlds.behaviorConfig.mode")));
    assert!(worlds_items
        .iter()
        .any(|item| item.key.as_deref() == Some("layers.0.worlds.behaviorConfig.lengthSteps")));
    assert!(worlds_items
        .iter()
        .any(|item| item.key.as_deref() == Some("layers.0.worlds.behaviorConfig.clearLoop")));
}

#[test]
pub(crate) fn looper_defaults_to_overdub_in_menu_and_state() {
    let runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "looper".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    assert_eq!(looper_mode_and_step(&runner).0, "overdub");
    assert!(!runner.menu.root.children[0].children[0]
        .children
        .iter()
        .any(|item| item.key.as_deref() == Some("layers.0.worlds.behaviorConfig.mode")));
}

#[test]
pub(crate) fn looper_overdub_records_release_and_replays_each_loop() {
    let mut runner = looper_runner();
    runner.auto_save_default = true;
    let index = platform_core::grid_index(2, 3);
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 2, "y": 3 }),
            request_snapshot: None,
        })
        .unwrap();
    assert!(runner.engine.model().unwrap().cells[index]);

    pulse_step(&mut runner);
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_release", "x": 2, "y": 3 }),
            request_snapshot: None,
        })
        .unwrap();
    assert!(runner.engine.model().unwrap().cells[index]);

    pulse_step(&mut runner);
    assert!(!runner.engine.model().unwrap().cells[index]);
    pulse_step(&mut runner);
    assert!(runner.engine.model().unwrap().cells[index]);
}

#[test]
pub(crate) fn looper_clear_loop_action_releases_playback_cells_and_marks_dirty() {
    let mut runner = looper_runner();
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 2, "y": 3 }),
            request_snapshot: None,
        })
        .unwrap();
    pulse_step(&mut runner);
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_release", "x": 2, "y": 3 }),
            request_snapshot: None,
        })
        .unwrap();
    pulse_step(&mut runner);
    pulse_step(&mut runner);
    let index = platform_core::grid_index(2, 3);
    assert!(runner.engine.model().unwrap().cells[index]);

    runner.config_dirty = false;
    runner.menu.state.stack = vec![0, 0];
    runner.menu.state.cursor = 6;
    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    assert!(!runner.engine.model().unwrap().cells[index]);
    assert_eq!(snapshot_from(&messages)["display"]["toast"], "Loop cleared");
    assert!(runner.config_dirty);
    assert!(has_note_off(&messages));
    let state = runner.engine.serialized_state().unwrap();
    assert!(state["steps"]
        .as_array()
        .unwrap()
        .iter()
        .all(|step| step.as_array().unwrap().is_empty()));
}

#[test]
pub(crate) fn looper_saved_state_persists_sequence_only_when_grid_state_is_saved() {
    let mut runner = looper_runner();
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 2, "y": 3 }),
            request_snapshot: None,
        })
        .unwrap();
    pulse_step(&mut runner);
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_release", "x": 2, "y": 3 }),
            request_snapshot: None,
        })
        .unwrap();

    let payload = runner.config_payload();
    let worlds = &payload["runtimeConfig"]["layers"][0]["worlds"];
    assert_eq!(worlds["behaviorId"], "looper");
    assert_eq!(worlds["behaviorConfig"]["lengthSteps"], 2);
    assert_eq!(worlds["savedState"]["steps"].as_array().unwrap().len(), 2);
    assert_eq!(
        worlds["savedState"]["steps"][0].as_array().unwrap().len(),
        1
    );
    assert_eq!(
        worlds["savedState"]["steps"][1].as_array().unwrap().len(),
        1
    );

    runner.save_grid_states[0] = false;
    let payload = runner.config_payload();
    let worlds = &payload["runtimeConfig"]["layers"][0]["worlds"];
    assert!(worlds.get("savedState").is_none());
}

#[test]
pub(crate) fn looper_length_edit_reinitializes_sequence_from_config() {
    let mut runner = looper_runner();
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 2, "y": 3 }),
            request_snapshot: None,
        })
        .unwrap();
    let before = runner.engine.serialized_state().unwrap();
    assert_eq!(before["steps"][0].as_array().unwrap().len(), 1);

    runner.menu.state.stack = vec![0, 0];
    runner.menu.state.cursor = 5;
    runner.menu.state.editing = true;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();

    assert_eq!(runner.behavior_config["lengthSteps"], 3);
    let state = runner.engine.serialized_state().unwrap();
    assert_eq!(state["steps"].as_array().unwrap().len(), 3);
    assert!(state["steps"][0].as_array().unwrap().is_empty());
}

#[test]
pub(crate) fn looper_punch_action_toggles_mode_and_preserves_live_state() {
    let mut runner = looper_runner();
    runner.auto_save_default = true;
    pulse_step(&mut runner);
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 2, "y": 3 }),
            request_snapshot: None,
        })
        .unwrap();
    let state_before = runner.engine.serialized_state().unwrap();
    assert_eq!(state_before["steps"][1].as_array().unwrap().len(), 1);
    assert_eq!(looper_mode_and_step(&runner).1, 1);
    runner.config_dirty = false;

    runner.menu.state.stack = vec![0, 0];
    runner.menu.state.cursor = 4;
    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();

    let (mode, step_index) = looper_mode_and_step(&runner);
    assert_eq!(runner.behavior_config["mode"], "play");
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["layers"][0]["worlds"]["behaviorConfig"]["mode"],
        "play"
    );
    assert!(runner.config_dirty);
    assert_eq!(mode, "play");
    assert_eq!(step_index, 1);
    assert_eq!(snapshot_from(&messages)["display"]["toast"], "Looper: Play");
    runner.make_deferred_menu_apply_due_for_test();
    assert_deferred_save(&runner.flush_deferred_menu_apply().unwrap());
    let state_after = runner.engine.serialized_state().unwrap();
    assert_eq!(state_after["steps"][1].as_array().unwrap().len(), 1);
    assert!(runner.engine.model().unwrap().cells[platform_core::grid_index(2, 3)]);

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(looper_mode_and_step(&runner).0, "overdub");
    assert_eq!(runner.behavior_config["mode"], "overdub");
    assert_eq!(
        snapshot_from(&messages)["display"]["toast"],
        "Looper: Overdub"
    );
}
