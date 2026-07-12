use super::*;

#[test]
pub(crate) fn output_buffer_frames_edits_into_config_payload_with_restart_toast() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    assert!(runner.menu.focus_item_key("sound.audioOutputBufferFrames"));
    runner.menu.state.editing = true;

    runner.menu.turn(1);
    runner.apply_menu_state().unwrap();

    assert_eq!(
        runner.config_payload()["runtimeConfig"]["sound"]["audioOutputBufferFrames"],
        512
    );
    assert_eq!(
        runner.snapshot().unwrap()["display"]["toast"],
        "Restart device to apply"
    );
}

#[test]
pub(crate) fn back_from_changed_output_buffer_opens_reboot_confirmation() {
    let mut runner = changed_output_buffer_runner();

    let messages = press_back(&mut runner);
    let snapshot = snapshot_from(&messages);

    assert_eq!(snapshot["display"]["title"], "Confirm Reboot");
    assert_eq!(snapshot["display"]["lines"][1], "> Cancel");
    assert_eq!(snapshot["display"]["lines"][2], "  Confirm");
    assert_eq!(snapshot["display"]["toast"], "");
}

#[test]
pub(crate) fn output_buffer_reboot_confirmation_cancel_does_not_emit_reboot() {
    let mut runner = changed_output_buffer_runner();
    let _ = press_back(&mut runner);

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();

    assert!(!messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects.contains(&RuntimePlatformEffect::Reboot)
    )));
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["sound"]["audioOutputBufferFrames"],
        512
    );
}

#[test]
pub(crate) fn output_buffer_reboot_confirmation_emits_reboot_and_shutdown_splash() {
    let mut runner = changed_output_buffer_runner();
    runner.oled_mode = NativeOledMode::Normal;
    runner.oled_splash_text.clear();
    runner.oled_splash_until = None;
    let _ = press_back(&mut runner);
    runner.confirm_dialog.as_mut().unwrap().cursor = 1;

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    let display = &snapshot_from(&messages)["display"];

    assert_eq!(display["splash"], "shutdown");
    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects == &vec![RuntimePlatformEffect::Reboot]
    )));
}

fn changed_output_buffer_runner() -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    assert!(runner.menu.focus_item_key("sound.audioOutputBufferFrames"));
    runner.menu.state.editing = true;
    runner.menu.turn(1);
    runner.apply_menu_state().unwrap();
    runner
}

fn press_back(runner: &mut NativeRunner) -> Vec<RunnerMessage> {
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_a", "pressed": true }),
            request_snapshot: None,
        })
        .unwrap()
}

#[test]
pub(crate) fn synth_gain_edits_into_config_payload() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.state.stack = vec![2, 0, 0, 2, 4];
    runner.menu.state.cursor = 0;

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": -10, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();

    assert_eq!(
        runner.config_payload()["runtimeConfig"]["instruments"][0]["synth"]["amp"]["gainPct"],
        70
    );
}

#[test]
pub(crate) fn sampler_tune_edits_into_config_payload() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].kind = "sampler".into();
    runner.instruments[0].name = "sampler".into();
    runner.menu.rebuild(runner.menu_config());
    runner.menu.state.stack = vec![2, 0, 0, 2];
    runner.menu.state.cursor = 4;

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 7, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();

    assert_eq!(
        runner.config_payload()["runtimeConfig"]["instruments"][0]["sample"]["tuneSemis"],
        7
    );
}

#[test]
pub(crate) fn sampler_extended_params_edit_into_config_payload() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].kind = "sampler".into();
    runner.instruments[0].name = "sampler".into();
    runner.menu.rebuild(runner.menu_config());

    runner.menu.state.stack = vec![2, 0, 0, 2];
    runner.menu.state.cursor = 6;
    runner.menu.state.editing = true;
    runner.menu.turn(-20);
    runner.apply_menu_state().unwrap();

    runner.menu.state.cursor = 7;
    runner.menu.turn(1);
    runner.apply_menu_state().unwrap();

    runner.menu.state.stack = vec![2, 0, 0, 2, 7];
    runner.menu.state.cursor = 0;
    runner.menu.state.editing = true;
    runner.menu.turn(-10);
    runner.apply_menu_state().unwrap();

    runner.menu.state.stack = vec![2, 0, 0, 2, 9];
    runner.menu.state.cursor = 0;
    runner.menu.state.editing = true;
    runner.menu.turn(1);
    runner.apply_menu_state().unwrap();
    runner.menu.state.cursor = 1;
    runner.menu.turn(-10);
    runner.apply_menu_state().unwrap();

    runner.menu.state.stack = vec![2, 0, 0, 2];
    runner.menu.state.cursor = 10;
    runner.menu.state.editing = true;
    runner.menu.turn(-25);
    runner.apply_menu_state().unwrap();

    runner.menu.state.stack = vec![2, 0, 0, 2, 11];
    runner.menu.state.cursor = 0;
    runner.menu.state.editing = true;
    runner.menu.turn(4);
    runner.apply_menu_state().unwrap();

    let sample = &runner.config_payload()["runtimeConfig"]["instruments"][0]["sample"];
    assert_eq!(sample["baseVelocity"], 80);
    assert_eq!(sample["velocityLevelsEnabled"], true);
    assert_eq!(sample["velocityLevels"]["high"], 110);
    assert_eq!(sample["filter"]["type"], "highpass");
    assert_eq!(sample["filter"]["cutoffHz"], 6548);
    assert_eq!(sample["amp"]["velocitySensitivityPct"], 75);
    assert_eq!(sample["ampEnv"]["attackMs"], 25);
}

#[test]
pub(crate) fn fx_bus_slot_type_edits_into_config_payload() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.turn_key("mixer.buses.0.slot1.type", 1);
    runner.apply_menu_state().unwrap();

    assert_eq!(
        runner.config_payload()["runtimeConfig"]["mixer"]["buses"][0]["slot1"]["type"],
        "tremolo"
    );
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["mixer"]["buses"][0]["slot1"]["params"]["rateHz"],
        4.0
    );
}

#[test]
pub(crate) fn global_fx_slot_type_edits_into_config_payload() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.turn_key("mixer.master.slots.0.type", 1);
    runner.apply_menu_state().unwrap();

    assert_eq!(
        runner.config_payload()["runtimeConfig"]["mixer"]["master"]["slots"][0]["type"],
        "vinyl"
    );
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["mixer"]["master"]["slots"][0]["params"]
            ["cracklePct"],
        8
    );
}

#[test]
pub(crate) fn fx_params_edit_into_config_payload() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner
        .apply_config_payload(json!({
            "runtimeConfig": {
                "mixer": {
                    "buses": [{ "slot1": { "type": "delay", "params": { "timeMs": 250, "feedback": 0.35, "mixPct": 35 } } }],
                    "master": { "slots": [{ "type": "distortion", "params": { "drive": 2.5, "clip": 0.6, "mixPct": 100 } }] }
                }
            }
        }))
        .unwrap();
    runner.menu.rebuild(runner.menu_config());

    runner
        .menu
        .turn_key("mixer.buses.0.slot1.params.feedback", 1);
    runner.menu.turn_key("mixer.master.slots.0.params.clip", 1);
    runner.apply_menu_state().unwrap();

    let payload = runner.config_payload();
    assert_eq!(
        payload["runtimeConfig"]["mixer"]["buses"][0]["slot1"]["params"]["feedback"],
        0.36
    );
    assert_eq!(
        payload["runtimeConfig"]["mixer"]["master"]["slots"][0]["params"]["clip"],
        0.65
    );
}

#[test]
pub(crate) fn invalid_bus_and_global_fx_types_are_sanitized_on_load() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["mixer"]["buses"][0]["slot1"] =
        json!({ "type": "pitch_shift", "params": {} });
    payload["runtimeConfig"]["mixer"]["master"]["slots"][0] =
        json!({ "type": "delay", "params": {} });

    runner.apply_config_payload(payload).unwrap();

    assert_eq!(runner.fx_buses[0].slot1_type, "none");
    assert_eq!(runner.global_fx_slots[0], "none");
}

#[test]
pub(crate) fn worlds_layer_config_always_exposes_auto_name() {
    for behavior_id in ["life", "none", "brain"] {
        let mut runner = NativeRunner::new(NativeRunnerConfig {
            behavior_id: behavior_id.into(),
            ..NativeRunnerConfig::default()
        })
        .unwrap();

        let _ = runner
            .send(HostMessage::DeviceInput {
                input: json!({ "type": "encoder_press", "id": "main" }),
                request_snapshot: None,
            })
            .unwrap();
        let entered = runner
            .send(HostMessage::DeviceInput {
                input: json!({ "type": "encoder_press", "id": "main" }),
                request_snapshot: None,
            })
            .unwrap();

        let lines = snapshot_from(&entered)["display"]["lines"]
            .as_array()
            .unwrap()
            .clone();
        assert!(
            lines
                .iter()
                .any(|line| line.as_str().unwrap_or("").contains("Layer Label")),
            "{behavior_id} should show Layer Label"
        );
        assert!(
            lines
                .iter()
                .any(|line| line.as_str().unwrap_or("").contains("Auto Label")),
            "{behavior_id} should show Auto Label"
        );
    }
}

#[test]
pub(crate) fn behavior_change_updates_active_layer_auto_name_label() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    select_behavior(&mut runner, "keys");

    assert_eq!(runner.layer_behavior_ids[0], "keys");
    runner.menu.back();
    runner.menu.rebuild(runner.menu_config());
    let snapshot = runner.snapshot().unwrap();
    let lines = snapshot["display"]["lines"].as_array().unwrap();
    assert!(lines
        .iter()
        .any(|line| line.as_str().unwrap_or("") == "> L1: keys >"));
}
