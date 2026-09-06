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
        "Restart device to"
    );
}

#[test]
pub(crate) fn dsp_mode_replaces_output_buffer_on_capacity_platform() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        audio_optimization_capacity_available: true,
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    assert!(runner.menu.root.children.iter().any(|item| {
        item.label == "System"
            && item.children.iter().any(|group| {
                group.label == "Sound"
                    && group.children.iter().any(|item| {
                        item.label == "DSP Mode" && item.key.as_deref() == Some("sound.optimizeFor")
                    })
            })
    }));
    assert!(runner.menu.focus_item_key("sound.optimizeFor"));
    assert_eq!(
        runner.menu.value_for_key("sound.optimizeFor"),
        Some("latency".into())
    );
    let snapshot = runner.menu.snapshot();
    let selected_row = snapshot.selected_row.expect("DSP mode row");
    assert_eq!(
        snapshot.full_lines[selected_row].as_deref(),
        Some("> DSP Mode Inline / low latency")
    );
    assert!(!runner.menu.focus_item_key("sound.audioOutputBufferFrames"));
}

#[test]
pub(crate) fn dsp_mode_edits_open_reboot_confirmation_without_live_audio_command() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        audio_optimization_capacity_available: true,
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    assert!(runner.menu.focus_item_key("sound.optimizeFor"));
    runner.menu.state.editing = true;
    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();

    assert_eq!(
        runner.config_payload()["runtimeConfig"]["sound"]["optimizeFor"],
        "capacity"
    );
    let snapshot = snapshot_from(&messages);
    assert_eq!(snapshot["display"]["title"], "Confirm Audio");
    let lines = snapshot["display"]["lines"].as_array().unwrap();
    assert!(lines.iter().any(|line| line == "> Cancel"));
    assert!(lines.iter().any(|line| line == "  Save / Reboot"));
    assert_eq!(snapshot["display"]["toast"], "");
    assert!(!runner.pending.pending_audio_restart_prompt);
    assert!(!runner.outbox.has_audio_commands());
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
pub(crate) fn back_from_immediate_dsp_confirmation_cancels_without_reprompting() {
    let mut runner = dsp_mode_runner();

    let messages = change_dsp_mode(&mut runner);
    let snapshot = snapshot_from(&messages);

    assert_eq!(snapshot["display"]["title"], "Confirm Audio");
    let lines = snapshot["display"]["lines"].as_array().unwrap();
    assert!(lines.iter().any(|line| line == "> Cancel"));
    assert!(lines.iter().any(|line| line == "  Save / Reboot"));
    assert_eq!(snapshot["display"]["toast"], "");
    assert!(!runner.pending.pending_audio_restart_prompt);

    let messages = press_back(&mut runner);
    assert_eq!(snapshot_from(&messages)["display"]["toast"], "Cancelled");
    assert!(runner.display.confirm_dialog.is_none());
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["sound"]["optimizeFor"],
        "capacity"
    );
    assert!(!messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects } if !effects.is_empty()
    )));

    let messages = press_back(&mut runner);
    assert!(runner.display.confirm_dialog.is_none());
    assert!(!messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects } if !effects.is_empty()
    )));
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
    runner.display.oled_mode = NativeOledMode::Normal;
    runner.display.oled_splash_text.clear();
    runner.display.oled_splash_until = None;
    let _ = press_back(&mut runner);
    runner.display.confirm_dialog.as_mut().unwrap().cursor = 1;

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

#[test]
pub(crate) fn immediate_dsp_apply_reboot_confirmation_emits_one_apply_effect() {
    let mut runner = dsp_mode_runner();
    let _ = change_dsp_mode(&mut runner);
    runner.display.confirm_dialog.as_mut().unwrap().cursor = 1;

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    let apply_count = messages
        .iter()
        .filter_map(|message| match message {
            RunnerMessage::PlatformEffects { effects } => Some(
                effects
                    .iter()
                    .filter(|effect| {
                        matches!(
                            effect,
                            RuntimePlatformEffect::ApplyDeviceConfigReboot { .. }
                        )
                    })
                    .count(),
            ),
            _ => None,
        })
        .sum::<usize>();
    assert_eq!(apply_count, 1);
    assert!(!messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects
                .iter()
                .any(|effect| matches!(effect, RuntimePlatformEffect::Reboot))
    )));
    let payload = messages
        .iter()
        .find_map(|message| match message {
            RunnerMessage::PlatformEffects { effects } => effects.iter().find_map(|effect| {
                let RuntimePlatformEffect::ApplyDeviceConfigReboot { payload } = effect else {
                    return None;
                };
                Some(payload)
            }),
            _ => None,
        })
        .expect("audio apply reboot payload");
    assert_eq!(payload["runtimeConfig"]["sound"]["optimizeFor"], "capacity");
}

fn changed_output_buffer_runner() -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    assert!(runner.menu.focus_item_key("sound.audioOutputBufferFrames"));
    runner.menu.state.editing = true;
    runner.menu.turn(1);
    runner.apply_menu_state().unwrap();
    runner
}

fn dsp_mode_runner() -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        audio_optimization_capacity_available: true,
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    assert!(runner.menu.focus_item_key("sound.optimizeFor"));
    runner.menu.state.editing = true;
    runner
}

fn change_dsp_mode(runner: &mut NativeRunner) -> Vec<RunnerMessage> {
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap()
}

fn press_back(runner: &mut NativeRunner) -> Vec<RunnerMessage> {
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_a", "pressed": true }),
            request_snapshot: None,
        })
        .unwrap()
}
