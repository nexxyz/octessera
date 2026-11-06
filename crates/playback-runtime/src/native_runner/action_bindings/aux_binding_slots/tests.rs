use crate::native_menu::NativeMenuAction;
use crate::native_runner::{NativeRunner, NativeRunnerConfig};
use crate::{CoreRunner, HostMessage, RunnerMessage, RuntimeAudioCommand};
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
fn normal_and_shift_aux_sides_with_system_targets_dirty_only_the_patch() {
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
    assert!(runner.pending.pending_autosave_payload_due_at.is_some());
    assert!(runner.pending.system_persistence.dirty_revision.is_none());
}

#[test]
fn aux_turn_moves_between_musical_and_system_targets_without_a_save() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    for key in ["transport.bpm", "displayBrightness", "transport.bpm"] {
        bind_current_to_aux(&mut runner, key, false);
        assert_eq!(
            runner.aux_bindings[0]
                .as_ref()
                .and_then(|binding| binding.turn_key.as_deref()),
            Some(key)
        );
    }
    clear_aux_turn(&mut runner, false);
    assert!(runner.aux_bindings[0].is_none());
    bind_current_to_aux(&mut runner, "displayBrightness", false);
    assert_eq!(
        runner.aux_bindings[0]
            .as_ref()
            .and_then(|binding| binding.turn_key.as_deref()),
        Some("displayBrightness")
    );
    assert!(runner.pending.system_persistence.dirty_revision.is_none());
}
