use super::*;

#[test]
pub(crate) fn looper_punch_aux_binding_uses_same_action_path() {
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
            request_snapshot: None,
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

pub(crate) fn assert_deferred_save(messages: &[RunnerMessage]) {
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
pub(crate) fn looper_length_edit_after_punch_preserves_play_mode_sequence_and_phase() {
    let mut runner = looper_runner();
    pulse_step(&mut runner);
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 2, "y": 3 }),
            request_snapshot: None,
        })
        .unwrap();
    runner.menu.state.stack = vec![0, 0];
    runner.menu.state.cursor = 4;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(looper_mode_and_step(&runner), ("play".into(), 1));

    runner.menu.state.stack = vec![0, 0];
    runner.menu.state.cursor = 5;
    runner.menu.state.editing = true;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();

    let state = runner.engine.serialized_state().unwrap();
    assert_eq!(runner.behavior_config["mode"], "play");
    assert_eq!(looper_mode_and_step(&runner), ("play".into(), 1));
    assert_eq!(state["steps"].as_array().unwrap().len(), 3);
    assert_eq!(state["steps"][1].as_array().unwrap().len(), 1);
}

#[test]
pub(crate) fn looper_length_edit_preserves_play_mode_when_config_mode_is_absent() {
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
            request_snapshot: None,
        })
        .unwrap();

    assert_eq!(looper_mode_and_step(&runner), ("play".into(), 1));
}
