use super::user_data_restore::{has_default_save, has_platform_effect, restore_status};
use super::*;
use crate::RuntimePlatformRequest;

#[test]
pub(crate) fn restore_rehydration_accepts_system_then_patch_before_releasing_write_barrier() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let restoring = runner
        .send(HostMessage::RuntimeResult {
            result: restore_status(RuntimeUserDataRestorePhase::Restoring),
        })
        .unwrap();
    assert!(runner.user_data_restore_is_active());
    let succeeded = runner
        .send(HostMessage::RuntimeResult {
            result: restore_status(RuntimeUserDataRestorePhase::Succeeded),
        })
        .unwrap();
    assert!(has_platform_effect(
        &succeeded,
        RuntimePlatformEffect::StoreLoadSystem
    ));
    assert!(!has_platform_effect(
        &succeeded,
        RuntimePlatformEffect::StoreLoadDefault
    ));
    let request = RuntimePlatformRequest::new(
        RuntimePlatformEffect::StoreLoadSystem,
        "restore-system-load".into(),
        None,
    );
    runner.register_platform_request(&request);
    let mut full = runner.config_payload();
    full["runtimeConfig"]["displayBrightness"] = json!(42);
    let documents = split_system_patch_documents(&full).unwrap();
    let loaded = runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::LoadSystemResult {
                payload: Some(documents.system.clone()),
            }
            .with_identity(request.request_id.clone(), request.revision),
        })
        .unwrap();
    assert!(runner.restore_rehydration_pending());
    assert_eq!(runner.display.ui.display_brightness, 42);
    assert!(has_platform_effect(
        &loaded,
        RuntimePlatformEffect::StoreLoadDefault
    ));
    let patch_request = RuntimePlatformRequest::new(
        RuntimePlatformEffect::StoreLoadDefault,
        "restore-patch-load".into(),
        Some(31),
    );
    runner.register_platform_request(&patch_request);
    let patch_result = runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::LoadDefaultResult {
                payload: Some(documents.patch),
            }
            .with_identity(patch_request.request_id, patch_request.revision),
        })
        .unwrap();
    assert!(!runner.restore_rehydration_pending());
    assert!(!runner.user_data_restore_is_active());
    assert_eq!(
        runner
            .display
            .user_data_restore
            .as_ref()
            .and_then(|restore| restore.request_id.as_deref()),
        Some("restore-1")
    );
    assert!(patch_result
        .iter()
        .any(|message| matches!(message, RunnerMessage::Snapshot { .. })));
    assert!(!has_platform_effect(
        &restoring,
        RuntimePlatformEffect::StoreLoadSystem
    ));
}

#[test]
pub(crate) fn successful_restore_rehydrates_live_runner() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut restored = runner.config_payload();
    restored["runtimeConfig"]["masterVolume"] = json!(81);
    restored["runtimeConfig"]["instruments"][0]["mixer"]["volume"] = json!(37);
    let documents = split_system_patch_documents(&restored).unwrap();

    let status_messages = runner
        .send(HostMessage::RuntimeResult {
            result: restore_status(RuntimeUserDataRestorePhase::Succeeded),
        })
        .unwrap();
    assert!(has_platform_effect(
        &status_messages,
        RuntimePlatformEffect::StoreLoadSystem
    ));
    assert!(!has_platform_effect(
        &status_messages,
        RuntimePlatformEffect::StoreLoadDefault
    ));
    assert!(runner.restore_rehydration_pending());

    let system_request = RuntimePlatformRequest::new(
        RuntimePlatformEffect::StoreLoadSystem,
        "restore-system".into(),
        None,
    );
    runner.register_platform_request(&system_request);

    let system_messages = runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::LoadSystemResult {
                payload: Some(documents.system),
            }
            .with_identity(system_request.request_id, system_request.revision),
        })
        .unwrap();
    assert!(runner.restore_rehydration_pending());
    assert!(has_platform_effect(
        &system_messages,
        RuntimePlatformEffect::StoreLoadDefault
    ));
    let patch_request = RuntimePlatformRequest::new(
        RuntimePlatformEffect::StoreLoadDefault,
        "restore-default-patch".into(),
        Some(34),
    );
    runner.register_platform_request(&patch_request);
    let applied_messages = runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::LoadDefaultResult {
                payload: Some(documents.patch),
            }
            .with_identity(patch_request.request_id, patch_request.revision),
        })
        .unwrap();
    assert_eq!(runner.config_payload()["runtimeConfig"]["masterVolume"], 81);
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["instruments"][0]["mixer"]["volume"],
        37
    );
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
    let request = RuntimePlatformRequest::new(
        RuntimePlatformEffect::StoreLoadSystem,
        "failed-restore-system".into(),
        None,
    );
    runner.register_platform_request(&request);
    let rejected = runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::LoadSystemResult {
                payload: Some(json!({"runtimeConfig": "invalid"})),
            }
            .with_identity(request.request_id, request.revision),
        })
        .unwrap();
    assert!(has_default_save(&rejected, Some("deferred")));
    assert!(!has_platform_effect(
        &rejected,
        RuntimePlatformEffect::StoreLoadDefault
    ));
    assert_eq!(
        runner
            .display
            .user_data_restore
            .as_ref()
            .map(|restore| restore.status.phase.clone()),
        Some(RuntimeUserDataRestorePhase::Failed)
    );
    assert!(!runner.restore_rehydration_pending());
    assert!(runner.pending.pending_save_revision.is_some());
}

#[test]
pub(crate) fn rejected_restore_patch_keeps_dirty_writes_blocked_after_failure() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.auto_save_default = true;
    runner.rolling_backups = true;
    runner.config_dirty = true;
    runner.dirty_revision = Some(19);
    runner.pending.pending_save_revision = Some(19);
    let old_patch = runner.patch_payload().unwrap();
    runner
        .send(HostMessage::RuntimeResult {
            result: restore_status(RuntimeUserDataRestorePhase::Succeeded),
        })
        .unwrap();
    let full = runner.config_payload();
    let documents = split_system_patch_documents(&full).unwrap();
    let system_request = RuntimePlatformRequest::new(
        RuntimePlatformEffect::StoreLoadSystem,
        "failed-patch-system".into(),
        None,
    );
    runner.register_platform_request(&system_request);
    let system_messages = runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::LoadSystemResult {
                payload: Some(documents.system),
            }
            .with_identity(system_request.request_id, system_request.revision),
        })
        .unwrap();
    assert!(runner.restore_rehydration_pending());
    assert!(has_platform_effect(
        &system_messages,
        RuntimePlatformEffect::StoreLoadDefault
    ));
    let patch_request = RuntimePlatformRequest::new(
        RuntimePlatformEffect::StoreLoadDefault,
        "failed-patch-child".into(),
        Some(20),
    );
    runner.register_platform_request(&patch_request);
    let mut truncated = documents.patch;
    truncated["runtimeConfig"]["instruments"]
        .as_array_mut()
        .unwrap()
        .pop();

    let failure = runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::LoadDefaultResult {
                payload: Some(truncated),
            }
            .with_identity(patch_request.request_id, patch_request.revision),
        })
        .unwrap();

    assert_eq!(
        runner
            .display
            .user_data_restore
            .as_ref()
            .map(|restore| restore.status.phase.clone()),
        Some(RuntimeUserDataRestorePhase::Failed)
    );
    assert!(runner.restore_rehydration_pending());
    assert!(runner.user_data_restore_is_active());
    assert_eq!(
        runner
            .display
            .user_data_restore
            .as_ref()
            .and_then(|restore| restore.request_id.as_deref()),
        Some("restore-1")
    );
    assert!(runner.config_dirty);
    assert_eq!(runner.patch_payload().unwrap(), old_patch);
    assert!(runner.display.runtime_error_presentation.is_some());
    assert!(failure
        .iter()
        .any(|message| matches!(message, RunnerMessage::Snapshot { .. })));
    let late_status = runner
        .send(HostMessage::RuntimeResult {
            result: restore_status(RuntimeUserDataRestorePhase::Failed),
        })
        .unwrap();
    assert!(runner.restore_rehydration_pending());
    let later_tick = runner
        .send(HostMessage::TransportPulseStep {
            pulses: 0,
            source: crate::SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();
    let later = runner.messages_with_snapshot().unwrap();
    for messages in [&late_status, &later_tick, &later] {
        assert!(!messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects.iter().any(|effect| matches!(effect,
                RuntimePlatformEffect::StoreSaveDefault { .. }
                    | RuntimePlatformEffect::StoreSaveBackup { .. }
                    | RuntimePlatformEffect::StoreSaveRecovery { .. }
            ))
        )));
    }
}
