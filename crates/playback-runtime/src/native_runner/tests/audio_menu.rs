use super::*;

mod fx;

#[test]
pub(crate) fn usb_menu_edits_payload_with_save_reboot_toast() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    assert!(runner.menu.focus_item_key("usb.audioOut"));
    runner.menu.state.editing = true;

    runner.menu.turn(1);
    runner.apply_menu_state().unwrap();

    assert_eq!(
        runner.config_payload()["runtimeConfig"]["usb"]["audioOut"],
        "usb"
    );
    assert_eq!(
        runner.snapshot().unwrap()["display"]["toast"],
        "USB: Save & Reboot"
    );
}

#[test]
pub(crate) fn usb_apply_reboot_is_confirmed_and_emits_payload() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.usb_audio_out = "both".into();
    runner.usb_midi_out_enabled = true;
    assert!(runner.menu.focus_item_key("usb.applyReboot"));

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    let snapshot = snapshot_from(&messages);
    assert_eq!(snapshot["display"]["title"], "Confirm USB");
    let lines = snapshot["display"]["lines"].as_array().unwrap();
    assert!(lines.iter().any(|line| line == "> Cancel"));
    assert!(lines.iter().any(|line| line == "  Save & Reboot"));

    runner.confirm_dialog.as_mut().unwrap().cursor = 1;
    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();

    let effect = messages.iter().find_map(|message| match message {
        RunnerMessage::PlatformEffects { effects } => effects.first(),
        _ => None,
    });
    assert!(
        matches!(effect, Some(RuntimePlatformEffect::UsbApplyReboot { payload }) if payload["runtimeConfig"]["usb"] == json!({ "audioOut": "both", "midiOutEnabled": true }))
    );
}

#[test]
pub(crate) fn usb_apply_reboot_cancel_keeps_menu_without_effect() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    assert!(runner.menu.focus_item_key("usb.applyReboot"));
    let before_path = runner.menu.current_focus_path();

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();

    assert_eq!(runner.menu.current_focus_path(), before_path);
    assert!(!messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects.iter().any(|effect| matches!(effect, RuntimePlatformEffect::UsbApplyReboot { .. }))
    )));
}

#[test]
pub(crate) fn usb_sd_transfer_actions_are_confirmed_and_emit_effects() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    assert!(runner.menu.focus_item_key("usb.sdTransferStart"));

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    let snapshot = snapshot_from(&messages);
    assert_eq!(snapshot["display"]["title"], "Confirm SD Transfer");
    assert!(snapshot["display"]["lines"]
        .as_array()
        .unwrap()
        .iter()
        .any(|line| line.as_str().unwrap_or_default().contains("disconnect")));

    runner.confirm_dialog.as_mut().unwrap().cursor = 1;
    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects == &vec![RuntimePlatformEffect::UsbSdTransferStart]
    )));

    assert!(runner.menu.focus_item_key("usb.sdTransferStop"));
    assert_eq!(
        runner.platform_effect_for_action("usb.sdTransferStop"),
        Some(RuntimePlatformEffect::UsbSdTransferStop)
    );
}

#[test]
pub(crate) fn usb_sd_transfer_start_stops_playback_and_opens_blocking_modal() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.transport = RuntimeTransportState::Playing;
    assert!(runner.menu.focus_item_key("usb.sdTransferStart"));

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    let messages = confirm_current_dialog(&mut runner);

    assert_eq!(runner.transport, RuntimeTransportState::Stopped);
    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects == &vec![RuntimePlatformEffect::UsbSdTransferStart]
    )));
    let snapshot = snapshot_from(&messages);
    assert_eq!(snapshot["display"]["title"], "SD Transfer");
    assert!(snapshot["display"]["lines"]
        .as_array()
        .unwrap()
        .iter()
        .any(|line| line == "> Stop Transfer"));

    let blocked_messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 0, "y": 0 }),
            request_snapshot: None,
        })
        .unwrap();
    assert!(!blocked_messages.iter().any(|message| matches!(
        message,
        RunnerMessage::MusicalEvents { .. } | RunnerMessage::PlatformEffects { .. }
    )));
}

#[test]
pub(crate) fn usb_sd_transfer_modal_closes_by_back_or_main_without_resuming() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.transport = RuntimeTransportState::Playing;
    assert!(runner.menu.focus_item_key("usb.sdTransferStart"));

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    let _ = confirm_current_dialog(&mut runner);
    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_a", "pressed": true }),
            request_snapshot: None,
        })
        .unwrap();

    assert_eq!(runner.transport, RuntimeTransportState::Stopped);
    assert!(runner.usb_sd_transfer_modal.is_none());
    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects == &vec![RuntimePlatformEffect::UsbSdTransferStop]
    )));

    runner.open_usb_sd_transfer_modal();
    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    assert!(runner.usb_sd_transfer_modal.is_none());
    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects == &vec![RuntimePlatformEffect::UsbSdTransferStop]
    )));
}

#[test]
pub(crate) fn recording_max_time_edits_config_payload() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.recording_max_minutes = 14;

    assert_eq!(
        runner.config_payload()["runtimeConfig"]["recording"]["maxMinutes"],
        14
    );
}

#[test]
pub(crate) fn recording_actions_emit_platform_effects() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.recording_max_minutes = 7;
    assert!(runner.menu.focus_item_key("recording.startAudio"));

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    let effect = messages.iter().find_map(|message| match message {
        RunnerMessage::PlatformEffects { effects } => effects.first(),
        _ => None,
    });
    assert_eq!(
        effect,
        Some(&RuntimePlatformEffect::RecordingStartAudio { max_minutes: 7 })
    );

    assert!(runner.menu.focus_item_key("recording.stop"));
    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    let effect = messages.iter().find_map(|message| match message {
        RunnerMessage::PlatformEffects { effects } => effects.first(),
        _ => None,
    });
    assert_eq!(effect, Some(&RuntimePlatformEffect::RecordingStop));
}

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
