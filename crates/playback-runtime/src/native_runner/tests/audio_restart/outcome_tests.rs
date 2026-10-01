use super::*;

#[test]
fn stale_save_ack_does_not_advance_restart_flow() {
    let mut runner = changed_buffer_runner(false);
    let _ = commit_with_main(&mut runner);
    runner.display.confirm_dialog.as_mut().unwrap().cursor = 2;

    let revision = runner.config_revision;
    let _ = choose_save_everything(&mut runner);
    runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::Identified {
                result: Box::new(RuntimeStoreResult::SaveDefaultResult {
                    ok: true,
                    is_auto: None,
                }),
                request_id: "stale".into(),
                revision: Some(revision.saturating_sub(1)),
            },
        })
        .unwrap();
    assert_eq!(runner.snapshot().unwrap()["display"]["title"], "Saving...");
}

#[test]
fn failed_save_does_not_reboot_or_advance_baseline() {
    let mut runner = changed_buffer_runner(false);
    let baseline = runner.restart_settings.persisted_default.clone();
    let _ = commit_with_main(&mut runner);
    let _ = choose_save_everything(&mut runner);

    let messages = identified_save_result(&mut runner, "save-1", false);
    assert_eq!(runner.restart_settings.persisted_default, baseline);
    assert_eq!(snapshot_from(&messages)["display"]["toast"], "Save failed");
    assert!(!messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects.iter().any(|effect| matches!(effect, RuntimePlatformEffect::Reboot))
    )));
}

#[test]
fn successful_save_offers_reboot_and_reboot_uses_recovery_lifecycle() {
    let mut runner = changed_buffer_runner(false);
    let _ = commit_with_main(&mut runner);
    let _ = choose_save_everything(&mut runner);
    let _ = identified_save_result(&mut runner, "save-1", true);
    assert_eq!(runner.snapshot().unwrap()["display"]["title"], "Restart?");
    assert_dialog_lines_fit(&runner.snapshot().unwrap());

    turn_confirm(&mut runner, 1);
    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    let effects: Vec<&RuntimePlatformEffect> = messages
        .iter()
        .filter_map(|message| match message {
            RunnerMessage::PlatformEffects { effects } => Some(effects),
            _ => None,
        })
        .flatten()
        .collect();
    assert_eq!(
        effects
            .iter()
            .filter(|effect| matches!(effect, RuntimePlatformEffect::StoreSaveRecovery { .. }))
            .count(),
        1
    );
    assert_eq!(
        effects
            .iter()
            .filter(|effect| matches!(effect, RuntimePlatformEffect::Reboot))
            .count(),
        1
    );
    assert!(!effects.iter().any(|effect| matches!(
        effect,
        RuntimePlatformEffect::ApplyDeviceConfigReboot { .. }
    )));
}

#[test]
fn continue_after_successful_save_does_not_reboot() {
    let mut runner = changed_buffer_runner(false);
    let _ = commit_with_main(&mut runner);
    let _ = choose_save_everything(&mut runner);
    let _ = identified_save_result(&mut runner, "save-1", true);

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    assert!(!messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects.iter().any(|effect| matches!(effect, RuntimePlatformEffect::Reboot))
    )));
}
