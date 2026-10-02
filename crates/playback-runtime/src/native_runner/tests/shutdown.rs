use super::*;
use crate::RuntimePlatformRequest;

fn device_input(runner: &mut NativeRunner, input: Value) -> Vec<RunnerMessage> {
    runner
        .send(HostMessage::DeviceInput {
            input,
            request_snapshot: None,
        })
        .unwrap()
}

fn edit_brightness(runner: &mut NativeRunner, delta: i32) -> Vec<RunnerMessage> {
    assert!(runner.menu.focus_item_key("displayBrightness"));
    let _ = device_input(runner, json!({ "type": "encoder_press", "id": "main" }));
    let _ = device_input(
        runner,
        json!({ "type": "encoder_turn", "id": "main", "delta": delta }),
    );
    device_input(runner, json!({ "type": "encoder_press", "id": "main" }))
}

fn system_save_effect(messages: &[RunnerMessage]) -> RuntimePlatformEffect {
    messages
        .iter()
        .find_map(|message| match message {
            RunnerMessage::PlatformEffects { effects } => effects.iter().find_map(|effect| {
                matches!(effect, RuntimePlatformEffect::StoreSaveSystem { .. })
                    .then(|| effect.clone())
            }),
            _ => None,
        })
        .expect("System save effect")
}

fn activate_power_action(
    runner: &mut NativeRunner,
    action_key: &str,
    title: &str,
) -> Vec<RunnerMessage> {
    assert!(runner.menu.focus_item_key(action_key));
    let opened = device_input(runner, json!({ "type": "encoder_press", "id": "main" }));
    assert_eq!(snapshot_from(&opened)["display"]["title"], title);
    let _ = device_input(
        runner,
        json!({ "type": "encoder_turn", "id": "main", "delta": 1 }),
    );
    device_input(runner, json!({ "type": "encoder_press", "id": "main" }))
}

fn assert_no_terminal_power_effects(messages: &[RunnerMessage]) {
    assert!(messages.iter().all(|message| !matches!(message,
        RunnerMessage::PlatformEffects { effects }
            if effects.iter().any(|effect| matches!(effect,
                RuntimePlatformEffect::StoreSaveRecovery { .. }
                    | RuntimePlatformEffect::Shutdown
                    | RuntimePlatformEffect::Reboot
            ))
    )));
}

#[test]
pub(crate) fn system_menu_shutdown_emits_shutdown_effect_and_splash() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.display.oled_mode = NativeOledMode::Normal;
    runner.display.oled_splash_text.clear();
    runner.display.oled_splash_until = None;
    assert!(runner.menu.focus_item_key("system.shutdown"));

    let opened = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(
        snapshot_from(&opened)["display"]["title"],
        "Confirm Shutdown"
    );

    let messages = confirm_current_dialog(&mut runner);
    let display = &snapshot_from(&messages)["display"];
    assert_eq!(display["splash"], "shutdown");
    assert!(runner.display.oled_splash_until.is_some());
    assert_eq!(display["toast"], "Shutting down");
    assert_recovery_save_then_effect(&messages, RuntimePlatformEffect::Shutdown);
}

#[test]
pub(crate) fn system_menu_reboot_emits_reboot_effect_and_shutdown_splash() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.display.oled_mode = NativeOledMode::Normal;
    runner.display.oled_splash_text.clear();
    runner.display.oled_splash_until = None;
    assert!(runner.menu.focus_item_key("system.reboot"));

    let opened = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(snapshot_from(&opened)["display"]["title"], "Confirm Reboot");

    let messages = confirm_current_dialog(&mut runner);
    let display = &snapshot_from(&messages)["display"];
    assert_eq!(display["splash"], "shutdown");
    assert!(runner.display.oled_splash_until.is_some());
    assert_eq!(display["toast"], "Rebooting");
    assert_recovery_save_then_effect(&messages, RuntimePlatformEffect::Reboot);
}

#[test]
pub(crate) fn shutdown_waits_for_inflight_and_latest_completed_system_save() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let request_a = RuntimePlatformRequest::new(
        system_save_effect(&edit_brightness(&mut runner, 1)),
        "system-save-a".into(),
        None,
    );
    runner.register_platform_request(&request_a);
    assert!(edit_brightness(&mut runner, 1).iter().all(|message| !matches!(message,
        RunnerMessage::PlatformEffects { effects }
            if effects.iter().any(|effect| matches!(effect, RuntimePlatformEffect::StoreSaveSystem { .. }))
    )));

    let blocked_a = activate_power_action(&mut runner, "system.shutdown", "Confirm Shutdown");
    assert_no_terminal_power_effects(&blocked_a);
    assert_eq!(
        runner
            .display
            .toast
            .as_ref()
            .map(|toast| toast.message.as_str()),
        Some("System save pending, try again")
    );

    let ack_a = runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::SaveSystemResult { ok: true }
                .with_identity(request_a.request_id, request_a.revision),
        })
        .unwrap();
    let request_b =
        RuntimePlatformRequest::new(system_save_effect(&ack_a), "system-save-b".into(), None);
    runner.register_platform_request(&request_b);

    let blocked_b = activate_power_action(&mut runner, "system.shutdown", "Confirm Shutdown");
    assert_no_terminal_power_effects(&blocked_b);
    assert_eq!(
        runner
            .display
            .toast
            .as_ref()
            .map(|toast| toast.message.as_str()),
        Some("System save pending, try again")
    );

    runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::SaveSystemResult { ok: true }
                .with_identity(request_b.request_id, request_b.revision),
        })
        .unwrap();
    let shutdown = activate_power_action(&mut runner, "system.shutdown", "Confirm Shutdown");
    assert_recovery_save_then_effect(&shutdown, RuntimePlatformEffect::Shutdown);
}

#[test]
pub(crate) fn reverted_system_edits_clear_only_system_dirtiness_and_allow_reboot() {
    for exit_with_back in [false, true] {
        let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
        assert!(runner.pending.system_persistence.saved_baseline.is_some());
        assert!(runner.menu.focus_item_key("displayBrightness"));
        let _ = device_input(
            &mut runner,
            json!({ "type": "encoder_press", "id": "main" }),
        );
        for delta in [1, -1] {
            let _ = device_input(
                &mut runner,
                json!({ "type": "encoder_turn", "id": "main", "delta": delta }),
            );
        }
        let exit = if exit_with_back {
            device_input(&mut runner, json!({ "type": "button_a", "pressed": true }))
        } else {
            device_input(
                &mut runner,
                json!({ "type": "encoder_press", "id": "main" }),
            )
        };
        assert!(system_save_effect_optional(&exit).is_none());
        assert_eq!(runner.pending.system_persistence.dirty_revision, None);
        assert_eq!(runner.reboot_blocked_by_pending_saves(), None);

        let reboot = activate_power_action(&mut runner, "system.reboot", "Confirm Reboot");
        assert_recovery_save_then_effect(&reboot, RuntimePlatformEffect::Reboot);
    }
}

#[test]
pub(crate) fn unrelated_unsaved_system_change_keeps_reboot_blocked_after_noop_edit() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    assert!(runner.menu.focus_item_key("displayBrightness"));
    let _ = device_input(
        &mut runner,
        json!({ "type": "encoder_press", "id": "main" }),
    );
    runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::MidiStatus {
                ok: true,
                message: None,
                selected_out_id: Some("external-output".into()),
                selected_in_id: None,
            },
        })
        .unwrap();
    let exit = device_input(
        &mut runner,
        json!({ "type": "encoder_press", "id": "main" }),
    );
    assert!(system_save_effect_optional(&exit).is_none());
    assert!(runner.pending.system_persistence.dirty_revision.is_some());
    assert_eq!(
        runner.reboot_blocked_by_pending_saves(),
        Some("Save or apply System settings before reboot")
    );

    let reboot = activate_power_action(&mut runner, "system.reboot", "Confirm Reboot");
    assert_no_terminal_power_effects(&reboot);
}

fn system_save_effect_optional(messages: &[RunnerMessage]) -> Option<RuntimePlatformEffect> {
    messages.iter().find_map(|message| match message {
        RunnerMessage::PlatformEffects { effects } => effects.iter().find_map(|effect| {
            matches!(effect, RuntimePlatformEffect::StoreSaveSystem { .. }).then(|| effect.clone())
        }),
        _ => None,
    })
}

fn assert_recovery_save_then_effect(messages: &[RunnerMessage], expected: RuntimePlatformEffect) {
    let effects = messages
        .iter()
        .flat_map(|message| match message {
            RunnerMessage::PlatformEffects { effects } => effects.as_slice(),
            _ => &[],
        })
        .collect::<Vec<_>>();
    assert!(
        matches!(effects.as_slice(), [RuntimePlatformEffect::StoreSaveRecovery { payload }, effect] if payload["runtimeConfig"].is_object() && **effect == expected)
    );
}
