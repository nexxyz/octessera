use super::*;
use crate::native_menu::NativeMenuAction;

fn system_info_failure() -> RuntimeStoreResult {
    RuntimeStoreResult::RuntimeFailure {
        error: crate::protocol::RuntimeErrorFacts::new(
            crate::RuntimeErrorDomain::Runtime,
            crate::RuntimeErrorCode::OperationFailed,
            crate::RuntimeOperation::SystemInfo,
            Some("system info failed".into()),
        ),
    }
}

#[test]
fn system_info_runtime_failure_updates_modal_and_clears_error_overlay() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.display.system_info_modal = Some(NativeSystemInfoModal::loading());
    runner.display.system_info_modal.as_mut().unwrap().scroll = 3;
    runner.display.runtime_error_presentation = Some(NativeRuntimeErrorPresentation {
        title: "old error".into(),
        lines: vec!["old error".into()],
    });

    runner.apply_store_result(system_info_failure()).unwrap();

    assert!(runner.display.runtime_error_presentation.is_none());
    assert_eq!(runner.display.system_info_modal.as_ref().unwrap().scroll, 0);
    assert!(matches!(
        runner.display.system_info_modal.as_ref().unwrap().state,
        NativeSystemInfoState::Error(ref error) if error.message == "system info failed"
    ));
}

#[test]
fn load_preset_without_payload_only_presents_the_named_load() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let behavior = runner.behavior;
    let transport = runner.transport.clone();
    runner
        .apply_store_result(RuntimeStoreResult::LoadPresetResult {
            name: "Empty patch".into(),
            payload: None,
        })
        .unwrap();

    assert_eq!(runner.behavior, behavior);
    assert_eq!(runner.transport, transport);
    assert_eq!(runner.current_preset_name.as_deref(), Some("Empty patch"));
    assert_eq!(
        runner.display.toast.as_ref().unwrap().message,
        "Loaded Empty patch"
    );
    assert!(!runner.outbox.has_platform_effects());
}

#[test]
fn failed_delete_and_default_operations_are_persistence_no_ops() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.current_preset_name = Some("Current".into());
    runner.display.toast = Some(NativeToast {
        message: "before".into(),
        offset: 0,
    });
    runner.mark_config_dirty();
    let revision = runner.config_revision;
    runner
        .outbox
        .push_platform_effect(RuntimePlatformEffect::MidiPanic);

    runner
        .apply_store_result(RuntimeStoreResult::DeletePresetResult {
            name: "Current".into(),
            ok: false,
        })
        .unwrap();
    runner
        .apply_store_result(RuntimeStoreResult::SaveDefaultResult {
            ok: false,
            is_auto: Some(true),
        })
        .unwrap();

    assert_eq!(runner.current_preset_name.as_deref(), Some("Current"));
    assert_eq!(runner.display.toast.as_ref().unwrap().message, "before");
    assert!(runner.config_dirty);
    assert_eq!(runner.dirty_revision, Some(revision));
    assert_eq!(
        runner.outbox.drain_platform_effects(),
        vec![RuntimePlatformEffect::MidiPanic]
    );
}

#[test]
fn rename_save_appends_delete_after_existing_outbox_and_preserves_message_order() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.skip_startup_splash();
    runner.preset_rename_source = Some("Old".into());
    let existing = RuntimePlatformEffect::StoreSaveRecovery {
        payload: json!({"existing": true}),
    };
    runner.outbox.push_platform_effect(existing.clone());

    let messages = runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::SavePresetResult {
                name: "New".into(),
                outcome: "saved".into(),
            },
        })
        .unwrap();
    let effects = messages
        .iter()
        .find_map(|message| match message {
            RunnerMessage::PlatformEffects { effects } => Some(effects.clone()),
            _ => None,
        })
        .expect("rename platform effects");

    assert_eq!(
        effects,
        vec![
            existing,
            RuntimePlatformEffect::StoreDeletePreset { name: "Old".into() },
        ]
    );
    assert_eq!(runner.current_preset_name.as_deref(), Some("New"));
    assert_eq!(runner.display.toast.as_ref().unwrap().message, "Saved New");
}

#[test]
fn persistence_no_ops_and_store_errors_keep_confirmation_priority() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.display.confirm_dialog = Some(NativeConfirmDialog {
        title: "Confirm".into(),
        lines: vec!["Confirm".into()],
        options: vec!["Cancel".into(), "Confirm".into()],
        cursor: 0,
        action: NativeMenuAction::NavigateBack,
        cancel_toast: None,
        confirm_before_execute: false,
    });
    runner.display.usb_sd_transfer_modal = Some(NativeUsbSdTransferModal {
        title: "Transfer".into(),
        lines: vec!["Transfer".into()],
    });

    runner
        .apply_store_result(RuntimeStoreResult::SaveBackupResult { ok: false })
        .unwrap();
    runner
        .apply_store_result(RuntimeStoreResult::SaveRecoveryResult { ok: false })
        .unwrap();
    runner
        .apply_store_result(RuntimeStoreResult::OperationSucceeded {
            operation: crate::RuntimeOperation::StoreSaveBackup,
            request_id: None,
            revision: None,
        })
        .unwrap();
    assert!(runner.display.toast.is_none());

    runner
        .apply_store_result(RuntimeStoreResult::StoreError {
            message: "Store unavailable".into(),
        })
        .unwrap();
    assert!(runner.display.confirm_dialog.is_some());
    assert!(runner.display.usb_sd_transfer_modal.is_none());
    assert_eq!(
        runner.display.toast.as_ref().unwrap().message,
        "Store unavailable"
    );
}

fn library_load_labels(runner: &NativeRunner) -> Vec<String> {
    runner
        .menu
        .item_for_key("preset.library.load")
        .unwrap()
        .children
        .iter()
        .map(|item| item.label.clone())
        .collect()
}

fn host_result(runner: &mut NativeRunner, result: RuntimeStoreResult) {
    runner.send(HostMessage::RuntimeResult { result }).unwrap();
}

#[test]
fn a_host_saved_preset_joins_the_load_lists_once_in_name_order() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.skip_startup_splash();
    host_result(
        &mut runner,
        RuntimeStoreResult::ListPresetsResult {
            names: vec!["Bass".into()],
        },
    );

    for _ in 0..2 {
        host_result(
            &mut runner,
            RuntimeStoreResult::SavePresetResult {
                name: "Arp".into(),
                outcome: "created".into(),
            },
        );
    }

    assert_eq!(library_load_labels(&runner), vec!["Arp", "Bass"]);
    let shortcut = runner.menu.item_for_key("preset.load").unwrap();
    assert_eq!(shortcut.children.len(), 2);
}

#[test]
fn a_deleted_preset_leaves_the_load_lists() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.skip_startup_splash();
    host_result(
        &mut runner,
        RuntimeStoreResult::ListPresetsResult {
            names: vec!["Arp".into(), "Bass".into()],
        },
    );

    host_result(
        &mut runner,
        RuntimeStoreResult::DeletePresetResult {
            name: "Arp".into(),
            ok: true,
        },
    );

    assert_eq!(library_load_labels(&runner), vec!["Bass"]);
}
