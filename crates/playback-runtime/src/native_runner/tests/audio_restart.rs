use super::*;

fn changed_buffer_runner(auto_save_default: bool) -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.auto_save_default = auto_save_default;
    runner.menu.rebuild(runner.menu_config());
    assert!(runner.menu.focus_item_key("sound.audioOutputBufferFrames"));
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
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
}

fn dirty_bpm_hdmi_buffer_runner() -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.auto_save_default = false;

    assert!(runner.menu.focus_item_key("transport.bpm"));
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    let _ = commit_with_main(&mut runner);

    assert!(runner.menu.focus_item_key("audioOutputs.hdmi"));
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    let _ = commit_with_main(&mut runner);
    assert_eq!(
        runner.snapshot().unwrap()["display"]["title"],
        "Save Setting"
    );
    let _ = commit_with_back(&mut runner);

    assert!(runner.menu.focus_item_key("sound.audioOutputBufferFrames"));
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    runner
}

fn identified_save_result(
    runner: &mut NativeRunner,
    request_id: &str,
    ok: bool,
) -> Vec<RunnerMessage> {
    identified_save_result_at(runner, request_id, runner.config_revision, ok)
}

fn identified_save_result_at(
    runner: &mut NativeRunner,
    request_id: &str,
    revision: u64,
    ok: bool,
) -> Vec<RunnerMessage> {
    runner.register_default_write_request(request_id, Some(revision));
    runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::Identified {
                result: Box::new(RuntimeStoreResult::SaveDefaultResult { ok, is_auto: None }),
                request_id: request_id.into(),
                revision: Some(revision),
            },
        })
        .unwrap()
}

fn commit_with_main(runner: &mut NativeRunner) -> Vec<RunnerMessage> {
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap()
}

fn commit_with_back(runner: &mut NativeRunner) -> Vec<RunnerMessage> {
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_a", "pressed": true }),
            request_snapshot: None,
        })
        .unwrap()
}

fn turn_confirm(runner: &mut NativeRunner, delta: i32) {
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": delta, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
}

fn choose_save_setting(runner: &mut NativeRunner) -> Vec<RunnerMessage> {
    turn_confirm(runner, 1);
    commit_with_main(runner)
}

fn choose_save_everything(runner: &mut NativeRunner) -> Vec<RunnerMessage> {
    turn_confirm(runner, 2);
    commit_with_main(runner)
}

fn assert_dialog_lines_fit(snapshot: &Value) {
    for line in snapshot["display"]["lines"].as_array().unwrap() {
        assert!(line.as_str().unwrap().chars().count() <= 19, "{line}");
    }
}

#[test]
fn restart_sensitive_edit_opens_save_setting_dialog() {
    let runner = changed_buffer_runner(false);
    let snapshot = runner.snapshot().unwrap();

    assert_eq!(snapshot["display"]["title"], "/SYS/Sound");
    let mut runner = runner;
    let messages = commit_with_main(&mut runner);
    let snapshot = snapshot_from(&messages);
    assert_eq!(snapshot["display"]["title"], "Save Setting");
    assert_dialog_lines_fit(&snapshot);
    assert!(snapshot["display"]["lines"]
        .as_array()
        .unwrap()
        .iter()
        .any(|line| line == "> Cancel"));
    assert!(snapshot["display"]["lines"]
        .as_array()
        .unwrap()
        .iter()
        .any(|line| line == "  Save this setting"));
    assert!(snapshot["display"]["lines"]
        .as_array()
        .unwrap()
        .iter()
        .any(|line| line == "  Save everything"));
}

#[test]
fn repeated_restart_setting_turns_wait_for_main_commit() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    assert!(runner.menu.focus_item_key("sound.audioOutputBufferFrames"));
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();

    for _ in 0..2 {
        let messages = runner
            .send(HostMessage::DeviceInput {
                input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
                request_snapshot: None,
            })
            .unwrap();
        assert_eq!(snapshot_from(&messages)["display"]["title"], "/SYS/Sound");
        assert!(runner.display.confirm_dialog.is_none());
    }
    assert_eq!(runner.audio_output_buffer_frames, 1024);

    let messages = commit_with_main(&mut runner);
    assert_eq!(snapshot_from(&messages)["display"]["title"], "Save Setting");
    assert_dialog_lines_fit(&snapshot_from(&messages));
}

#[test]
fn restart_sensitive_back_commit_opens_save_setting_dialog() {
    let mut runner = changed_buffer_runner(false);
    let messages = commit_with_back(&mut runner);

    assert_eq!(snapshot_from(&messages)["display"]["title"], "Save Setting");
    assert_dialog_lines_fit(&snapshot_from(&messages));
}

#[test]
fn save_this_setting_writes_only_changed_leaf_and_preserves_unrelated_dirty_state() {
    let mut runner = dirty_bpm_hdmi_buffer_runner();
    let baseline_bpm = runner.restart_settings.persisted_default["runtimeConfig"]["bpm"].clone();

    let _ = commit_with_main(&mut runner);
    let messages = choose_save_setting(&mut runner);
    let (payload, mode) = messages
        .iter()
        .find_map(|message| match message {
            RunnerMessage::PlatformEffects { effects } => effects.iter().find_map(|effect| {
                let RuntimePlatformEffect::StoreSaveDefault { payload, mode } = effect else {
                    return None;
                };
                Some((payload, mode))
            }),
            _ => None,
        })
        .expect("setting-only save effect");

    assert_eq!(mode.as_deref(), Some("restart-setting"));
    assert_eq!(
        payload["runtimeConfig"]["sound"]["audioOutputBufferFrames"],
        512
    );
    assert_eq!(payload["runtimeConfig"]["bpm"], baseline_bpm);
    assert_eq!(payload["runtimeConfig"]["audioOutputs"]["hdmi"], false);
    assert_eq!(runner.transport.bpm, 121.0);
    assert!(runner.audio_outputs.hdmi());
    assert!(runner.config_dirty);
    assert_eq!(snapshot_from(&messages)["display"]["title"], "Saving...");

    let _ = identified_save_result(&mut runner, "save-setting", true);
    assert_eq!(runner.snapshot().unwrap()["display"]["title"], "Restart?");
    assert_eq!(runner.audio_output_buffer_frames, 512);
    assert_eq!(runner.transport.bpm, 121.0);
    assert!(runner.audio_outputs.hdmi());
    assert!(runner.config_dirty);
    let messages = commit_with_back(&mut runner);
    assert!(!messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects.iter().any(|effect| matches!(effect, RuntimePlatformEffect::Reboot))
    )));
}

#[test]
fn save_everything_writes_full_current_payload() {
    let mut runner = changed_buffer_runner(false);
    let _ = commit_with_main(&mut runner);
    let messages = choose_save_everything(&mut runner);
    let payload = messages
        .iter()
        .find_map(|message| match message {
            RunnerMessage::PlatformEffects { effects } => effects.iter().find_map(|effect| {
                let RuntimePlatformEffect::StoreSaveDefault { payload, mode } = effect else {
                    return None;
                };
                (mode.as_deref() == Some("restart-everything")).then_some(payload)
            }),
            _ => None,
        })
        .expect("full save effect");

    assert_eq!(payload, &runner.config_payload());
    let _ = identified_save_result(&mut runner, "save-everything", true);
    assert!(!runner.config_dirty);
}

#[test]
fn auto_save_on_writes_full_payload_once_without_save_choice() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.auto_save_default = true;
    runner.menu.rebuild(runner.menu_config());
    assert!(runner.menu.focus_item_key("sound.audioOutputBufferFrames"));
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
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
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    let messages = commit_with_main(&mut runner);
    let effects: Vec<&RuntimePlatformEffect> = messages
        .iter()
        .filter_map(|message| match message {
            RunnerMessage::PlatformEffects { effects } => Some(effects),
            _ => None,
        })
        .flatten()
        .collect();

    assert_eq!(
        effects
            .iter()
            .filter(|effect| matches!(effect, RuntimePlatformEffect::StoreSaveDefault { .. }))
            .count(),
        1
    );
    let payload = effects
        .iter()
        .find_map(|effect| match effect {
            RuntimePlatformEffect::StoreSaveDefault { payload, .. } => Some(payload),
            _ => None,
        })
        .unwrap();
    assert_eq!(payload, &runner.config_payload());
    assert_eq!(snapshot_from(&messages)["display"]["title"], "Saving...");
    assert!(!messages.iter().any(|message| {
        matches!(message, RunnerMessage::Snapshot { snapshot } if snapshot["display"]["title"] == "Save Setting")
    }));
}

#[test]
fn stale_save_ack_does_not_advance_restart_flow() {
    let mut runner = changed_buffer_runner(false);
    let _ = commit_with_main(&mut runner);
    runner.display.confirm_dialog.as_mut().unwrap().cursor = 2;

    let revision = runner.config_revision;
    let _ = choose_save_everything(&mut runner);
    runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::Identified {
                result: Box::new(RuntimeStoreResult::SaveDefaultResult {
                    ok: true,
                    is_auto: None,
                }),
                request_id: "stale".into(),
                revision: Some(revision.saturating_sub(1)),
            },
        })
        .unwrap();
    assert_eq!(runner.snapshot().unwrap()["display"]["title"], "Saving...");
}

#[test]
fn failed_save_does_not_reboot_or_advance_baseline() {
    let mut runner = changed_buffer_runner(false);
    let baseline = runner.restart_settings.persisted_default.clone();
    let _ = commit_with_main(&mut runner);
    let _ = choose_save_everything(&mut runner);

    let messages = identified_save_result(&mut runner, "save-1", false);
    assert_eq!(runner.restart_settings.persisted_default, baseline);
    assert_eq!(snapshot_from(&messages)["display"]["toast"], "Save failed");
    assert!(!messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects.iter().any(|effect| matches!(effect, RuntimePlatformEffect::Reboot))
    )));
}

#[test]
fn successful_save_offers_reboot_and_reboot_uses_recovery_lifecycle() {
    let mut runner = changed_buffer_runner(false);
    let _ = commit_with_main(&mut runner);
    let _ = choose_save_everything(&mut runner);
    let _ = identified_save_result(&mut runner, "save-1", true);
    assert_eq!(runner.snapshot().unwrap()["display"]["title"], "Restart?");
    assert_dialog_lines_fit(&runner.snapshot().unwrap());

    turn_confirm(&mut runner, 1);
    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    let effects: Vec<&RuntimePlatformEffect> = messages
        .iter()
        .filter_map(|message| match message {
            RunnerMessage::PlatformEffects { effects } => Some(effects),
            _ => None,
        })
        .flatten()
        .collect();
    assert_eq!(
        effects
            .iter()
            .filter(|effect| matches!(effect, RuntimePlatformEffect::StoreSaveRecovery { .. }))
            .count(),
        1
    );
    assert_eq!(
        effects
            .iter()
            .filter(|effect| matches!(effect, RuntimePlatformEffect::Reboot))
            .count(),
        1
    );
    assert!(!effects.iter().any(|effect| matches!(
        effect,
        RuntimePlatformEffect::ApplyDeviceConfigReboot { .. }
    )));
}

#[test]
fn continue_after_successful_save_does_not_reboot() {
    let mut runner = changed_buffer_runner(false);
    let _ = commit_with_main(&mut runner);
    let _ = choose_save_everything(&mut runner);
    let _ = identified_save_result(&mut runner, "save-1", true);

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    assert!(!messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects.iter().any(|effect| matches!(effect, RuntimePlatformEffect::Reboot))
    )));
}
