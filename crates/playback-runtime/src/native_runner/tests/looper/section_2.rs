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
    assert_eq!(runner.active_layer().behavior_config["mode"], "play");
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["layers"][0]["build"]["behaviorConfig"]["mode"],
        "play"
    );
    assert!(runner.config_dirty);
    assert_eq!(snapshot_from(&messages)["display"]["toast"], "Looper: Play");
    runner.make_deferred_work_due_now();
    assert_deferred_save(&runner.flush_deferred_menu_apply().unwrap());
}

pub(crate) fn assert_deferred_save(messages: &[RunnerMessage]) {
    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects.iter().any(|effect| matches!(
                effect,
                RuntimePlatformEffect::StoreSaveDefault { mode, .. }
                    if matches!(mode, Some(crate::DefaultSaveMode::Deferred))
            ))
    )));
}

#[test]
pub(crate) fn looper_length_edit_after_punch_keeps_recording_and_phase() {
    let mut runner = looper_runner();
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 2, "y": 3 }),
            request_snapshot: None,
        })
        .unwrap();
    runner.menu.open_labels(&["Build", "L1: looper"]);
    runner.menu.select_label("Punch In/Out");
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(looper_mode_and_step(&runner), ("play".into(), 1));

    runner.menu.open_labels(&["Build", "L1: looper"]);
    runner.menu.select_label("Length");
    runner.menu.state.editing = true;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_fn", "pressed": true }),
            request_snapshot: None,
        })
        .unwrap();
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_fn", "pressed": false }),
            request_snapshot: None,
        })
        .unwrap();

    let state = runner.active_engine().serialized_state().unwrap();
    assert_eq!(runner.active_layer().behavior_config["mode"], "play");
    assert_eq!(looper_mode_and_step(&runner), ("play".into(), 1));
    assert_eq!(state["steps"].as_array().unwrap().len(), 3);
    assert_eq!(state["steps"][0].as_array().unwrap().len(), 1);
}

#[test]
pub(crate) fn looper_length_edit_keeps_live_mode_when_config_has_none() {
    let mut runner = looper_runner();
    runner.layers[0]
        .behavior_config
        .as_object_mut()
        .unwrap()
        .remove("mode");
    runner.layers[runner.active_layer_index]
        .engine
        .on_input(
            platform_core::DeviceInput::BehaviorAction(platform_core::BehaviorActionInput {
                action_type: "setMode:play".into(),
            }),
            runner.transport.bpm as f32,
        )
        .unwrap();
    assert_eq!(looper_mode_and_step(&runner), ("play".into(), 1));

    runner.menu.open_labels(&["Build", "L1: looper"]);
    runner.menu.select_label("Length");
    runner.menu.state.editing = true;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_fn", "pressed": true }),
            request_snapshot: None,
        })
        .unwrap();
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_fn", "pressed": false }),
            request_snapshot: None,
        })
        .unwrap();

    assert_eq!(looper_mode_and_step(&runner), ("play".into(), 1));
}

#[test]
pub(crate) fn a_saved_looper_with_steps_past_its_length_loads_and_saves_them_again() {
    let runner = looper_runner();
    let mut patch =
        crate::native_runner::split_local_system_patch_documents(&runner.config_payload())
            .unwrap()
            .patch;
    let steps = json!([[{ "kind": "press", "tile": 26 }], [{ "kind": "press", "tile": 27 }], []]);
    let build = &mut patch["runtimeConfig"]["layers"][0]["build"];
    build["behaviorConfig"]["lengthSteps"] = json!(1);
    build["savedState"] = json!({ "lengthSteps": 1, "mode": "overdub", "steps": steps });

    let mut fresh = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    fresh
        .apply_store_result(RuntimeStoreResult::LoadDefaultResult {
            payload: Some(patch),
        })
        .unwrap();

    let saved = crate::native_runner::split_local_system_patch_documents(&fresh.config_payload())
        .unwrap()
        .patch;
    assert_eq!(
        saved["runtimeConfig"]["layers"][0]["build"]["savedState"]["lengthSteps"],
        1
    );
    assert_eq!(
        saved["runtimeConfig"]["layers"][0]["build"]["savedState"]["steps"],
        steps
    );
}
