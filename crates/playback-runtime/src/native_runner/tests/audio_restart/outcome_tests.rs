use super::*;

#[test]
fn stale_save_ack_does_not_advance_restart_flow() {
    let mut runner = changed_optimization_runner(false);
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
    let mut runner = changed_optimization_runner(false);
    let baseline = runner.restart_settings.persisted_default.clone();
    let _ = commit_with_main(&mut runner);
    let _ = choose_save_everything(&mut runner);

    let messages = identified_save_result(&mut runner, "save-1", false);
    assert_eq!(runner.restart_settings.persisted_default, baseline);
    assert_eq!(snapshot_from(&messages)["display"]["toast"], "Save failed");
    assert!(!messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects.iter().any(|effect| matches!(effect, RuntimePlatformEffect::Reboot | RuntimePlatformEffect::RestartApp))
    )));
}

fn confirm_restart_now(runner: &mut NativeRunner) -> Vec<RuntimePlatformEffect> {
    turn_confirm(runner, 1);
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap()
        .into_iter()
        .filter_map(|message| match message {
            RunnerMessage::PlatformEffects { effects } => Some(effects),
            _ => None,
        })
        .flatten()
        .collect()
}

fn restart_dialog_lines(runner: &NativeRunner) -> Vec<String> {
    let snapshot = runner.snapshot().unwrap();
    assert_eq!(snapshot["display"]["title"], "Restart?");
    assert_dialog_lines_fit(&snapshot);
    snapshot["display"]["lines"]
        .as_array()
        .unwrap()
        .iter()
        .map(|line| line.as_str().unwrap().to_string())
        .collect()
}

fn count(effects: &[RuntimePlatformEffect], matches: fn(&RuntimePlatformEffect) -> bool) -> usize {
    effects.iter().filter(|effect| matches(effect)).count()
}

#[test]
fn perf_mode_save_offers_app_restart_through_recovery_lifecycle() {
    let mut runner = changed_optimization_runner(false);
    let _ = commit_with_main(&mut runner);
    let _ = choose_save_everything(&mut runner);
    let _ = identified_save_result(&mut runner, "save-1", true);
    assert_eq!(
        restart_dialog_lines(&runner),
        [
            "System saved.",
            "Restart the app",
            "to apply?",
            "> Continue",
            "  Restart now"
        ]
    );

    let effects = confirm_restart_now(&mut runner);
    assert_eq!(
        count(&effects, |effect| matches!(
            effect,
            RuntimePlatformEffect::StoreSaveRecovery { .. }
        )),
        1
    );
    assert_eq!(
        count(&effects, |effect| matches!(
            effect,
            RuntimePlatformEffect::RestartApp
        )),
        1
    );
    assert_eq!(
        count(&effects, |effect| matches!(
            effect,
            RuntimePlatformEffect::Reboot | RuntimePlatformEffect::ApplyDeviceConfigReboot { .. }
        )),
        0
    );
    assert_eq!(runner.snapshot().unwrap()["display"]["toast"], "Restarting");
}

#[test]
fn perf_mode_saved_with_another_restart_setting_still_reboots() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        audio_optimization_capacity_available: true,
        jack_audio_required: true,
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.auto_save_default = false;
    for key in ["audioOutputs.hdmi", "sound.optimizeFor"] {
        assert!(runner.menu.focus_item_key(key));
        let _ = commit_with_main(&mut runner);
        turn_confirm(&mut runner, 1);
        let _ = commit_with_main(&mut runner);
        assert_eq!(
            runner.snapshot().unwrap()["display"]["title"],
            "Apply System"
        );
        if key == "audioOutputs.hdmi" {
            let _ = commit_with_back(&mut runner);
        }
    }
    let _ = choose_save_everything(&mut runner);
    let _ = identified_save_result(&mut runner, "save-1", true);
    assert_eq!(
        restart_dialog_lines(&runner),
        [
            "System saved.",
            "Reboot to apply?",
            "> Continue",
            "  Reboot now"
        ]
    );

    let effects = confirm_restart_now(&mut runner);
    assert_eq!(
        count(&effects, |effect| matches!(
            effect,
            RuntimePlatformEffect::Reboot
        )),
        1
    );
    assert_eq!(
        count(&effects, |effect| matches!(
            effect,
            RuntimePlatformEffect::RestartApp
        )),
        0
    );
}

#[test]
fn continue_after_successful_save_does_not_reboot() {
    let mut runner = changed_optimization_runner(false);
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
            if effects.iter().any(|effect| matches!(effect, RuntimePlatformEffect::Reboot | RuntimePlatformEffect::RestartApp))
    )));
}
