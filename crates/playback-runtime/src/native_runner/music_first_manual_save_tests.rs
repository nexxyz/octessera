use super::*;
use crate::tests::support::FakeHost;

fn dispatch(
    runner: &mut NativeRunner,
    runtime: &mut crate::PlaybackRuntime,
    host: &mut FakeHost,
    input: Value,
) -> crate::RuntimeIngest {
    runtime
        .dispatch_host_message_music_first(
            HostMessage::DeviceInput {
                input,
                request_snapshot: Some(false),
            },
            runner,
            host,
        )
        .unwrap()
}

fn press(
    runner: &mut NativeRunner,
    runtime: &mut crate::PlaybackRuntime,
    host: &mut FakeHost,
) -> crate::RuntimeIngest {
    dispatch(
        runner,
        runtime,
        host,
        json!({ "type": "encoder_press", "id": "main" }),
    )
}

fn turn(
    runner: &mut NativeRunner,
    runtime: &mut crate::PlaybackRuntime,
    host: &mut FakeHost,
    delta: i32,
) -> crate::RuntimeIngest {
    dispatch(
        runner,
        runtime,
        host,
        json!({ "type": "encoder_turn", "id": "main", "delta": delta }),
    )
}

fn playing_keys_runner(runtime: &mut crate::PlaybackRuntime, host: &mut FakeHost) -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "keys".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.menu.rebuild(runner.menu_config());
    for input in [
        json!({ "type": "button_s", "pressed": true }),
        json!({ "type": "button_s", "pressed": false }),
        json!({ "type": "grid_press", "x": 2, "y": 3 }),
        json!({ "type": "grid_release", "x": 2, "y": 3 }),
    ] {
        let _ = dispatch(&mut runner, runtime, host, input);
    }
    assert_eq!(runner.transport.transport, RuntimeTransportState::Playing);
    assert!(!host.musical_events.is_empty());
    runner
}

fn confirm_action(
    runner: &mut NativeRunner,
    runtime: &mut crate::PlaybackRuntime,
    host: &mut FakeHost,
    key: &str,
) -> crate::RuntimeIngest {
    assert!(runner.menu.focus_item_key(key));
    let opened = press(runner, runtime, host);
    assert!(!opened
        .messages
        .iter()
        .any(|message| matches!(message, RunnerMessage::Snapshot { .. })));
    let _ = turn(runner, runtime, host, 1);
    press(runner, runtime, host)
}

#[test]
fn default_save_confirmation_cancels_or_emits_only_metadata_after_confirm() {
    let mut runtime = crate::PlaybackRuntime::new(crate::RuntimeConfig::default());
    let mut host = FakeHost::default();
    let mut runner = playing_keys_runner(&mut runtime, &mut host);
    let serialization_calls = runner.behavior_state_serialization_calls.get();
    runner.menu.rebuild(runner.menu_config());
    assert!(runner.menu.focus_item_key("default.save"));
    let _ = press(&mut runner, &mut runtime, &mut host);
    let cancelled = press(&mut runner, &mut runtime, &mut host);
    assert!(cancelled
        .messages
        .iter()
        .all(|message| !matches!(message, RunnerMessage::PlatformEffects { .. })));
    assert!(runner.take_manual_save_request().is_none());
    assert_eq!(
        runner.behavior_state_serialization_calls.get(),
        serialization_calls
    );

    let confirmed = confirm_action(&mut runner, &mut runtime, &mut host, "default.save");
    assert!(confirmed
        .messages
        .iter()
        .any(|message| matches!(message, RunnerMessage::RuntimeStatus { .. })));
    assert!(confirmed
        .messages
        .iter()
        .all(|message| !matches!(message, RunnerMessage::Snapshot { .. })));
    assert!(host.effects.is_empty());
    assert_eq!(
        runner.take_manual_save_request(),
        Some(NativeManualSaveRequest::Default)
    );
    assert!(runner.take_manual_save_request().is_none());
    assert_eq!(
        runner.behavior_state_serialization_calls.get(),
        serialization_calls
    );
}

#[test]
fn default_save_with_an_accepted_write_keeps_the_save_in_progress_feedback() {
    let mut runtime = crate::PlaybackRuntime::new(crate::RuntimeConfig::default());
    let mut host = FakeHost::default();
    let mut runner = playing_keys_runner(&mut runtime, &mut host);
    runner.mark_config_dirty();
    let revision = runner.config_revision;
    let payload = std::sync::Arc::new(
        runner
            .capture_config_snapshot()
            .into_portable_patch_payload()
            .unwrap(),
    );
    assert!(runner.register_native_default_write("accepted-default", revision, true));
    assert!(runner.attach_native_default_write_payload("accepted-default", revision, payload,));
    runner.menu.rebuild(runner.menu_config());
    let _ = confirm_action(&mut runner, &mut runtime, &mut host, "default.save");

    assert_eq!(
        runner
            .display
            .toast
            .as_ref()
            .map(|toast| toast.message.as_str()),
        Some("Save in progress")
    );
    assert!(runner.take_manual_save_request().is_none());
}

#[test]
fn preset_save_actions_freeze_cleaned_target_overwrite_mode_and_rename_source() {
    let mut runtime = crate::PlaybackRuntime::new(crate::RuntimeConfig::default());
    let mut host = FakeHost::default();
    let mut runner = playing_keys_runner(&mut runtime, &mut host);
    let serialization_calls = runner.behavior_state_serialization_calls.get();
    runner.preset_names = vec!["Source".into(), "Other".into()];
    runner.menu.rebuild(runner.menu_config());
    assert!(runner.menu.focus_item_key("preset.renamePick.Source"));
    let _ = press(&mut runner, &mut runtime, &mut host);

    runner.preset_draft_name = "  Morning set  ".into();
    runner.menu.rebuild(runner.menu_config());
    let save_as = confirm_action(&mut runner, &mut runtime, &mut host, "preset.saveAs.save");
    assert!(save_as
        .messages
        .iter()
        .all(|message| !matches!(message, RunnerMessage::Snapshot { .. })));
    assert_eq!(
        runner.take_manual_save_request(),
        Some(NativeManualSaveRequest::Preset {
            name: "Morning set".into(),
            mode: None,
            rename_from: None,
        })
    );

    runner.current_preset_name = Some("Alpha".into());
    runner.menu.rebuild(runner.menu_config());
    let save_current = confirm_action(&mut runner, &mut runtime, &mut host, "preset.saveCurrent");
    assert!(save_current
        .messages
        .iter()
        .all(|message| !matches!(message, RunnerMessage::Snapshot { .. })));
    assert_eq!(
        runner.take_manual_save_request(),
        Some(NativeManualSaveRequest::Preset {
            name: "Alpha".into(),
            mode: Some("overwrite".into()),
            rename_from: None,
        })
    );

    runner.menu.rebuild(runner.menu_config());
    runner.preset_draft_name = "  Target  ".into();
    runner.menu.rebuild(runner.menu_config());
    let rename = confirm_action(&mut runner, &mut runtime, &mut host, "preset.rename.apply");
    assert!(rename
        .messages
        .iter()
        .all(|message| !matches!(message, RunnerMessage::Snapshot { .. })));
    assert_eq!(
        runner.take_manual_save_request(),
        Some(NativeManualSaveRequest::Preset {
            name: "Target".into(),
            mode: None,
            rename_from: Some("Source".into()),
        })
    );
    assert!(host.effects.is_empty());
    assert_eq!(
        runner.behavior_state_serialization_calls.get(),
        serialization_calls
    );
}
