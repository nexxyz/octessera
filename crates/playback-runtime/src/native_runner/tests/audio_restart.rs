use super::*;
use crate::RuntimePlatformRequest;

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
    runner
}

fn dirty_bpm_hdmi_buffer_runner() -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        jack_audio_required: true,
        ..NativeRunnerConfig::default()
    })
    .unwrap();
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
            input: json!({ "type": "button_fn", "pressed": true }),
            request_snapshot: None,
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_fn", "pressed": false }),
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
        "Apply System"
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
            input: json!({ "type": "button_fn", "pressed": true }),
            request_snapshot: None,
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_fn", "pressed": false }),
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
    _revision: u64,
    ok: bool,
) -> Vec<RunnerMessage> {
    if runner
        .restart_settings
        .pending_write_scope()
        .is_some_and(|scope| scope.is_restart())
    {
        let payload = runner
            .restart_settings
            .pending_write_payload()
            .expect("System Apply payload");
        runner.register_platform_request(&RuntimePlatformRequest::new(
            RuntimePlatformEffect::StoreSaveSystem {
                payload: payload.as_ref().clone(),
            },
            request_id.into(),
            None,
        ));
        runner
            .send(HostMessage::RuntimeResult {
                result: RuntimeStoreResult::SaveSystemResult { ok }
                    .with_identity(request_id.into(), None),
            })
            .unwrap()
    } else {
        runner.register_default_write_request(request_id, None);
        runner
            .send(HostMessage::RuntimeResult {
                result: RuntimeStoreResult::SaveDefaultResult { ok, is_auto: None }
                    .with_identity(request_id.into(), None),
            })
            .unwrap()
    }
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
        assert!(line.as_str().unwrap().chars().count() <= 20, "{line}");
    }
}

#[test]
fn restart_sensitive_edit_opens_save_setting_dialog() {
    let runner = changed_buffer_runner(false);
    let snapshot = runner.snapshot().unwrap();

    assert_eq!(snapshot["display"]["title"], "/SYS/Audio/Engine");
    let mut runner = runner;
    let messages = commit_with_main(&mut runner);
    let snapshot = snapshot_from(&messages);
    assert_eq!(snapshot["display"]["title"], "Apply System");
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
        .any(|line| line == "  Save this one"));
    assert!(snapshot["display"]["lines"]
        .as_array()
        .unwrap()
        .iter()
        .any(|line| line == "  Save System"));
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

    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_fn", "pressed": true }),
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
        assert_eq!(
            snapshot_from(&messages)["display"]["title"],
            "/SYS/Audio/Engine"
        );
        assert!(runner.display.confirm_dialog.is_none());
    }
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_fn", "pressed": false }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(runner.audio_output_buffer_frames, 1024);

    let messages = commit_with_main(&mut runner);
    assert_eq!(snapshot_from(&messages)["display"]["title"], "Apply System");
    assert_dialog_lines_fit(&snapshot_from(&messages));
}

#[test]
fn restart_sensitive_back_commit_opens_save_setting_dialog() {
    let mut runner = changed_buffer_runner(false);
    let messages = commit_with_back(&mut runner);

    assert_eq!(snapshot_from(&messages)["display"]["title"], "Apply System");
    assert_dialog_lines_fit(&snapshot_from(&messages));
}

#[test]
fn apply_system_setting_saves_only_system_and_preserves_unrelated_patch_dirty_state() {
    let mut runner = dirty_bpm_hdmi_buffer_runner();
    let patch_dirty_revision = runner.dirty_revision;

    let _ = commit_with_main(&mut runner);
    let messages = choose_save_setting(&mut runner);
    let payload = messages
        .iter()
        .find_map(|message| match message {
            RunnerMessage::PlatformEffects { effects } => effects.iter().find_map(|effect| {
                let RuntimePlatformEffect::StoreSaveSystem { payload } = effect else {
                    return None;
                };
                Some(payload)
            }),
            _ => None,
        })
        .expect("System Apply save effect");

    assert_eq!(payload["kind"], "octessera.system");
    assert_eq!(payload["schemaVersion"], 1);
    assert_eq!(
        payload["runtimeConfig"]["sound"]["audioOutputBufferFrames"],
        512
    );
    assert_eq!(payload["runtimeConfig"]["audioOutputs"]["hdmi"], false);
    assert_eq!(runner.transport.bpm, 121.0);
    assert!(runner.audio_outputs.hdmi());
    assert!(runner.config_dirty);
    assert_eq!(runner.dirty_revision, patch_dirty_revision);
    assert_eq!(snapshot_from(&messages)["display"]["title"], "Saving...");

    let _ = identified_save_result(&mut runner, "save-setting", true);
    assert_eq!(runner.snapshot().unwrap()["display"]["title"], "Restart?");
    assert_eq!(runner.audio_output_buffer_frames, 512);
    assert_eq!(runner.transport.bpm, 121.0);
    assert!(runner.audio_outputs.hdmi());
    assert!(runner.config_dirty);
    assert_eq!(runner.dirty_revision, patch_dirty_revision);
    assert!(runner.pending.system_persistence.dirty_revision.is_some());
    let messages = commit_with_back(&mut runner);
    assert!(!messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects.iter().any(|effect| matches!(effect, RuntimePlatformEffect::Reboot))
    )));
}

#[test]
fn apply_save_system_settings_writes_a_system_document_only() {
    let mut runner = changed_buffer_runner(false);
    let _ = commit_with_main(&mut runner);
    let messages = choose_save_everything(&mut runner);
    let payload = messages
        .iter()
        .find_map(|message| match message {
            RunnerMessage::PlatformEffects { effects } => effects.iter().find_map(|effect| {
                let RuntimePlatformEffect::StoreSaveSystem { payload } = effect else {
                    return None;
                };
                Some(payload)
            }),
            _ => None,
        })
        .expect("System Apply save effect");

    assert_eq!(
        payload,
        &super::super::system_persistence::SystemPersistenceState::system_document(&runner)
            .unwrap()
    );
    let _ = identified_save_result(&mut runner, "save-everything", true);
    assert!(!runner.config_dirty);
}

#[test]
fn patch_auto_save_policy_does_not_automatically_apply_system_restart_settings() {
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

    assert!(!effects.iter().any(|effect| matches!(
        effect,
        RuntimePlatformEffect::StoreSaveDefault { .. }
            | RuntimePlatformEffect::StoreSaveSystem { .. }
    )));
    assert_eq!(snapshot_from(&messages)["display"]["title"], "Apply System");
    assert!(!runner.config_dirty);
    assert!(runner.pending.system_persistence.dirty_revision.is_some());
}

#[path = "audio_restart_outcome_tests.rs"]
mod outcome_tests;
