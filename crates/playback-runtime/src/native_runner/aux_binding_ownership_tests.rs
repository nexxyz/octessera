use crate::native_menu::NativeMenuAction;
use crate::native_runner::{NativeRunner, NativeRunnerConfig};
use crate::{
    CoreRunner, HostMessage, RunnerMessage, RuntimeAudioCommand, RuntimePlatformEffect,
    RuntimePlatformRequest, RuntimeStoreResult,
};
use serde_json::{json, Value};

fn input(runner: &mut NativeRunner, input: Value) -> Vec<RunnerMessage> {
    runner
        .send(HostMessage::DeviceInput {
            input,
            request_snapshot: None,
        })
        .unwrap()
}

fn bind_current_to_aux(runner: &mut NativeRunner, key: &str, shifted: bool) {
    assert!(runner.menu.focus_item_key(key));
    if shifted {
        let _ = input(runner, json!({ "type": "button_shift", "pressed": true }));
    }
    let _ = input(runner, json!({ "type": "button_fn", "pressed": true }));
    let _ = input(runner, json!({ "type": "encoder_press", "id": "aux1" }));
    let _ = input(runner, json!({ "type": "button_fn", "pressed": false }));
    if shifted {
        let _ = input(runner, json!({ "type": "button_shift", "pressed": false }));
    }
}

fn clear_aux_turn(runner: &mut NativeRunner, shifted: bool) {
    let key = if shifted {
        "shiftAux:0:turn.none"
    } else {
        "aux:0:turn.none"
    };
    assert!(runner.menu.focus_item_key(key));
    let _ = input(runner, json!({ "type": "encoder_press", "id": "main" }));
}

fn complete_patch_save(runner: &mut NativeRunner, request_id: &str) {
    let effect = runner
        .platform_effect_for_action("default.save")
        .unwrap()
        .unwrap();
    assert!(
        matches!(&effect, RuntimePlatformEffect::StoreSaveDefault { payload, .. }
        if payload["kind"] == "octessera.patch" && payload["schemaVersion"] == 2)
    );
    runner.register_platform_request(&RuntimePlatformRequest::new(
        effect,
        request_id.into(),
        None,
    ));
    let _ = runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::SaveDefaultResult {
                ok: true,
                is_auto: None,
            }
            .with_identity(request_id.into(), None),
        })
        .unwrap();
}

fn complete_system_save(runner: &mut NativeRunner, request_id: &str) {
    let effect = runner
        .platform_effect_for_action("system.save")
        .unwrap()
        .unwrap();
    assert!(
        matches!(&effect, RuntimePlatformEffect::StoreSaveSystem { payload }
        if payload["kind"] == "octessera.system" && payload["schemaVersion"] == 1)
    );
    runner.register_platform_request(&RuntimePlatformRequest::new(
        effect,
        request_id.into(),
        None,
    ));
    let _ = runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::SaveSystemResult { ok: true }
                .with_identity(request_id.into(), None),
        })
        .unwrap();
}

#[test]
fn device_menu_edits_dirty_only_their_domain_and_preserve_system_backups_policy() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let revision = runner.config_revision;
    assert!(runner.menu.focus_item_key("masterVolume"));
    let _ = input(
        &mut runner,
        json!({ "type": "encoder_press", "id": "main" }),
    );
    let audio = input(
        &mut runner,
        json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
    );
    let _ = input(
        &mut runner,
        json!({ "type": "encoder_press", "id": "main" }),
    );
    assert!(!runner.config_dirty);
    assert!(runner.dirty_revision.is_none());
    assert_eq!(
        runner.pending.system_persistence.dirty_revision,
        Some(revision + 1)
    );
    assert!(runner.pending.pending_autosave_payload_due_at.is_none());
    assert!(audio.iter().any(|message| matches!(message,
        RunnerMessage::AudioCommands { commands }
            if commands.iter().any(|command| matches!(command,
                RuntimeAudioCommand::SetMasterVolume { .. }
            ))
    )));

    let mut patch = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    assert!(patch.menu.focus_item_key("transport.bpm"));
    let _ = input(&mut patch, json!({ "type": "encoder_press", "id": "main" }));
    let _ = input(
        &mut patch,
        json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
    );
    let _ = input(&mut patch, json!({ "type": "encoder_press", "id": "main" }));
    assert!(patch.config_dirty);
    assert_eq!(patch.dirty_revision, Some(patch.config_revision));
    assert!(patch.pending.system_persistence.dirty_revision.is_none());
    assert!(patch.pending.pending_autosave_payload_due_at.is_some());

    let due = patch.pending.pending_autosave_payload_due_at;
    assert!(patch.menu.focus_item_key("autoSaveDefault"));
    let previous_auto_save = patch.auto_save_default;
    let _ = input(&mut patch, json!({ "type": "encoder_press", "id": "main" }));
    let _ = input(
        &mut patch,
        json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
    );
    let _ = input(&mut patch, json!({ "type": "encoder_press", "id": "main" }));
    assert_ne!(patch.auto_save_default, previous_auto_save);
    assert_eq!(patch.pending.pending_autosave_payload_due_at, due);
    assert!(patch.config_dirty);
    assert_eq!(
        patch.pending.system_persistence.dirty_revision,
        Some(patch.config_revision)
    );
}

#[test]
fn normal_and_shift_aux_sides_can_be_mixed_between_system_and_patch() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    bind_current_to_aux(&mut runner, "transport.bpm", false);
    bind_current_to_aux(&mut runner, "system.info", false);
    bind_current_to_aux(&mut runner, "displayBrightness", true);
    bind_current_to_aux(&mut runner, "system.clearAll", true);

    assert_eq!(
        runner.aux_bindings[0]
            .as_ref()
            .and_then(|binding| binding.turn_key.as_deref()),
        Some("transport.bpm")
    );
    assert_eq!(
        runner.aux_bindings[0]
            .as_ref()
            .and_then(|binding| binding.press_action.as_ref()),
        Some(&NativeMenuAction::PlatformEffect("system.info".into()))
    );
    assert_eq!(
        runner.shift_aux_bindings[0]
            .as_ref()
            .and_then(|binding| binding.turn_key.as_deref()),
        Some("displayBrightness")
    );
    assert_eq!(
        runner.shift_aux_bindings[0]
            .as_ref()
            .and_then(|binding| binding.press_action.as_ref()),
        Some(&NativeMenuAction::PlatformEffect("system.clearAll".into()))
    );
    assert!(runner.config_dirty);
    assert!(runner.pending.system_persistence.dirty_revision.is_some());
}

#[test]
fn cross_owner_aux_turn_requires_old_document_to_be_cleared_and_saved() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    bind_current_to_aux(&mut runner, "transport.bpm", false);
    complete_patch_save(&mut runner, "aux-patch-old");
    bind_current_to_aux(&mut runner, "displayBrightness", false);
    assert_eq!(
        runner.aux_bindings[0]
            .as_ref()
            .and_then(|binding| binding.turn_key.as_deref()),
        Some("transport.bpm")
    );

    clear_aux_turn(&mut runner, false);
    bind_current_to_aux(&mut runner, "displayBrightness", false);
    assert_eq!(
        runner.aux_bindings[0]
            .as_ref()
            .and_then(|binding| binding.turn_key.as_deref()),
        None
    );
    complete_patch_save(&mut runner, "aux-patch-clear");
    bind_current_to_aux(&mut runner, "displayBrightness", false);
    assert_eq!(
        runner.aux_bindings[0]
            .as_ref()
            .and_then(|binding| binding.turn_key.as_deref()),
        Some("displayBrightness")
    );
    complete_system_save(&mut runner, "aux-system-old");

    bind_current_to_aux(&mut runner, "transport.bpm", false);
    assert_eq!(
        runner.aux_bindings[0]
            .as_ref()
            .and_then(|binding| binding.turn_key.as_deref()),
        Some("displayBrightness")
    );
    clear_aux_turn(&mut runner, false);
    bind_current_to_aux(&mut runner, "transport.bpm", false);
    assert_eq!(
        runner.aux_bindings[0]
            .as_ref()
            .and_then(|binding| binding.turn_key.as_deref()),
        None
    );
    complete_system_save(&mut runner, "aux-system-clear");
    bind_current_to_aux(&mut runner, "transport.bpm", false);
    assert_eq!(
        runner.aux_bindings[0]
            .as_ref()
            .and_then(|binding| binding.turn_key.as_deref()),
        Some("transport.bpm")
    );
}
