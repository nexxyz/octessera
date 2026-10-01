use super::super::{NativeRunner, NativeRunnerConfig};
use crate::{
    CoreRunner, HostMessage, RunnerMessage, RuntimePlatformEffect, RuntimePlatformRequest,
    RuntimeStoreResult,
};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

fn input(runner: &mut NativeRunner, input: Value) -> Vec<RunnerMessage> {
    runner
        .send(HostMessage::DeviceInput {
            input,
            request_snapshot: None,
        })
        .unwrap()
}

fn confirm(runner: &mut NativeRunner, action_key: &str, title: &str) -> Vec<RunnerMessage> {
    assert!(runner.menu.focus_item_key(action_key));
    let opened = input(runner, json!({ "type": "encoder_press", "id": "main" }));
    let snapshot = super::super::tests::snapshot_from(&opened);
    assert_eq!(snapshot["display"]["title"], title);
    assert!(snapshot["display"]["lines"]
        .as_array()
        .unwrap()
        .iter()
        .any(|line| line == "> Cancel"));
    let _ = input(
        runner,
        json!({ "type": "encoder_turn", "id": "main", "delta": 1 }),
    );
    input(runner, json!({ "type": "encoder_press", "id": "main" }))
}

fn start_pending_patch_save(runner: &mut NativeRunner, request_id: &str) -> u64 {
    runner.mark_config_dirty();
    let revision = runner.dirty_revision.unwrap();
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
    runner.pending.pending_save_revision = Some(revision);
    revision
}

#[test]
fn confirmed_system_save_updates_system_baseline_without_acknowledging_patch_save() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let patch_revision = start_pending_patch_save(&mut runner, "pending-patch-save");
    runner.mark_system_dirty();
    let system_dirty_revision = runner.pending.system_persistence.dirty_revision;

    let messages = confirm(&mut runner, "system.save", "Confirm System");
    let (effect, payload) = messages
        .iter()
        .find_map(|message| match message {
            RunnerMessage::PlatformEffects { effects } => effects.iter().find_map(|effect| {
                let RuntimePlatformEffect::StoreSaveSystem { payload } = effect else {
                    return None;
                };
                Some((effect.clone(), payload.clone()))
            }),
            _ => None,
        })
        .expect("confirmed System Save effect");
    let request = RuntimePlatformRequest::new(effect, "system-save-ui".into(), None);
    runner.register_platform_request(&request);
    runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::SaveSystemResult { ok: true }
                .with_identity(request.request_id.clone(), request.revision),
        })
        .unwrap();

    assert_eq!(
        runner.pending.system_persistence.saved_baseline.as_deref(),
        Some(&payload)
    );
    assert_eq!(runner.pending.system_persistence.dirty_revision, None);
    assert!(runner.config_dirty);
    assert_eq!(runner.dirty_revision, Some(patch_revision));
    assert_eq!(runner.pending.pending_save_revision, Some(patch_revision));
    assert_eq!(
        runner.pending_default_write_revision(),
        Some(patch_revision)
    );
    assert!(system_dirty_revision.is_some());
}

#[test]
fn confirmed_system_load_runs_while_patch_save_is_pending_and_preserves_its_debounce() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let patch_request_revision = start_pending_patch_save(&mut runner, "pending-patch-save");
    runner.mark_fast_autosave_dirty();
    let patch_dirty_revision = runner.dirty_revision;
    runner.mark_system_dirty();
    let due = Instant::now() + Duration::from_secs(30);
    runner.pending.pending_autosave_payload_due_at = Some(due);
    let patch = runner.patch_payload().unwrap();
    let mut full = runner.config_payload();
    full["runtimeConfig"]["displayBrightness"] = json!(46);
    let system = crate::split_system_patch_documents(&full).unwrap().system;

    let messages = confirm(&mut runner, "system.load", "Confirm System");
    assert!(messages.iter().any(|message| matches!(message,
        RunnerMessage::PlatformEffects { effects }
            if effects.contains(&RuntimePlatformEffect::StoreLoadSystem)
    )));
    let request = RuntimePlatformRequest::new(
        RuntimePlatformEffect::StoreLoadSystem,
        "system-load-ui".into(),
        None,
    );
    runner.register_platform_request(&request);
    runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::LoadSystemResult {
                payload: Some(system.clone()),
            }
            .with_identity(request.request_id, request.revision),
        })
        .unwrap();

    assert_eq!(runner.display.ui.display_brightness, 46);
    assert_eq!(
        runner.pending.system_persistence.saved_baseline.as_deref(),
        Some(&system)
    );
    assert_eq!(runner.patch_payload().unwrap(), patch);
    assert!(runner.config_dirty);
    assert_eq!(runner.dirty_revision, patch_dirty_revision);
    assert_eq!(
        runner.pending.pending_save_revision,
        Some(patch_request_revision)
    );
    assert_eq!(runner.pending.pending_autosave_payload_due_at, Some(due));
    assert_eq!(
        runner.pending_default_write_revision(),
        Some(patch_request_revision)
    );
}
