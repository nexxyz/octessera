use super::*;

fn ready_result() -> RuntimeStoreResult {
    transfer_result(RuntimeUserDataTransferPhase::Ready)
}

fn transfer_result(phase: RuntimeUserDataTransferPhase) -> RuntimeStoreResult {
    RuntimeStoreResult::UserDataTransferStatus {
        status: RuntimeUserDataTransferStatus {
            phase,
            url: Some("http://192.168.42.1:8081".into()),
            code: Some("Ab2Cd3Ef4H".into()),
            expires_in_seconds: Some(900),
        },
    }
    .with_identity("transfer-1".into(), Some(1))
}

fn send_main(runner: &mut NativeRunner, input: Value) -> Vec<RunnerMessage> {
    runner
        .send(HostMessage::DeviceInput {
            input,
            request_snapshot: None,
        })
        .unwrap()
}

#[test]
pub(crate) fn backup_restore_menu_action_is_direct_and_emits_open_effect() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        jack_audio_required: true,
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    assert!(runner.menu.focus_item_key("system.backupRestore"));
    let messages = send_main(
        &mut runner,
        json!({ "type": "encoder_press", "id": "main" }),
    );
    assert!(runner.display.confirm_dialog.is_none());
    assert!(messages.iter().any(|message| {
        matches!(message, RunnerMessage::PlatformEffects { effects }
            if effects == &[RuntimePlatformEffect::UserDataTransferOpen])
    }));
}

#[test]
pub(crate) fn ready_transfer_card_has_semantic_lines_and_bounded_oled_frame() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.apply_store_result(ready_result()).unwrap();
    let messages = runner.messages_with_snapshot().unwrap();
    let snapshot = snapshot_from(&messages);
    assert_eq!(snapshot["display"]["title"], "Backup / Restore");
    assert_eq!(snapshot["display"]["bodyLayout"], "card");
    assert_eq!(
        snapshot["display"]["lines"],
        json!([
            "Use local client",
            "IP 192.168.42.1",
            "PORT 8081",
            "CODE Ab2Cd3Ef4H",
            "Ends in 15 min",
            "Stop service"
        ])
    );
    assert_eq!(snapshot["selectedRow"], 5);
    assert!(snapshot["display"]["scrollOffset"].is_null());
    let input = crate::oled_frame::presentation_input_from_snapshot(
        &snapshot,
        crate::oled_frame::OledPresentationMetrics::default(),
    )
    .unwrap()
    .unwrap();
    let pixels = crate::oled_frame::render_oled_frame(&input);
    assert_eq!(pixels.len(), 128 * 128 * 2);
    assert!(pixels.iter().any(|pixel| *pixel != 0));
}

#[test]
pub(crate) fn back_hides_ready_transfer_card_while_stop_closes_it() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.apply_store_result(ready_result()).unwrap();
    let hidden = send_main(&mut runner, json!({ "type": "button_a", "pressed": true }));
    assert_eq!(
        snapshot_from(&hidden)["display"]["title"],
        runner.active_layer_title()
    );
    assert!(!runner.display.user_data_transfer.as_ref().unwrap().visible);

    runner.apply_store_result(ready_result()).unwrap();
    let closed = send_main(
        &mut runner,
        json!({ "type": "encoder_press", "id": "main" }),
    );
    assert!(closed.iter().any(|message| {
        matches!(message, RunnerMessage::PlatformEffects { effects }
            if effects == &[RuntimePlatformEffect::UserDataTransferClose])
    }));
    assert!(runner.display.user_data_transfer.is_none());
}

#[test]
fn an_uploaded_backup_shows_its_question_even_on_a_hidden_card_and_keeps_the_service() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.apply_store_result(ready_result()).unwrap();
    send_main(&mut runner, json!({ "type": "button_a", "pressed": true }));
    assert!(!runner.display.user_data_transfer.as_ref().unwrap().visible);

    runner
        .apply_store_result(transfer_result(
            RuntimeUserDataTransferPhase::ConfirmRestore,
        ))
        .unwrap();
    let snapshot = snapshot_from(&runner.messages_with_snapshot().unwrap());
    assert_eq!(snapshot["display"]["title"], "Restore backup?");
    assert_eq!(
        snapshot["display"]["lines"],
        json!([
            "Replaces the",
            "music on here.",
            "Main: Restore",
            "Back: Cancel"
        ])
    );
    assert!(snapshot["selectedRow"].is_null());
    for input in [
        json!({ "type": "encoder_press", "id": "main" }),
        json!({ "type": "button_a", "pressed": true }),
    ] {
        let messages = send_main(&mut runner, input);
        assert!(!messages
            .iter()
            .any(|message| matches!(message, RunnerMessage::PlatformEffects { .. })));
        assert_eq!(
            snapshot_from(&messages)["display"]["title"],
            "Restore backup?"
        );
    }

    runner.apply_store_result(ready_result()).unwrap();
    let snapshot = snapshot_from(&runner.messages_with_snapshot().unwrap());
    assert_eq!(snapshot["display"]["title"], "Backup / Restore");
}

#[test]
fn the_restore_question_outranks_an_open_confirmation_dialog_until_it_is_answered() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.apply_store_result(ready_result()).unwrap();
    send_main(&mut runner, json!({ "type": "button_a", "pressed": true }));
    super::in_place_menu_actions::press_key(&mut runner, "system.clearAll");
    assert!(runner.display.confirm_dialog.is_some());

    runner
        .apply_store_result(transfer_result(
            RuntimeUserDataTransferPhase::ConfirmRestore,
        ))
        .unwrap();
    let messages = send_main(
        &mut runner,
        json!({ "type": "encoder_press", "id": "main" }),
    );
    assert_eq!(
        snapshot_from(&messages)["display"]["title"],
        "Restore backup?"
    );
    assert!(runner.display.confirm_dialog.is_some());

    runner.apply_store_result(ready_result()).unwrap();
    assert_ne!(
        snapshot_from(&runner.messages_with_snapshot().unwrap())["display"]["title"],
        "Restore backup?"
    );
    assert!(runner.display.confirm_dialog.is_some());
}

#[test]
pub(crate) fn closed_status_clears_and_unsupported_is_dismissible() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.apply_store_result(ready_result()).unwrap();
    runner
        .apply_store_result(RuntimeStoreResult::UserDataTransferStatus {
            status: RuntimeUserDataTransferStatus {
                phase: RuntimeUserDataTransferPhase::Closed,
                url: None,
                code: None,
                expires_in_seconds: None,
            },
        })
        .unwrap();
    assert!(runner.display.user_data_transfer.is_none());

    runner
        .apply_store_result(RuntimeStoreResult::UserDataTransferStatus {
            status: RuntimeUserDataTransferStatus {
                phase: RuntimeUserDataTransferPhase::Unsupported,
                url: None,
                code: None,
                expires_in_seconds: None,
            },
        })
        .unwrap();
    let unsupported = runner.messages_with_snapshot().unwrap();
    assert_eq!(
        snapshot_from(&unsupported)["display"]["lines"],
        json!(["Not supported here", "Use a Pi device", "Close"])
    );
    let dismissed = send_main(&mut runner, json!({ "type": "button_a", "pressed": true }));
    assert_eq!(
        snapshot_from(&dismissed)["display"]["title"],
        runner.active_layer_title()
    );
    assert!(runner.display.user_data_transfer.is_none());
}

#[test]
pub(crate) fn restore_progress_and_runtime_error_keep_priority_over_transfer_card() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.apply_store_result(ready_result()).unwrap();
    runner.apply_user_data_restore_status(
        RuntimeUserDataRestoreStatus {
            phase: RuntimeUserDataRestorePhase::Restoring,
        },
        None,
        None,
    );
    assert_eq!(
        runner.snapshot().unwrap()["display"]["title"],
        "Restoring..."
    );
    runner.display.user_data_restore = None;
    runner.display.runtime_error_presentation = Some(NativeRuntimeErrorPresentation {
        title: "Runtime error".into(),
        lines: vec!["Check device".into()],
    });
    let snapshot = runner.snapshot().unwrap();
    assert_eq!(snapshot["display"]["title"], "Runtime error");
    assert_eq!(snapshot["display"]["bodyLayout"], "rows");
}

#[test]
fn export_status_toasts_over_the_open_transfer_card() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.apply_store_result(ready_result()).unwrap();
    runner
        .apply_store_result(
            RuntimeStoreResult::UserDataExportStatus {
                message: "Export: 2 items skipped".into(),
            }
            .with_identity("transfer-1".into(), Some(1)),
        )
        .unwrap();
    assert_eq!(
        runner.display.toast.as_ref().unwrap().message,
        "Export: 2 items skipped"
    );
    assert!(runner
        .display
        .user_data_transfer
        .as_ref()
        .is_some_and(|transfer| transfer.visible));
}
