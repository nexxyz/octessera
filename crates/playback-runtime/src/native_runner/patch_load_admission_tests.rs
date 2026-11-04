use super::tests::snapshot_from;
use super::*;

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

fn effects(messages: &[RunnerMessage]) -> Vec<RuntimePlatformEffect> {
    messages
        .iter()
        .filter_map(|message| match message {
            RunnerMessage::PlatformEffects { effects } => Some(effects.clone()),
            _ => None,
        })
        .flatten()
        .collect()
}

fn is_load(effect: &RuntimePlatformEffect) -> bool {
    matches!(
        effect,
        RuntimePlatformEffect::StoreLoadDefault | RuntimePlatformEffect::StoreLoadPreset { .. }
    )
}

fn toast(runner: &NativeRunner) -> Option<&str> {
    runner
        .display
        .toast
        .as_ref()
        .map(|toast| toast.message.as_str())
}

/// Confirms a load that has to wait: no load goes out yet, and the dialog is closed.
fn confirm_waiting_load(runner: &mut NativeRunner, key: &str) -> Vec<RunnerMessage> {
    let messages = confirm_load(runner, key);
    assert!(
        !effects(&messages).iter().any(is_load),
        "{key} loaded early"
    );
    assert!(runner.display.confirm_dialog.is_none());
    assert_eq!(toast(runner), Some("Saving, then loading"));
    messages
}

fn finish_default_save(
    runner: &mut NativeRunner,
    request_id: &str,
    ok: bool,
) -> Vec<RunnerMessage> {
    let revision = runner.pending_default_write_revision();
    runner.register_default_write_request(request_id, revision);
    let result = if ok {
        RuntimeStoreResult::SaveDefaultResult {
            ok: true,
            is_auto: None,
        }
    } else {
        RuntimeStoreResult::RuntimeFailure {
            error: crate::RuntimeErrorFacts::new(
                crate::RuntimeErrorDomain::Storage,
                crate::RuntimeErrorCode::OperationFailed,
                crate::RuntimeOperation::StoreSaveDefault,
                Some("disk full".into()),
            ),
        }
    };
    runner
        .send(HostMessage::RuntimeResult {
            result: result.with_identity(request_id.into(), revision),
        })
        .unwrap()
}

/// The Pi side of a music-first manual save: take the request, write it, report success.
fn finish_manual_default_save(runner: &mut NativeRunner) -> Vec<RunnerMessage> {
    assert_eq!(
        runner.take_manual_save_request(),
        Some(NativeManualSaveRequest::Default)
    );
    let revision = runner.config_revision;
    assert!(runner.register_native_default_write("manual-save", revision, false));
    let payload = std::sync::Arc::new(
        runner
            .capture_config_snapshot()
            .into_portable_patch_payload()
            .unwrap(),
    );
    assert!(runner.attach_native_default_write_payload("manual-save", revision, payload));
    runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::SaveDefaultResult {
                ok: true,
                is_auto: None,
            }
            .with_identity("manual-save".into(), Some(revision)),
        })
        .unwrap()
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

#[test]
fn pending_default_write_finishes_before_default_and_named_load() {
    for (key, load) in [
        ("default.load", RuntimePlatformEffect::StoreLoadDefault),
        (
            "preset.load.Alpha",
            RuntimePlatformEffect::StoreLoadPreset {
                name: "Alpha".into(),
            },
        ),
    ] {
        let mut runner = preset_runner();
        let _ = queue_default_save_write(&mut runner);
        let waiting = confirm_waiting_load(&mut runner, key);
        assert!(!effects(&waiting)
            .iter()
            .any(|effect| matches!(effect, RuntimePlatformEffect::StoreSaveDefault { .. })));

        let saved = finish_default_save(&mut runner, "default-save", true);
        assert_eq!(
            effects(&saved)
                .into_iter()
                .filter(is_load)
                .collect::<Vec<_>>(),
            vec![load]
        );
    }
}

#[test]
fn pending_manual_default_save_finishes_before_load() {
    let mut runner = preset_runner();
    queue_manual_default_save(&mut runner);
    let _ = confirm_waiting_load(&mut runner, "preset.load.Alpha");

    let saved = finish_manual_default_save(&mut runner);
    assert_eq!(
        effects(&saved)
            .into_iter()
            .filter(is_load)
            .collect::<Vec<_>>(),
        vec![RuntimePlatformEffect::StoreLoadPreset {
            name: "Alpha".into()
        }]
    );
}

#[test]
fn dirty_autosave_patch_is_saved_now_then_loaded() {
    let mut runner = preset_runner();
    runner.auto_save_default = true;
    runner.mark_fast_autosave_dirty();
    assert!(runner.pending.pending_autosave_payload_due_at.is_some());

    let _ = confirm_waiting_load(&mut runner, "default.load");
    assert!(runner.pending.pending_autosave_payload_due_at.is_none());

    let saved = finish_manual_default_save(&mut runner);
    assert!(!runner.config_dirty);
    assert_eq!(
        effects(&saved)
            .into_iter()
            .filter(is_load)
            .collect::<Vec<_>>(),
        vec![RuntimePlatformEffect::StoreLoadDefault]
    );
}

#[test]
fn save_started_while_load_confirmation_is_open_finishes_first() {
    let mut runner = preset_runner();
    assert!(runner.menu.focus_item_key("default.load"));
    let _ = press_main(&mut runner);
    runner.auto_save_default = true;
    runner.mark_config_dirty();
    let _ = turn_main(&mut runner, 1);
    let confirmed = press_main(&mut runner);
    assert!(!effects(&confirmed).iter().any(is_load));
    assert_eq!(toast(&runner), Some("Saving, then loading"));

    let saved = finish_manual_default_save(&mut runner);
    assert!(effects(&saved).iter().any(is_load));
}

#[test]
fn failed_save_drops_the_waiting_load() {
    let mut runner = preset_runner();
    let _ = queue_default_save_write(&mut runner);
    let _ = confirm_waiting_load(&mut runner, "preset.load.Alpha");

    let failed = finish_default_save(&mut runner, "default-save", false);
    assert!(!effects(&failed).iter().any(is_load));
    assert_eq!(toast(&runner), Some("Save failed, not loaded"));
    assert!(runner.pending.patch_load_after_save.is_none());
}

#[test]
fn consecutive_loads_each_open_their_dialog_and_load() {
    let mut runner = preset_runner();
    let patch = super::system_persistence::SystemPersistenceState::patch_document(&runner).unwrap();
    assert!(runner.menu.focus_item_key("default.load"));
    for round in 0..5 {
        let opened = press_main(&mut runner);
        assert_eq!(
            snapshot_from(&opened)["display"]["title"],
            "Confirm Patch",
            "round {round}"
        );
        let _ = turn_main(&mut runner, 1);
        let confirmed = press_main(&mut runner);
        assert!(effects(&confirmed).contains(&RuntimePlatformEffect::StoreLoadDefault));
        let mut loaded = patch.clone();
        loaded["runtimeConfig"]["transport"]["bpm"] = json!(90 + round);
        let _ = runner
            .send(HostMessage::RuntimeResult {
                result: RuntimeStoreResult::LoadDefaultResult {
                    payload: Some(loaded),
                },
            })
            .unwrap();
        assert_eq!(runner.transport.bpm, f64::from(90 + round));
        assert_eq!(toast(&runner), Some("Patch loaded"));
        assert!(runner.display.confirm_dialog.is_none());
        assert_eq!(runner.menu.current_key(), Some("default.load"));
    }
}
