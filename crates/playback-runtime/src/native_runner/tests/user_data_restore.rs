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
