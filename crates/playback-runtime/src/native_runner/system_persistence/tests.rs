use super::*;
use crate::native_runner::{NativeRunner, NativeRunnerConfig};
use crate::{
    split_system_patch_documents, CoreRunner, HostMessage, RuntimePlatformEffect,
    RuntimePlatformRequest, RuntimeStoreResult,
};
use serde_json::json;
use serde_json::Value;

fn runner() -> NativeRunner {
    NativeRunner::new(NativeRunnerConfig::default()).unwrap()
}

#[test]
fn system_document_matches_the_device_projection_for_mixed_runtime_config() {
    let mut runner = runner();
    runner.display.ui.master_volume = 37;
    runner.display.ui.display_brightness = 61;
    runner.display.ui.numeric_display_mode = "hex".into();
    runner.sample_favourite_dirs = vec!["/samples/favourites".into()];
    runner.auto_save_default = true;
    runner.rolling_backups = false;
    runner.midi_enabled = true;
    runner.selected_midi_output_id = Some("midi-out-2".into());
    runner.selected_midi_input_id = Some("midi-in-3".into());
    runner.audio_output_buffer_frames = 256;
    runner.recording_max_minutes = 23;
    runner.aux_bindings[0] = Some(crate::native_runner::NativeAuxBinding {
        turn_key: Some("displayBrightness".into()),
        press_action: Some(crate::native_menu::NativeMenuAction::PlatformEffect(
            "midi.panic".into(),
        )),
    });
    runner.aux_bindings[1] = Some(crate::native_runner::NativeAuxBinding {
        turn_key: Some("layers.0.algorithmStep".into()),
        press_action: Some(crate::native_menu::NativeMenuAction::PlatformEffect(
            "midi.panic".into(),
        )),
    });
    runner.shift_aux_bindings[0] = Some(crate::native_runner::NativeAuxBinding {
        turn_key: Some("masterVolume".into()),
        press_action: Some(crate::native_menu::NativeMenuAction::PlatformEffect(
            "preset.load".into(),
        )),
    });
    runner.shift_aux_bindings[1] = Some(crate::native_runner::NativeAuxBinding {
        turn_key: Some("instruments.0.synth.osc1.levelPct".into()),
        press_action: Some(crate::native_menu::NativeMenuAction::PlatformEffect(
            "midi.panic".into(),
        )),
    });

    let expected =
        super::super::device_config_payload_from_payload(runner.config_payload()).unwrap();
    runner.behavior_state_serialization_calls.set(0);

    let actual = SystemPersistenceState::system_document(&runner).unwrap();

    assert_eq!(
        actual,
        json!({
            "kind": "octessera.system",
            "schemaVersion": 1,
            "runtimeConfig": expected["runtimeConfig"],
        })
    );
    assert_eq!(runner.behavior_state_serialization_calls.get(), 0);
    assert_eq!(
        actual["runtimeConfig"]["auxBindings"]["aux1"]["turnKey"],
        "displayBrightness"
    );
    assert_eq!(
        actual["runtimeConfig"]["auxBindings"]["aux1"]["pressAction"]["action"],
        "midi.panic"
    );
    assert_eq!(
        actual["runtimeConfig"]["auxBindings"]["aux2"]["turnKey"],
        Value::Null
    );
    assert_eq!(
        actual["runtimeConfig"]["auxBindings"]["aux2"]["pressAction"]["action"],
        "midi.panic"
    );
    assert_eq!(
        actual["runtimeConfig"]["shiftAuxBindings"]["aux1"]["turnKey"],
        "masterVolume"
    );
    assert_eq!(
        actual["runtimeConfig"]["shiftAuxBindings"]["aux1"]["pressAction"],
        Value::Null
    );
    assert_eq!(
        actual["runtimeConfig"]["shiftAuxBindings"]["aux2"]["turnKey"],
        Value::Null
    );
    assert_eq!(
        actual["runtimeConfig"]["shiftAuxBindings"]["aux2"]["pressAction"]["action"],
        "midi.panic"
    );
}

#[test]
fn system_document_does_not_serialize_musical_behavior_state() {
    let runner = runner();
    runner.config_payload();
    assert!(runner.behavior_state_serialization_calls.get() > 0);
    runner.behavior_state_serialization_calls.set(0);

    SystemPersistenceState::system_document(&runner).unwrap();

    assert_eq!(runner.behavior_state_serialization_calls.get(), 0);
}

fn mark_patch_revision(runner: &mut NativeRunner, revision: u64) {
    while runner.config_revision < revision {
        runner.mark_config_dirty();
    }
}

fn register_default_save(runner: &mut NativeRunner, request_id: &str) -> u64 {
    let revision = runner.config_revision;
    let effect = runner
        .platform_effect_for_action("default.save")
        .unwrap()
        .unwrap();
    runner.register_platform_request(&RuntimePlatformRequest::new(
        effect,
        request_id.into(),
        None,
    ));
    revision
}

fn complete_default_save(runner: &mut NativeRunner, request_id: &str, _revision: u64) {
    runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::SaveDefaultResult {
                ok: true,
                is_auto: None,
            }
            .with_identity(request_id.into(), None),
        })
        .unwrap();
}

fn register_system_save(
    runner: &mut NativeRunner,
    request_id: &str,
    revision: Option<u64>,
    payload: Value,
) -> RuntimePlatformRequest {
    let request = RuntimePlatformRequest::new(
        RuntimePlatformEffect::StoreSaveSystem { payload },
        request_id.into(),
        revision,
    );
    runner.register_platform_request(&request);
    request
}

#[test]
fn default_ack_uses_captured_patch_token_after_system_advances_global_revision() {
    let mut runner = runner();
    mark_patch_revision(&mut runner, 10);
    runner.mark_system_dirty();
    assert_eq!(runner.config_revision, 11);
    assert_eq!(runner.dirty_revision, Some(10));
    assert_eq!(runner.pending.system_persistence.dirty_revision, Some(11));

    let revision = register_default_save(&mut runner, "mixed-save-11");
    assert_eq!(revision, 11);
    complete_default_save(&mut runner, "mixed-save-11", revision);

    assert!(!runner.config_dirty);
    assert_eq!(runner.dirty_revision, None);
    assert_eq!(runner.pending.system_persistence.dirty_revision, Some(11));
}

#[test]
fn default_ack_does_not_clear_a_patch_edit_newer_than_the_captured_token() {
    let mut runner = runner();
    mark_patch_revision(&mut runner, 10);
    runner.mark_system_dirty();
    let revision = register_default_save(&mut runner, "mixed-save-11");
    runner.mark_config_dirty();
    assert_eq!(runner.dirty_revision, Some(12));

    complete_default_save(&mut runner, "mixed-save-11", revision);

    assert!(runner.config_dirty);
    assert_eq!(runner.dirty_revision, Some(12));
    assert_eq!(runner.pending.system_persistence.dirty_revision, Some(11));
}

#[test]
fn system_save_ack_uses_submitted_payload_and_keeps_a_newer_system_edit_dirty() {
    let mut runner = runner();
    runner.mark_system_dirty();
    assert_eq!(runner.config_revision, 1);
    assert!(runner.dirty_revision.is_none());
    assert!(!runner.config_dirty);
    assert!(runner.pending.pending_autosave_payload_due_at.is_none());
    let submitted =
        json!({ "kind": "octessera.system", "runtimeConfig": { "displayBrightness": 31 } });
    let mut request = register_system_save(&mut runner, "system-save-1", None, submitted.clone());
    if let RuntimePlatformEffect::StoreSaveSystem { payload } = &mut request.effect {
        *payload = json!({ "kind": "mutated-after-submit" });
    }
    runner.mark_system_dirty();
    let later_system_revision = runner.pending.system_persistence.dirty_revision;

    runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::SaveSystemResult { ok: true }
                .with_identity("system-save-1".into(), None),
        })
        .unwrap();

    assert_eq!(
        runner.pending.system_persistence.saved_baseline.as_deref(),
        Some(&submitted)
    );
    assert_eq!(
        runner.pending.system_persistence.dirty_revision,
        later_system_revision
    );
    assert_eq!(runner.dirty_revision, None);
    assert!(!runner.config_dirty);
}

#[test]
fn system_save_failure_keeps_system_baseline_and_dirty_revision() {
    let mut runner = runner();
    runner.pending.system_persistence.saved_baseline = Some(Arc::new(json!({ "saved": "before" })));
    runner.mark_system_dirty();
    let dirty_revision = runner.pending.system_persistence.dirty_revision;
    let _request = register_system_save(
        &mut runner,
        "system-save-fail",
        None,
        json!({ "submitted": true }),
    );

    runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::SaveSystemResult { ok: false }
                .with_identity("system-save-fail".into(), None),
        })
        .unwrap();

    assert_eq!(
        runner.pending.system_persistence.saved_baseline.as_deref(),
        Some(&json!({ "saved": "before" }))
    );
    assert_eq!(
        runner.pending.system_persistence.dirty_revision,
        dirty_revision
    );
}

#[test]
fn system_load_resets_only_system_baseline_and_dirty_state() {
    let mut runner = runner();
    mark_patch_revision(&mut runner, 8);
    runner.pending.pending_save_revision = Some(8);
    let autosave_due = std::time::Instant::now();
    runner.pending.pending_autosave_payload_due_at = Some(autosave_due);
    runner.mark_system_dirty();
    let restart_baseline = runner.restart_settings.persisted_default.clone();
    let patch_dirty_revision = runner.dirty_revision;
    let mut system = split_system_patch_documents(&runner.config_payload())
        .unwrap()
        .system;
    system["runtimeConfig"]["displayBrightness"] = json!(42);
    let request = RuntimePlatformRequest::new(
        RuntimePlatformEffect::StoreLoadSystem,
        "system-load-1".into(),
        Some(44),
    );
    runner.register_platform_request(&request);

    runner
        .apply_store_result(
            RuntimeStoreResult::LoadSystemResult {
                payload: Some(system.clone()),
            }
            .with_identity("system-load-1".into(), Some(44)),
        )
        .unwrap();

    assert_eq!(runner.pending.system_persistence.dirty_revision, None);
    assert_eq!(
        runner.pending.system_persistence.saved_baseline.as_deref(),
        Some(&system)
    );
    assert_eq!(runner.dirty_revision, patch_dirty_revision);
    assert!(runner.config_dirty);
    assert_eq!(runner.pending.pending_save_revision, Some(8));
    assert_eq!(
        runner.pending.pending_autosave_payload_due_at,
        Some(autosave_due)
    );
    assert_eq!(&*runner.restart_settings.persisted_default, &system);
    assert_ne!(
        &*runner.restart_settings.persisted_default,
        &*restart_baseline
    );
}

#[test]
fn stale_system_save_ack_does_not_change_domain_baseline_or_dirty_token() {
    let mut runner = runner();
    runner.mark_system_dirty();
    let saved = json!({ "saved": "baseline" });
    runner.pending.system_persistence.saved_baseline = Some(Arc::new(saved.clone()));
    let request = register_system_save(
        &mut runner,
        "system-current",
        Some(7),
        json!({ "submitted": "current" }),
    );
    let dirty_revision = runner.pending.system_persistence.dirty_revision;

    for (request_id, revision) in [("system-stale", Some(7)), ("system-current", Some(8))] {
        runner
            .send(HostMessage::RuntimeResult {
                result: RuntimeStoreResult::SaveSystemResult { ok: true }
                    .with_identity(request_id.into(), revision),
            })
            .unwrap();
    }

    assert_eq!(
        runner.pending.system_persistence.saved_baseline.as_deref(),
        Some(&saved)
    );
    assert_eq!(
        runner.pending.system_persistence.dirty_revision,
        dirty_revision
    );

    let submitted = match request.effect {
        RuntimePlatformEffect::StoreSaveSystem { payload } => payload,
        _ => unreachable!(),
    };
    runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::SaveSystemResult { ok: true }
                .with_identity("system-current".into(), Some(7)),
        })
        .unwrap();
    assert_eq!(
        runner.pending.system_persistence.saved_baseline.as_deref(),
        Some(&submitted)
    );
    assert_eq!(runner.pending.system_persistence.dirty_revision, None);
    runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::SaveSystemResult { ok: true }
                .with_identity("system-current".into(), Some(7)),
        })
        .unwrap();
    assert_eq!(
        runner.pending.system_persistence.saved_baseline.as_deref(),
        Some(&submitted)
    );
}
