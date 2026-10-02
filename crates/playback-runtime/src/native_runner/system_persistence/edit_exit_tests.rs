use super::super::{NativeRunner, NativeRunnerConfig};
use super::SystemPersistenceState;
use crate::{
    CoreRunner, HostMessage, RunnerMessage, RuntimePlatformEffect, RuntimePlatformRequest,
    RuntimeStoreResult,
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

fn midi_status(runner: &mut NativeRunner) {
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
}

fn system_saves(messages: &[RunnerMessage]) -> Vec<Value> {
    messages
        .iter()
        .flat_map(|message| match message {
            RunnerMessage::PlatformEffects { effects } => effects.iter().collect::<Vec<_>>(),
            _ => Vec::new(),
        })
        .filter_map(|effect| match effect {
            RuntimePlatformEffect::StoreSaveSystem { payload } => Some(payload.clone()),
            _ => None,
        })
        .collect()
}

fn edit_brightness(runner: &mut NativeRunner) -> Vec<RunnerMessage> {
    assert!(runner.menu.focus_item_key("displayBrightness"));
    let _ = input(runner, json!({ "type": "encoder_press", "id": "main" }));
    let turn = input(
        runner,
        json!({ "type": "encoder_turn", "id": "main", "delta": 1 }),
    );
    assert!(system_saves(&turn).is_empty());
    input(runner, json!({ "type": "encoder_press", "id": "main" }))
}

#[test]
fn changed_main_and_back_edits_save_once_only_after_exit() {
    let mut main = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    assert!(main.menu.focus_item_key("displayBrightness"));
    let _ = input(&mut main, json!({ "type": "encoder_press", "id": "main" }));
    for _ in 0..3 {
        let turn = input(
            &mut main,
            json!({ "type": "encoder_turn", "id": "main", "delta": 1 }),
        );
        assert!(system_saves(&turn).is_empty());
    }
    let exit = input(&mut main, json!({ "type": "encoder_press", "id": "main" }));
    let saves = system_saves(&exit);
    assert_eq!(saves.len(), 1);
    assert_eq!(saves[0]["kind"], "octessera.system");

    let mut back = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    assert!(back.menu.focus_item_key("displayBrightness"));
    let _ = input(&mut back, json!({ "type": "encoder_press", "id": "main" }));
    let _ = input(
        &mut back,
        json!({ "type": "encoder_turn", "id": "main", "delta": 1 }),
    );
    let exit = input(&mut back, json!({ "type": "button_a", "pressed": true }));
    assert_eq!(system_saves(&exit).len(), 1);
}

#[test]
fn unchanged_or_reverted_edits_do_not_save() {
    let mut unchanged = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    assert!(unchanged.menu.focus_item_key("displayBrightness"));
    let _ = input(
        &mut unchanged,
        json!({ "type": "encoder_press", "id": "main" }),
    );
    assert!(system_saves(&input(
        &mut unchanged,
        json!({ "type": "encoder_press", "id": "main" })
    ))
    .is_empty());

    let mut reverted = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    assert!(reverted.menu.focus_item_key("displayBrightness"));
    let _ = input(
        &mut reverted,
        json!({ "type": "encoder_press", "id": "main" }),
    );
    for delta in [1, -1] {
        let _ = input(
            &mut reverted,
            json!({ "type": "encoder_turn", "id": "main", "delta": delta }),
        );
    }
    assert!(system_saves(&input(
        &mut reverted,
        json!({ "type": "encoder_press", "id": "main" })
    ))
    .is_empty());
}

#[test]
fn patch_only_parameter_edit_does_not_save_system() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    assert!(runner.menu.focus_item_key("sound.noteLengthMs"));
    let _ = input(
        &mut runner,
        json!({ "type": "encoder_press", "id": "main" }),
    );
    let _ = input(
        &mut runner,
        json!({ "type": "encoder_turn", "id": "main", "delta": 1 }),
    );
    let exit = input(
        &mut runner,
        json!({ "type": "encoder_press", "id": "main" }),
    );
    assert!(system_saves(&exit).is_empty());
}

#[test]
fn unrelated_midi_status_during_patch_edit_does_not_save_system() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    assert!(runner.menu.focus_item_key("sound.noteLengthMs"));
    let _ = input(
        &mut runner,
        json!({ "type": "encoder_press", "id": "main" }),
    );
    midi_status(&mut runner);
    let exit = input(
        &mut runner,
        json!({ "type": "encoder_press", "id": "main" }),
    );
    assert!(system_saves(&exit).is_empty());
}

#[test]
fn unrelated_midi_status_during_unchanged_system_edit_does_not_save() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    assert!(runner.menu.focus_item_key("displayBrightness"));
    let _ = input(
        &mut runner,
        json!({ "type": "encoder_press", "id": "main" }),
    );
    midi_status(&mut runner);
    let exit = input(
        &mut runner,
        json!({ "type": "encoder_press", "id": "main" }),
    );
    assert!(system_saves(&exit).is_empty());
}

#[test]
fn cancelled_restart_edit_is_masked_from_following_ordinary_save() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        audio_optimization_capacity_available: true,
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    assert!(runner.menu.focus_item_key("sound.optimizeFor"));
    let baseline = SystemPersistenceState::system_document(&runner).unwrap();
    let _ = input(
        &mut runner,
        json!({ "type": "encoder_press", "id": "main" }),
    );
    let _ = input(
        &mut runner,
        json!({ "type": "encoder_turn", "id": "main", "delta": 1 }),
    );
    assert!(system_saves(&input(
        &mut runner,
        json!({ "type": "encoder_press", "id": "main" })
    ))
    .is_empty());
    runner.cancel_restart_flow();

    let payload = system_saves(&edit_brightness(&mut runner)).remove(0);
    assert_eq!(
        payload["runtimeConfig"]["sound"]["optimizeFor"],
        baseline["runtimeConfig"]["sound"]["optimizeFor"]
    );
}

#[test]
fn automatic_save_failure_stays_visible_and_keeps_system_dirty() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let baseline = runner.pending.system_persistence.saved_baseline.clone();
    let payload = system_saves(&edit_brightness(&mut runner)).remove(0);
    let dirty_revision = runner.pending.system_persistence.dirty_revision;
    let request = RuntimePlatformRequest::new(
        RuntimePlatformEffect::StoreSaveSystem { payload },
        "auto-save-failure".into(),
        None,
    );
    runner.register_platform_request(&request);
    let result = runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::SaveSystemResult { ok: false }
                .with_identity(request.request_id, request.revision),
        })
        .unwrap();

    assert_eq!(
        runner.pending.system_persistence.dirty_revision,
        dirty_revision
    );
    assert_eq!(runner.pending.system_persistence.saved_baseline, baseline);
    assert!(runner.display.runtime_error_presentation.is_some());
    assert!(result
        .iter()
        .any(|message| matches!(message, RunnerMessage::Snapshot { .. })));
}

#[test]
fn pending_write_sends_latest_completed_payload_not_later_unfinished_edit() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let first_payload = system_saves(&edit_brightness(&mut runner)).remove(0);
    let first_request = RuntimePlatformRequest::new(
        RuntimePlatformEffect::StoreSaveSystem {
            payload: first_payload,
        },
        "system-a".into(),
        None,
    );
    runner.register_platform_request(&first_request);

    assert!(system_saves(&edit_brightness(&mut runner)).is_empty());
    let second_payload = runner
        .pending
        .system_persistence
        .waiting_auto_save
        .as_ref()
        .unwrap()
        .payload
        .clone();
    let _ = input(
        &mut runner,
        json!({ "type": "encoder_press", "id": "main" }),
    );
    let _ = input(
        &mut runner,
        json!({ "type": "encoder_turn", "id": "main", "delta": 1 }),
    );
    let ack = runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::SaveSystemResult { ok: true }
                .with_identity(first_request.request_id.clone(), first_request.revision),
        })
        .unwrap();
    assert!(ack.iter().all(|message| match message {
        RunnerMessage::Snapshot { snapshot } => snapshot["display"]["toast"] != "System saved",
        _ => true,
    }));
    let dispatched = system_saves(&ack).remove(0);
    assert_eq!(dispatched, second_payload);
    let second_request = RuntimePlatformRequest::new(
        RuntimePlatformEffect::StoreSaveSystem {
            payload: dispatched,
        },
        "system-b".into(),
        None,
    );
    runner.register_platform_request(&second_request);
    assert_ne!(
        runner
            .pending
            .system_persistence
            .pending_request
            .as_ref()
            .and_then(|pending| pending.captured_system_dirty_revision),
        runner.pending.system_persistence.dirty_revision
    );
    let _ = input(
        &mut runner,
        json!({ "type": "encoder_press", "id": "main" }),
    );
    let ack = runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::SaveSystemResult { ok: true }
                .with_identity(second_request.request_id.clone(), second_request.revision),
        })
        .unwrap();
    assert_eq!(system_saves(&ack).len(), 1);
    assert!(ack.iter().all(|message| match message {
        RunnerMessage::Snapshot { snapshot } => snapshot["display"]["toast"] != "System saved",
        _ => true,
    }));
}
