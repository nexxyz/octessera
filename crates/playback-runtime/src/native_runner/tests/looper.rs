use super::*;

fn looper_runner() -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "looper".into(),
        behavior_config: json!({ "mode": "overdub", "lengthSteps": 2 }),
        note_behaviors: vec![platform_core::NoteBehavior::Hold; 16],
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.transport = RuntimeTransportState::Playing;
    runner
}

fn looper_mode_and_step(runner: &NativeRunner) -> (String, usize) {
    match runner.engine.state() {
        platform_core::NativeBehaviorState::Looper(state) => (state.mode.clone(), state.step_index),
        _ => panic!("expected looper state"),
    }
}

fn has_note_off(messages: &[RunnerMessage]) -> bool {
    messages.iter().any(|message| match message {
        RunnerMessage::MusicalEvents { events } => events
            .iter()
            .any(|event| matches!(event, platform_core::MusicalEvent::NoteOff { .. })),
        _ => false,
    })
}

fn pulse_step(runner: &mut NativeRunner) {
    runner
        .send(HostMessage::TransportPulseStep {
            pulses: 12,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();
}

#[test]
fn looper_menu_exposes_punch_length_and_clear() {
    let runner = looper_runner();
    let l1_items = &runner.menu.root.children[0].children[0].children;
    assert!(l1_items.iter().any(|item| item.key.as_deref()
        == Some("parts.0.l1.behaviorConfig.toggleMode")
        && item.label == "Punch In/Out"));
    assert!(!l1_items
        .iter()
        .any(|item| item.key.as_deref() == Some("parts.0.l1.behaviorConfig.mode")));
    assert!(l1_items
        .iter()
        .any(|item| item.key.as_deref() == Some("parts.0.l1.behaviorConfig.lengthSteps")));
    assert!(l1_items
        .iter()
        .any(|item| item.key.as_deref() == Some("parts.0.l1.behaviorConfig.clearLoop")));
}

#[test]
fn looper_defaults_to_overdub_in_menu_and_state() {
    let runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "looper".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    assert_eq!(looper_mode_and_step(&runner).0, "overdub");
    assert!(!runner.menu.root.children[0].children[0]
        .children
        .iter()
        .any(|item| item.key.as_deref() == Some("parts.0.l1.behaviorConfig.mode")));
}

#[test]
fn looper_overdub_records_release_and_replays_each_loop() {
    let mut runner = looper_runner();
    runner.auto_save_default = true;
    let index = platform_core::grid_index(2, 3);
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 2, "y": 3 }),
        })
        .unwrap();
    assert!(runner.engine.model().unwrap().cells[index]);

    pulse_step(&mut runner);
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_release", "x": 2, "y": 3 }),
        })
        .unwrap();
    assert!(runner.engine.model().unwrap().cells[index]);

    pulse_step(&mut runner);
    assert!(!runner.engine.model().unwrap().cells[index]);
    pulse_step(&mut runner);
    assert!(runner.engine.model().unwrap().cells[index]);
}

#[test]
fn looper_clear_loop_action_releases_playback_cells_and_marks_dirty() {
    let mut runner = looper_runner();
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 2, "y": 3 }),
        })
        .unwrap();
    pulse_step(&mut runner);
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_release", "x": 2, "y": 3 }),
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
fn looper_saved_state_persists_sequence_only_when_grid_state_is_saved() {
    let mut runner = looper_runner();
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 2, "y": 3 }),
        })
        .unwrap();
    pulse_step(&mut runner);
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_release", "x": 2, "y": 3 }),
        })
        .unwrap();

    let payload = runner.config_payload();
    let l1 = &payload["runtimeConfig"]["parts"][0]["l1"];
    assert_eq!(l1["behaviorId"], "looper");
    assert_eq!(l1["behaviorConfig"]["lengthSteps"], 2);
    assert_eq!(l1["savedState"]["steps"].as_array().unwrap().len(), 2);
    assert_eq!(l1["savedState"]["steps"][0].as_array().unwrap().len(), 1);
    assert_eq!(l1["savedState"]["steps"][1].as_array().unwrap().len(), 1);

    runner.save_grid_states[0] = false;
    let payload = runner.config_payload();
    let l1 = &payload["runtimeConfig"]["parts"][0]["l1"];
    assert!(l1.get("savedState").is_none());
}

#[test]
fn looper_length_edit_preserves_recorded_sequence() {
    let mut runner = looper_runner();
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 2, "y": 3 }),
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
        })
        .unwrap();

    assert_eq!(runner.behavior_config["lengthSteps"], 3);
    let state = runner.engine.serialized_state().unwrap();
    assert_eq!(state["steps"].as_array().unwrap().len(), 3);
    assert_eq!(state["steps"][0].as_array().unwrap().len(), 1);
}

#[test]
fn looper_punch_action_toggles_mode_and_preserves_live_state() {
    let mut runner = looper_runner();
    runner.auto_save_default = true;
    pulse_step(&mut runner);
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 2, "y": 3 }),
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
        })
        .unwrap();

    let (mode, step_index) = looper_mode_and_step(&runner);
    assert_eq!(runner.behavior_config["mode"], "play");
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["parts"][0]["l1"]["behaviorConfig"]["mode"],
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
        })
        .unwrap();
    assert_eq!(looper_mode_and_step(&runner).0, "overdub");
    assert_eq!(runner.behavior_config["mode"], "overdub");
    assert_eq!(
        snapshot_from(&messages)["display"]["toast"],
        "Looper: Overdub"
    );
}

#[test]
fn looper_punch_aux_binding_uses_same_action_path() {
    let mut runner = looper_runner();
    runner.auto_save_default = true;
    runner.config_dirty = false;
    runner.aux_bindings[0] = Some(NativeAuxBinding {
        turn_key: None,
        press_action: Some(NativeMenuAction::BehaviorAction("toggleMode".into())),
    });

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "aux1" }),
        })
        .unwrap();

    assert_eq!(looper_mode_and_step(&runner).0, "play");
    assert_eq!(runner.behavior_config["mode"], "play");
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["parts"][0]["l1"]["behaviorConfig"]["mode"],
        "play"
    );
    assert!(runner.config_dirty);
    assert_eq!(snapshot_from(&messages)["display"]["toast"], "Looper: Play");
    runner.make_deferred_menu_apply_due_for_test();
    assert_deferred_save(&runner.flush_deferred_menu_apply().unwrap());
}

fn assert_deferred_save(messages: &[RunnerMessage]) {
    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects.iter().any(|effect| matches!(
                effect,
                RuntimePlatformEffect::StoreSaveDefault { mode, .. }
                    if mode.as_deref() == Some("deferred")
            ))
    )));
}

#[test]
fn looper_length_edit_after_punch_preserves_play_mode_sequence_and_phase() {
    let mut runner = looper_runner();
    pulse_step(&mut runner);
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 2, "y": 3 }),
        })
        .unwrap();
    runner.menu.state.stack = vec![0, 0];
    runner.menu.state.cursor = 4;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    assert_eq!(looper_mode_and_step(&runner), ("play".into(), 1));

    runner.menu.state.stack = vec![0, 0];
    runner.menu.state.cursor = 5;
    runner.menu.state.editing = true;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
        })
        .unwrap();

    let state = runner.engine.serialized_state().unwrap();
    assert_eq!(runner.behavior_config["mode"], "play");
    assert_eq!(looper_mode_and_step(&runner), ("play".into(), 1));
    assert_eq!(state["steps"].as_array().unwrap().len(), 3);
    assert_eq!(state["steps"][1].as_array().unwrap().len(), 1);
}

#[test]
fn looper_length_edit_preserves_play_mode_when_config_mode_is_absent() {
    let mut runner = looper_runner();
    runner
        .behavior_config
        .as_object_mut()
        .unwrap()
        .remove("mode");
    runner.part_behavior_configs[0]
        .as_object_mut()
        .unwrap()
        .remove("mode");
    runner
        .engine
        .on_input(
            platform_core::DeviceInput::BehaviorAction(platform_core::BehaviorActionInput {
                action_type: "setMode:play".into(),
            }),
            runner.bpm as f32,
        )
        .unwrap();
    pulse_step(&mut runner);
    assert_eq!(looper_mode_and_step(&runner), ("play".into(), 1));

    runner.menu.state.stack = vec![0, 0];
    runner.menu.state.cursor = 5;
    runner.menu.state.editing = true;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
        })
        .unwrap();

    assert_eq!(looper_mode_and_step(&runner), ("play".into(), 1));
}
