use super::tests::snapshot_from;
use super::*;
use std::time::{Duration, Instant};

fn input(runner: &mut NativeRunner, input: Value) -> Vec<RunnerMessage> {
    runner
        .send_music_first(HostMessage::DeviceInput {
            input,
            request_snapshot: None,
        })
        .unwrap()
}

fn standard_input(runner: &mut NativeRunner, input: Value) -> Vec<RunnerMessage> {
    runner
        .send(HostMessage::DeviceInput {
            input,
            request_snapshot: None,
        })
        .unwrap()
}

fn press_main(runner: &mut NativeRunner) -> Vec<RunnerMessage> {
    input(runner, json!({ "type": "encoder_press", "id": "main" }))
}

fn turn_main(runner: &mut NativeRunner, delta: i64) -> Vec<RunnerMessage> {
    input(
        runner,
        json!({ "type": "encoder_turn", "id": "main", "delta": delta }),
    )
}

fn confirm_load(runner: &mut NativeRunner, key: &str) -> Vec<RunnerMessage> {
    assert!(runner.menu.focus_item_key(key));
    let opened = press_main(runner);
    let expected = if key == "default.load" {
        "Confirm Patch"
    } else {
        "Confirm Load"
    };
    let snapshot = snapshot_from(&opened);
    assert_eq!(snapshot["display"]["title"], expected);
    let lines = snapshot["display"]["lines"].as_array().unwrap();
    let cancel_index = lines.iter().position(|line| line == "> Cancel").unwrap();
    let confirm_index = lines
        .iter()
        .position(|line| line.as_str().is_some_and(|line| line.trim() == "Confirm"))
        .unwrap();
    assert!(cancel_index < confirm_index);
    let _ = turn_main(runner, 1);
    press_main(runner)
}

fn preset_runner() -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let _ = runner
        .send_music_first(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::ListPresetsResult {
                names: vec!["Alpha".into()],
            },
        })
        .unwrap();
    runner
}

fn has_patch_load_or_save(messages: &[RunnerMessage]) -> bool {
    messages.iter().any(|message| matches!(message,
        RunnerMessage::PlatformEffects { effects } if effects.iter().any(|effect| matches!(effect,
            RuntimePlatformEffect::StoreLoadDefault
                | RuntimePlatformEffect::StoreLoadPreset { .. }
                | RuntimePlatformEffect::StoreSaveDefault { .. }
        ))
    ))
}

fn assert_blocked_before_confirmation(runner: &mut NativeRunner, key: &str) {
    let before = (
        runner.restart_settings.clone(),
        runner.pending.pending_save_revision,
        runner.pending.manual_save_request.clone(),
        runner.pending.pending_autosave_payload_due_at,
        runner.pending.autosave_payload_notified_at,
    );
    assert!(runner.menu.focus_item_key(key));
    let messages = press_main(runner);
    assert!(!has_patch_load_or_save(&messages));
    assert!(runner.display.confirm_dialog.is_none());
    assert_eq!(
        runner
            .display
            .toast
            .as_ref()
            .map(|toast| toast.message.as_str()),
        Some("Save pending, try again")
    );
    assert!(messages.iter().any(|message| matches!(message,
        RunnerMessage::Snapshot { snapshot }
            if snapshot["display"]["toast"]
                .as_str()
                .is_some_and(|toast| toast.contains("pending"))
    )));
    assert_eq!(
        (
            runner.restart_settings.clone(),
            runner.pending.pending_save_revision,
            runner.pending.manual_save_request.clone(),
            runner.pending.pending_autosave_payload_due_at,
            runner.pending.autosave_payload_notified_at,
        ),
        before
    );
}

fn queue_manual_default_save(runner: &mut NativeRunner) {
    assert!(runner.menu.focus_item_key("default.save"));
    let opened = press_main(runner);
    assert_eq!(snapshot_from(&opened)["display"]["title"], "Confirm Patch");
    let _ = turn_main(runner, 1);
    let _ = press_main(runner);
    assert_eq!(
        runner.pending.manual_save_request,
        Some(NativeManualSaveRequest::Default)
    );
}

fn queue_default_save_write(runner: &mut NativeRunner) -> u64 {
    assert!(runner.menu.focus_item_key("default.save"));
    let _ = standard_input(runner, json!({ "type": "encoder_press", "id": "main" }));
    let _ = standard_input(
        runner,
        json!({ "type": "encoder_turn", "id": "main", "delta": 1 }),
    );
    let saved = standard_input(runner, json!({ "type": "encoder_press", "id": "main" }));
    let revision = saved
        .iter()
        .find_map(|message| match message {
            RunnerMessage::PlatformEffects { effects } => {
                effects.iter().find_map(|effect| match effect {
                    RuntimePlatformEffect::StoreSaveDefault { .. } => Some(runner.config_revision),
                    _ => None,
                })
            }
            _ => None,
        })
        .expect("default save request");
    assert!(runner.restart_settings.has_pending_write());
    revision
}

#[test]
fn pending_manual_default_save_refuses_default_and_preset_load_device_flows() {
    let mut runner = preset_runner();
    queue_manual_default_save(&mut runner);

    assert_blocked_before_confirmation(&mut runner, "default.load");
    assert_blocked_before_confirmation(&mut runner, "preset.load.Alpha");
}

#[test]
fn pending_default_write_refuses_default_and_named_load_before_confirmation() {
    let mut runner = preset_runner();
    let _ = queue_default_save_write(&mut runner);

    assert_blocked_before_confirmation(&mut runner, "default.load");
    assert_blocked_before_confirmation(&mut runner, "preset.load.Alpha");
}

#[test]
fn save_started_while_load_confirmation_is_open_is_rechecked_on_confirm() {
    let mut runner = preset_runner();
    assert!(runner.menu.focus_item_key("default.load"));
    let opened = press_main(&mut runner);
    assert_eq!(snapshot_from(&opened)["display"]["title"], "Confirm Patch");
    let opened_snapshot = snapshot_from(&opened);
    let lines = opened_snapshot["display"]["lines"].as_array().unwrap();
    let cancel_index = lines.iter().position(|line| line == "> Cancel").unwrap();
    let confirm_index = lines
        .iter()
        .position(|line| line.as_str().is_some_and(|line| line.trim() == "Confirm"))
        .unwrap();
    assert!(cancel_index < confirm_index);

    runner.pending.pending_save_revision = Some(runner.config_revision);
    let _ = turn_main(&mut runner, 1);
    let confirmed = press_main(&mut runner);
    assert!(!has_patch_load_or_save(&confirmed));
    assert_eq!(
        runner
            .display
            .toast
            .as_ref()
            .map(|toast| toast.message.as_str()),
        Some("Save pending, try again")
    );
}

#[test]
fn pending_patch_revision_and_autosave_debounce_refuse_load_without_emitting_old_save() {
    let mut runner = preset_runner();
    runner.auto_save_default = true;
    runner.mark_fast_autosave_dirty();
    let due = runner.pending.pending_autosave_payload_due_at.unwrap();
    assert!(runner
        .persistence_intent_at(due - Duration::from_millis(1))
        .is_none());
    let revision = runner.dirty_revision.unwrap();
    runner.pending.pending_save_revision = Some(revision);
    runner.pending.pending_autosave_payload_due_at =
        Some(Instant::now() - Duration::from_millis(1));

    assert_blocked_before_confirmation(&mut runner, "default.load");
    assert_blocked_before_confirmation(&mut runner, "preset.load.Alpha");
}

#[test]
fn autosave_enabled_dirty_patch_is_blocked_before_debounce_has_a_persistence_intent() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.auto_save_default = true;
    runner.mark_fast_autosave_dirty();
    let due = runner.pending.pending_autosave_payload_due_at.unwrap();
    assert!(runner
        .persistence_intent_at(due - Duration::from_millis(1))
        .is_none());
    runner.pending.pending_save_revision = None;

    assert_blocked_before_confirmation(&mut runner, "default.load");
}

#[test]
fn dirty_patch_with_autosave_disabled_and_no_pending_save_can_be_loaded() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.auto_save_default = false;
    runner.mark_fast_autosave_dirty();
    assert!(runner.pending.pending_save_revision.is_none());
    assert!(runner.pending.pending_autosave_payload_due_at.is_some());
    assert!(!runner.restart_settings.has_pending_write());

    let messages = confirm_load(&mut runner, "default.load");
    assert!(messages.iter().any(|message| matches!(message,
        RunnerMessage::PlatformEffects { effects }
            if effects.contains(&RuntimePlatformEffect::StoreLoadDefault)
    )));
}

#[test]
fn cleared_pending_patch_save_allows_default_load_and_system_info_is_not_blocked() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let messages = confirm_load(&mut runner, "default.load");
    assert!(messages.iter().any(|message| matches!(message,
        RunnerMessage::PlatformEffects { effects }
            if effects.contains(&RuntimePlatformEffect::StoreLoadDefault)
    )));

    runner.mark_config_dirty();
    runner.pending.pending_save_revision = Some(runner.config_revision);
    assert_eq!(
        runner.platform_effect_for_action("system.info").unwrap(),
        Some(RuntimePlatformEffect::SystemInfoRequest)
    );
}

#[test]
fn eligible_default_load_opens_cancel_first_confirmation() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    assert!(runner.menu.focus_item_key("default.load"));
    let opened = press_main(&mut runner);
    let snapshot = snapshot_from(&opened);
    assert_eq!(snapshot["display"]["title"], "Confirm Patch");
    let lines = snapshot["display"]["lines"].as_array().unwrap();
    let cancel_index = lines.iter().position(|line| line == "> Cancel").unwrap();
    let confirm_index = lines
        .iter()
        .position(|line| line.as_str().is_some_and(|line| line.trim() == "Confirm"))
        .unwrap();
    assert!(cancel_index < confirm_index);
}

#[test]
fn successful_default_save_completion_clears_admission_and_allows_load() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let revision = queue_default_save_write(&mut runner);
    runner.register_default_write_request("default-save-finished", Some(revision));
    let _ = runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::SaveDefaultResult {
                ok: true,
                is_auto: None,
            }
            .with_identity("default-save-finished".into(), Some(revision)),
        })
        .unwrap();
    assert!(!runner.restart_settings.has_pending_write());

    let loaded = confirm_load(&mut runner, "default.load");
    assert!(loaded.iter().any(|message| matches!(message,
        RunnerMessage::PlatformEffects { effects }
            if effects.contains(&RuntimePlatformEffect::StoreLoadDefault)
    )));
}
