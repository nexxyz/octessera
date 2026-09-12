use super::*;
use crate::{
    HostAdapter, PlaybackRuntime, RuntimeAdapterError, RuntimeAudioCommand, RuntimePlatformRequest,
};
use platform_core::MusicalEvent;

fn input(runner: &mut NativeRunner, value: Value) -> Vec<RunnerMessage> {
    runner
        .send(HostMessage::DeviceInput {
            input: value,
            request_snapshot: None,
        })
        .unwrap()
}

pub(super) fn press(runner: &mut NativeRunner) -> Vec<RunnerMessage> {
    input(runner, json!({ "type": "encoder_press", "id": "main" }))
}

pub(super) fn turn(runner: &mut NativeRunner, delta: i32) -> Vec<RunnerMessage> {
    input(
        runner,
        json!({ "type": "encoder_turn", "delta": delta, "id": "main" }),
    )
}

fn back(runner: &mut NativeRunner) -> Vec<RunnerMessage> {
    input(runner, json!({ "type": "button_a", "pressed": true }))
}

pub(super) fn enter_edit(runner: &mut NativeRunner, key: &str) {
    assert!(runner.menu.focus_item_key(key));
    press(runner);
}

fn changed_buffer_runner(auto_save: bool) -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.auto_save_default = auto_save;
    runner.menu.rebuild(runner.menu_config());
    enter_edit(&mut runner, "sound.audioOutputBufferFrames");
    turn(&mut runner, 1);
    runner
}

fn choose_save_everything(runner: &mut NativeRunner) -> Vec<RunnerMessage> {
    turn(runner, 2);
    press(runner)
}

fn send_identified_save_result(
    runner: &mut NativeRunner,
    request_id: &str,
    revision: u64,
    ok: bool,
) -> Vec<RunnerMessage> {
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

fn assert_dialog_lines_fit(snapshot: &Value) {
    for line in snapshot["display"]["lines"].as_array().unwrap() {
        assert!(line.as_str().unwrap().chars().count() <= 19, "{line}");
    }
}

fn assert_menu_lines_fit(runner: &NativeRunner) {
    for line in runner.menu.snapshot().full_lines.iter().flatten() {
        assert!(line.chars().count() <= 19, "{line}");
    }
}

struct ImmediateFailureHost;

impl HostAdapter for ImmediateFailureHost {
    fn handle_musical_event(&mut self, _event: &MusicalEvent) -> Result<(), RuntimeAdapterError> {
        Ok(())
    }

    fn handle_platform_effect(
        &mut self,
        request: &RuntimePlatformRequest,
    ) -> Result<Vec<HostMessage>, RuntimeAdapterError> {
        if matches!(
            request.effect,
            RuntimePlatformEffect::StoreSaveDefault { .. }
        ) {
            Err(RuntimeAdapterError::from("default save failed immediately"))
        } else {
            Ok(Vec::new())
        }
    }

    fn handle_audio_command(
        &mut self,
        _command: &RuntimeAudioCommand,
    ) -> Result<(), RuntimeAdapterError> {
        Ok(())
    }

    fn handle_midi_message(&mut self, _bytes: &[u8]) -> Result<(), RuntimeAdapterError> {
        Ok(())
    }

    fn silence_internal_audio(&mut self) -> Result<(), RuntimeAdapterError> {
        Ok(())
    }

    fn panic_external_midi(&mut self) -> Result<(), RuntimeAdapterError> {
        Ok(())
    }
}

fn start_ordinary_default_write(runner: &mut NativeRunner, request_id: &str) -> u64 {
    assert!(runner.menu.focus_item_key("default.save"));
    press(runner);
    turn(runner, 1);
    press(runner);
    let revision = runner.restart_settings.pending_write_revision().unwrap();
    runner.register_default_write_request(request_id, Some(revision));
    revision
}

#[test]
fn restart_save_waits_for_pending_ordinary_write_without_overwriting_it() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let ordinary_revision = start_ordinary_default_write(&mut runner, "ordinary-1");
    enter_edit(&mut runner, "sound.audioOutputBufferFrames");
    turn(&mut runner, 1);
    let _ = press(&mut runner);

    let snapshot = runner.snapshot().unwrap();
    assert!(!snapshot["display"]["lines"]
        .as_array()
        .unwrap()
        .iter()
        .any(|line| line == "  Save this setting"));
    assert!(runner.restart_settings.has_pending_write());
    let _ = choose_save_everything(&mut runner);
    assert_eq!(
        runner.restart_settings.pending_write_revision(),
        Some(ordinary_revision)
    );

    let _ = send_identified_save_result(&mut runner, "ordinary-1", ordinary_revision, true);
    assert!(runner.snapshot().unwrap()["display"]["lines"]
        .as_array()
        .unwrap()
        .iter()
        .any(|line| line == "  Save this setting"));
    let messages = choose_save_everything(&mut runner);
    assert_eq!(messages
        .iter()
        .filter(|message| matches!(message, RunnerMessage::PlatformEffects { effects } if effects.iter().any(|effect| matches!(effect, RuntimePlatformEffect::StoreSaveDefault { mode: Some(mode), .. } if mode == "restart-everything"))))
        .count(), 1);
}

#[test]
fn restart_cancel_does_not_cancel_pending_ordinary_write() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let revision = start_ordinary_default_write(&mut runner, "ordinary-1");
    enter_edit(&mut runner, "sound.audioOutputBufferFrames");
    turn(&mut runner, 1);
    let _ = press(&mut runner);
    back(&mut runner);

    assert!(runner.display.confirm_dialog.is_none());
    assert!(runner.restart_settings.has_pending_write());
    let _ = send_identified_save_result(&mut runner, "ordinary-1", revision, true);
    assert!(
        !runner.restart_settings.has_pending_write(),
        "{:#?}",
        runner.restart_settings
    );
    assert!(runner.display.confirm_dialog.is_none());
}

#[test]
fn pending_deferred_write_survives_restart_edit_and_starts_one_restart_save() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.auto_save_default = true;
    enter_edit(&mut runner, "transport.bpm");
    turn(&mut runner, 1);
    runner.make_deferred_menu_apply_due_for_test();
    let deferred = runner.flush_deferred_menu_apply().unwrap();
    assert!(deferred.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects.iter().any(|effect| matches!(effect, RuntimePlatformEffect::StoreSaveDefault { mode: Some(mode), .. } if mode == "deferred"))
    )));
    let revision = runner.restart_settings.pending_write_revision().unwrap();
    runner.register_default_write_request("deferred-1", Some(revision));

    enter_edit(&mut runner, "sound.audioOutputBufferFrames");
    turn(&mut runner, 1);
    let _ = press(&mut runner);
    let snapshot = runner.snapshot().unwrap();
    assert_eq!(snapshot["display"]["title"], "/SYS/Audio/Engine");
    assert!(runner.display.confirm_dialog.is_none());
    runner.make_deferred_menu_apply_due_for_test();
    let deferred = runner.flush_deferred_menu_apply().unwrap();
    assert!(!deferred.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects.iter().any(|effect| matches!(effect, RuntimePlatformEffect::StoreSaveDefault { .. }))
    )));
    back(&mut runner);
    assert!(runner.restart_settings.has_pending_write());

    let messages = send_identified_save_result(&mut runner, "deferred-1", revision, true);
    let follow_up_revision = runner.restart_settings.pending_write_revision().unwrap();
    assert_eq!(
        messages
            .iter()
            .filter(|message| matches!(message, RunnerMessage::PlatformEffects { effects } if effects.iter().any(|effect| matches!(effect, RuntimePlatformEffect::StoreSaveDefault { mode: Some(mode), .. } if mode == "deferred"))))
            .count(),
        0
    );
    assert_eq!(
        messages
            .iter()
            .filter(|message| matches!(message, RunnerMessage::PlatformEffects { effects } if effects.iter().any(|effect| matches!(effect, RuntimePlatformEffect::StoreSaveDefault { mode: Some(mode), .. } if mode == "restart-everything"))))
            .count(),
        1
    );
    runner.register_default_write_request("deferred-2", Some(follow_up_revision));
    let messages = send_identified_save_result(&mut runner, "deferred-2", follow_up_revision, true);
    assert!(!runner.restart_settings.has_pending_write());
    assert_eq!(runner.snapshot().unwrap()["display"]["title"], "Restart?");
    assert!(!messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects.iter().any(|effect| matches!(effect, RuntimePlatformEffect::StoreSaveDefault { .. }))
    )));
}

#[test]
fn restart_results_require_the_unique_identified_transaction() {
    let mut runner = changed_buffer_runner(true);
    let messages = press(&mut runner);
    let revision = runner.restart_settings.pending_write_revision().unwrap();
    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects.iter().any(|effect| matches!(effect, RuntimePlatformEffect::StoreSaveDefault { mode: Some(mode), .. } if mode == "restart-everything"))
    )));
    runner.register_default_write_request("restart-1", Some(revision));

    runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::SaveDefaultResult {
                ok: true,
                is_auto: Some(true),
            },
        })
        .unwrap();
    assert!(runner.restart_settings.has_pending_write());
    assert_eq!(runner.snapshot().unwrap()["display"]["title"], "Saving...");

    send_identified_save_result(&mut runner, "wrong-id", revision, false);
    assert!(runner.restart_settings.has_pending_write());
    assert_eq!(runner.snapshot().unwrap()["display"]["title"], "Saving...");
    let stale_revision = revision.saturating_sub(1);
    runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::Identified {
                result: Box::new(RuntimeStoreResult::SaveDefaultResult {
                    ok: false,
                    is_auto: Some(true),
                }),
                request_id: "restart-1".into(),
                revision: Some(stale_revision),
            },
        })
        .unwrap();
    assert!(runner.restart_settings.has_pending_write());
    runner.make_deferred_menu_apply_due_for_test();
    let messages = runner.flush_deferred_menu_apply().unwrap();
    assert!(!messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects.iter().any(|effect| matches!(effect, RuntimePlatformEffect::StoreSaveDefault { .. }))
    )));

    let _ = send_identified_save_result(&mut runner, "restart-1", revision, true);
    assert_eq!(runner.snapshot().unwrap()["display"]["title"], "Restart?");
    let duplicate = send_identified_save_result(&mut runner, "restart-1", revision, true);
    assert_eq!(runner.snapshot().unwrap()["display"]["title"], "Restart?");
    assert!(!duplicate.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects.iter().any(|effect| matches!(effect, RuntimePlatformEffect::Reboot | RuntimePlatformEffect::StoreSaveRecovery { .. }))
    )));
}

#[test]
fn invalid_audio_baseline_omits_setting_only_save() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        jack_audio_required: true,
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.restart_settings.persisted_default["runtimeConfig"]["audioOutputs"] = json!({
        "dac": "invalid",
        "usb": false,
        "hdmi": false
    });
    enter_edit(&mut runner, "audioOutputs.usb");
    turn(&mut runner, 1);
    let _ = press(&mut runner);
    let snapshot = runner.snapshot().unwrap();
    assert!(!snapshot["display"]["lines"]
        .as_array()
        .unwrap()
        .iter()
        .any(|line| line == "  Save this setting"));
    let messages = choose_save_everything(&mut runner);
    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects.iter().any(|effect| matches!(effect, RuntimePlatformEffect::StoreSaveDefault { mode: Some(mode), .. } if mode == "restart-everything"))
    )));
}

#[test]
fn restart_dialogs_and_sound_rows_fit_the_oled_body() {
    let normal = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    assert_menu_lines_fit(&normal);
    let capacity = NativeRunner::new(NativeRunnerConfig {
        audio_optimization_capacity_available: true,
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    assert_menu_lines_fit(&capacity);

    let mut runner = changed_buffer_runner(false);
    let messages = press(&mut runner);
    assert_dialog_lines_fit(&snapshot_from(&messages));
    let _ = choose_save_everything(&mut runner);
    let revision = runner.restart_settings.pending_write_revision().unwrap();
    runner.register_default_write_request("fit-1", Some(revision));
    let messages = send_identified_save_result(&mut runner, "fit-1", revision, true);
    assert_dialog_lines_fit(&snapshot_from(&messages));
}

#[test]
fn back_during_save_and_restart_choice_only_cancels_restart_ui() {
    let mut runner = changed_buffer_runner(false);
    let _ = press(&mut runner);
    let _ = choose_save_everything(&mut runner);
    let revision = runner.restart_settings.pending_write_revision().unwrap();
    runner.register_default_write_request("back-1", Some(revision));
    let messages = back(&mut runner);
    assert_eq!(snapshot_from(&messages)["display"]["title"], "Saving...");
    assert!(
        runner.restart_settings.has_pending_write(),
        "{:#?}",
        runner.restart_settings
    );

    let _ = send_identified_save_result(&mut runner, "back-1", revision, true);
    let messages = back(&mut runner);
    assert!(runner.display.confirm_dialog.is_none());
    assert!(!messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects.iter().any(|effect| matches!(effect, RuntimePlatformEffect::Reboot))
    )));
}

#[test]
fn immediate_default_save_adapter_failure_exits_saving() {
    let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut host = ImmediateFailureHost;
    assert!(runner.menu.focus_item_key("sound.audioOutputBufferFrames"));

    for event in [
        json!({ "type": "encoder_press", "id": "main" }),
        json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
        json!({ "type": "encoder_press", "id": "main" }),
        json!({ "type": "encoder_turn", "delta": 2, "id": "main" }),
    ] {
        runtime
            .dispatch_host_message(
                HostMessage::DeviceInput {
                    input: event,
                    request_snapshot: None,
                },
                &mut runner,
                &mut host,
            )
            .unwrap();
    }
    let output = runtime
        .dispatch_host_message(
            HostMessage::DeviceInput {
                input: json!({ "type": "encoder_press", "id": "main" }),
                request_snapshot: None,
            },
            &mut runner,
            &mut host,
        )
        .unwrap();

    assert!(runner.display.confirm_dialog.is_none());
    assert!(!runner.restart_settings.has_pending_write());
    assert!(output.messages.iter().any(|message| matches!(
        message,
        RunnerMessage::RuntimeStatus { status } if status.error.is_some()
    )));
}
