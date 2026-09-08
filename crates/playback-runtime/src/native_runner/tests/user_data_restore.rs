use super::*;
use crate::{RuntimeUserDataRestorePhase, RuntimeUserDataRestoreStatus};

fn restore_status(phase: RuntimeUserDataRestorePhase) -> RuntimeStoreResult {
    RuntimeStoreResult::UserDataRestoreStatus {
        status: RuntimeUserDataRestoreStatus { phase },
    }
    .with_identity("restore-1".into(), Some(1))
}

fn display_lines(messages: &[RunnerMessage]) -> Vec<String> {
    snapshot_from(messages)["display"]["lines"]
        .as_array()
        .unwrap()
        .iter()
        .map(|line| line.as_str().unwrap().to_string())
        .collect()
}

fn has_platform_effect(messages: &[RunnerMessage], expected: RuntimePlatformEffect) -> bool {
    messages.iter().any(|message| {
        matches!(
            message,
            RunnerMessage::PlatformEffects { effects } if effects.contains(&expected)
        )
    })
}

fn input(runner: &mut NativeRunner, value: Value) -> Vec<RunnerMessage> {
    runner
        .send(HostMessage::DeviceInput {
            input: value,
            request_snapshot: None,
        })
        .unwrap()
}

fn has_default_save(messages: &[RunnerMessage], mode: Option<&str>) -> bool {
    messages.iter().any(|message| {
        matches!(
            message,
            RunnerMessage::PlatformEffects { effects }
                if effects.iter().any(|effect| matches!(
                    effect,
                    RuntimePlatformEffect::StoreSaveDefault { mode: effect_mode, .. }
                        if effect_mode.as_deref() == mode
                ))
        )
    })
}

fn identified_default_save(request_id: &str, revision: u64) -> HostMessage {
    HostMessage::RuntimeResult {
        result: RuntimeStoreResult::Identified {
            result: Box::new(RuntimeStoreResult::SaveDefaultResult {
                ok: true,
                is_auto: None,
            }),
            request_id: request_id.into(),
            revision: Some(revision),
        },
    }
}

fn pending_restart_runner() -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.auto_save_default = true;
    assert!(runner.menu.focus_item_key("transport.bpm"));
    let _ = input(
        &mut runner,
        json!({ "type": "encoder_press", "id": "main" }),
    );
    let _ = input(
        &mut runner,
        json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
    );
    runner.make_deferred_menu_apply_due_for_test();
    let messages = runner.flush_deferred_menu_apply().unwrap();
    assert!(has_default_save(&messages, Some("deferred")));
    let revision = runner.restart_settings.pending_write_revision().unwrap();
    runner.register_default_write_request("deferred-restore", Some(revision));

    assert!(runner.menu.focus_item_key("sound.audioOutputBufferFrames"));
    let _ = input(
        &mut runner,
        json!({ "type": "encoder_press", "id": "main" }),
    );
    let _ = input(
        &mut runner,
        json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
    );
    let _ = input(
        &mut runner,
        json!({ "type": "encoder_press", "id": "main" }),
    );
    assert!(runner.restart_settings.has_pending_write());
    runner
}

#[test]
pub(crate) fn restore_invalidation_abandons_cancelled_default_write() {
    for outcome in ["failed", "missing", "invalid"] {
        let mut runner = pending_restart_runner();
        let bpm = runner.transport.bpm;
        let buffer = runner.audio_output_buffer_frames;
        assert!(runner
            .send(HostMessage::RuntimeResult {
                result: restore_status(RuntimeUserDataRestorePhase::Restoring),
            })
            .is_ok());
        assert!(!runner.restart_settings.has_pending_write(), "{outcome}");
        assert!(runner.pending.pending_save_revision.is_none(), "{outcome}");

        let failure_messages = match outcome {
            "failed" => runner
                .send(HostMessage::RuntimeResult {
                    result: restore_status(RuntimeUserDataRestorePhase::Failed),
                })
                .unwrap(),
            "missing" | "invalid" => {
                let succeeded = runner
                    .send(HostMessage::RuntimeResult {
                        result: restore_status(RuntimeUserDataRestorePhase::Succeeded),
                    })
                    .unwrap();
                assert!(has_platform_effect(
                    &succeeded,
                    RuntimePlatformEffect::StoreLoadDefault
                ));
                let result = if outcome == "missing" {
                    RuntimeStoreResult::LoadDefaultResult { payload: None }
                } else {
                    RuntimeStoreResult::LoadDefaultResult {
                        payload: Some(json!({ "runtimeConfig": "invalid" })),
                    }
                };
                let error = runner
                    .send(HostMessage::RuntimeResult { result })
                    .unwrap_err();
                assert!(!error.is_empty());
                Vec::new()
            }
            _ => unreachable!(),
        };

        if outcome == "failed" {
            assert!(has_default_save(&failure_messages, Some("deferred")));
            let recovery_revision = runner.restart_settings.pending_write_revision().unwrap();
            runner.register_default_write_request("restore-retry", Some(recovery_revision));
            let recovery = runner
                .send(identified_default_save("restore-retry", recovery_revision))
                .unwrap();
            assert!(!has_default_save(&recovery, Some("restart-everything")));
        }

        assert!(!runner.restart_settings.has_pending_write(), "{outcome}");
        assert!(runner.pending.pending_save_revision.is_none(), "{outcome}");
        assert!(!runner.restore_rehydration_pending(), "{outcome}");
        assert_eq!(runner.transport.bpm, bpm, "{outcome}");
        assert_eq!(runner.audio_output_buffer_frames, buffer, "{outcome}");
        if outcome != "failed" {
            assert!(runner.config_dirty, "{outcome}");
        }

        let dismissed = input(&mut runner, json!({ "type": "button_a", "pressed": true }));
        assert!(runner.display.user_data_restore.is_none(), "{outcome}");
        if has_default_save(&dismissed, Some("deferred")) {
            let recovery_revision = runner.restart_settings.pending_write_revision().unwrap();
            runner.register_default_write_request("restore-retry", Some(recovery_revision));
            let recovery = runner
                .send(identified_default_save("restore-retry", recovery_revision))
                .unwrap();
            assert!(!has_default_save(&recovery, Some("restart-everything")));
        }
        assert!(runner.menu.focus_item_key("default.save"));
        let _ = input(
            &mut runner,
            json!({ "type": "encoder_press", "id": "main" }),
        );
        let _ = input(
            &mut runner,
            json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
        );
        let messages = input(
            &mut runner,
            json!({ "type": "encoder_press", "id": "main" }),
        );
        assert!(has_default_save(&messages, None), "{outcome}");
        let revision = runner.restart_settings.pending_write_revision().unwrap();
        runner.register_default_write_request("future-default", Some(revision));
        let messages = runner
            .send(identified_default_save("future-default", revision))
            .unwrap();
        assert!(
            !has_default_save(&messages, Some("restart-everything")),
            "{outcome}"
        );

        assert!(runner.menu.focus_item_key("sound.audioOutputBufferFrames"));
        let _ = input(
            &mut runner,
            json!({ "type": "encoder_press", "id": "main" }),
        );
        let _ = input(
            &mut runner,
            json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
        );
        let messages = input(
            &mut runner,
            json!({ "type": "encoder_press", "id": "main" }),
        );
        assert!(
            has_default_save(&messages, Some("restart-everything")),
            "{outcome}"
        );
    }
}

#[test]
pub(crate) fn restore_lifecycle_is_typed_blocking_and_bounded() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let restoring = runner
        .send(HostMessage::RuntimeResult {
            result: restore_status(RuntimeUserDataRestorePhase::Restoring),
        })
        .unwrap();
    assert_eq!(
        snapshot_from(&restoring)["display"]["title"],
        "Restoring..."
    );
    assert_eq!(display_lines(&restoring), vec!["Please wait"]);
    assert!(runner
        .display
        .user_data_restore
        .as_ref()
        .is_some_and(|state| state.status.phase == RuntimeUserDataRestorePhase::Restoring));

    let blocked = runner
        .send(HostMessage::DeviceInput {
            input: json!({"type":"grid_press","x":1,"y":1}),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(display_lines(&blocked), vec!["Please wait"]);

    let succeeded = runner
        .send(HostMessage::RuntimeResult {
            result: restore_status(RuntimeUserDataRestorePhase::Succeeded),
        })
        .unwrap();
    assert_eq!(
        snapshot_from(&succeeded)["display"]["title"],
        "Restore complete"
    );
    assert_eq!(display_lines(&succeeded), vec!["Data restored", "> Close"]);
    assert!(display_lines(&succeeded)
        .iter()
        .all(|line| line.chars().count() <= 20));

    let closed = runner
        .send(HostMessage::DeviceInput {
            input: json!({"type":"button_a","pressed":true}),
            request_snapshot: None,
        })
        .unwrap();
    assert!(runner.display.user_data_restore.is_none());
    assert_ne!(
        snapshot_from(&closed)["display"]["title"],
        "Restore complete"
    );
}

#[test]
pub(crate) fn restore_failure_is_distinct_and_terminal_status_does_not_rewind() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner
        .apply_store_result(restore_status(RuntimeUserDataRestorePhase::Failed))
        .unwrap();
    runner
        .apply_store_result(restore_status(RuntimeUserDataRestorePhase::Succeeded))
        .unwrap();
    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({"type":"other"}),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(
        snapshot_from(&messages)["display"]["title"],
        "Restore failed"
    );
    assert_eq!(
        display_lines(&messages),
        vec!["Pre-restore kept", "> Close"]
    );
}

#[test]
pub(crate) fn successful_restore_rehydrates_live_runner() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut restored = runner.config_payload();
    restored["runtimeConfig"]["masterVolume"] = json!(81);

    let status_messages = runner
        .send(HostMessage::RuntimeResult {
            result: restore_status(RuntimeUserDataRestorePhase::Succeeded),
        })
        .unwrap();
    assert!(has_platform_effect(
        &status_messages,
        RuntimePlatformEffect::StoreLoadDefault
    ));
    assert!(runner.restore_rehydration_pending());

    let applied_messages = runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::LoadDefaultResult {
                payload: Some(restored.clone()),
            },
        })
        .unwrap();
    assert_eq!(runner.config_payload()["runtimeConfig"]["masterVolume"], 81);
    assert!(!runner.restore_rehydration_pending());
    assert!(applied_messages
        .iter()
        .any(|message| matches!(message, RunnerMessage::Snapshot { .. })));
}

#[test]
pub(crate) fn failed_restore_rehydration_marks_failure_and_retries_dirty_save() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.auto_save_default = true;
    runner.config_dirty = true;
    runner.dirty_revision = Some(7);
    runner.pending.pending_save_revision = Some(7);
    runner
        .send(HostMessage::RuntimeResult {
            result: restore_status(RuntimeUserDataRestorePhase::Succeeded),
        })
        .unwrap();

    let error = runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::LoadDefaultResult {
                payload: Some(json!({"runtimeConfig": "invalid"})),
            },
        })
        .unwrap_err();
    assert!(!error.is_empty());
    assert_eq!(
        runner
            .display
            .user_data_restore
            .as_ref()
            .map(|restore| restore.status.phase.clone()),
        Some(RuntimeUserDataRestorePhase::Failed)
    );
    assert!(!runner.restore_rehydration_pending());
    assert!(runner.pending.pending_save_revision.is_none());
    assert!(runner
        .messages_with_snapshot()
        .unwrap()
        .iter()
        .any(|message| {
            matches!(
                message,
                RunnerMessage::PlatformEffects { effects }
                    if effects.iter().any(|effect| matches!(
                        effect,
                        RuntimePlatformEffect::StoreSaveDefault { mode: Some(mode), .. }
                            if mode == "deferred"
                    ))
            )
        }));
}

#[test]
pub(crate) fn failed_restore_status_releases_pending_save_for_retry() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.auto_save_default = true;
    runner.config_dirty = true;
    runner.dirty_revision = Some(9);
    runner.pending.pending_save_revision = Some(9);
    let messages = runner
        .send(HostMessage::RuntimeResult {
            result: restore_status(RuntimeUserDataRestorePhase::Failed),
        })
        .unwrap();
    assert!(messages.iter().any(|message| {
        matches!(
            message,
            RunnerMessage::PlatformEffects { effects }
                if effects.iter().any(|effect| matches!(
                    effect,
                    RuntimePlatformEffect::StoreSaveDefault { mode: Some(mode), .. }
                        if mode == "deferred"
                ))
        )
    }));
}
