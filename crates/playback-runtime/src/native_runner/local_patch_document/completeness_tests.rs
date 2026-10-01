use super::super::{
    compose_local_system_patch_documents, portable_patch_projection,
    split_local_system_patch_documents,
};
use crate::native_runner::{NativeRunner, NativeRunnerConfig};
use crate::protocol::{
    HostMessage, RunnerMessage, RuntimePlatformEffect, RuntimePlatformRequest, RuntimeStoreResult,
};
use crate::CoreRunner;
use serde_json::{json, Value};

fn local_documents() -> (Value, Value) {
    let runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let documents = split_local_system_patch_documents(&runner.config_payload()).unwrap();
    (documents.system, documents.patch)
}

fn truncate(patch: &mut Value, pointer: &str) {
    patch
        .pointer_mut(pointer)
        .and_then(Value::as_array_mut)
        .and_then(Vec::pop)
        .expect("fixed native array entry");
}

fn reject_truncated_patch(system: &Value, mut patch: Value, pointer: &str, path: &str) {
    truncate(&mut patch, pointer);
    let error = compose_local_system_patch_documents(system, &patch).unwrap_err();
    assert!(error.contains(path), "{error}");

    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].sample_paths[0] = Some("userdata/stale.wav".into());
    runner.instruments[1].sample_paths[7] = Some("sd-card/stale.wav".into());
    runner.config_dirty = true;
    let before = runner.config_payload();
    runner
        .apply_store_result(RuntimeStoreResult::LoadDefaultResult {
            payload: Some(patch),
        })
        .unwrap();

    assert_eq!(runner.config_payload(), before);
    assert!(runner.config_dirty);
    assert!(runner.display.runtime_error_presentation.is_some());
    assert_eq!(
        runner.instruments[0].sample_paths[0].as_deref(),
        Some("userdata/stale.wav")
    );
    assert_eq!(
        runner.instruments[1].sample_paths[7].as_deref(),
        Some("sd-card/stale.wav")
    );
}

#[test]
fn local_documents_reject_every_shortened_fixed_native_array_before_load() {
    let (system, patch) = local_documents();
    for (pointer, path) in [
        ("/runtimeConfig/layers", "patch.runtimeConfig.layers"),
        (
            "/runtimeConfig/instruments",
            "patch.runtimeConfig.instruments",
        ),
        ("/runtimeConfig/linkLfos", "patch.runtimeConfig.linkLfos"),
        (
            "/runtimeConfig/mixer/buses",
            "patch.runtimeConfig.mixer.buses",
        ),
        (
            "/runtimeConfig/mixer/master/slots",
            "patch.runtimeConfig.mixer.master.slots",
        ),
    ] {
        reject_truncated_patch(&system, patch.clone(), pointer, path);
    }

    for instrument in 0..platform_core::INSTRUMENT_COUNT {
        reject_truncated_patch(
            &system,
            patch.clone(),
            format!("/runtimeConfig/instruments/{instrument}/sample/slots").as_str(),
            format!("patch.runtimeConfig.instruments[{instrument}].sample.slots").as_str(),
        );
    }
    for layer in 0..platform_core::LAYER_COUNT {
        reject_truncated_patch(
            &system,
            patch.clone(),
            format!("/runtimeConfig/layers/{layer}/link/triggerProbabilityMap").as_str(),
            format!("patch.runtimeConfig.layers[{layer}].link.triggerProbabilityMap").as_str(),
        );
    }
}

fn input(runner: &mut NativeRunner, input: Value) -> Vec<RunnerMessage> {
    runner
        .send(HostMessage::DeviceInput {
            input,
            request_snapshot: None,
        })
        .unwrap()
}

#[test]
fn setting_only_system_save_remains_available_with_local_samples() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "keys".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.instruments[0].note_behavior = "hold".into();
    runner.instruments[0].sample_paths[0] = Some("userdata/User Kit/custom.wav".into());
    runner.instruments[1].sample_paths[7] = Some("sd-card/octessera/samples/kick.wav".into());
    runner.sync_engine_runtime_config();
    let _ = input(&mut runner, json!({ "type": "grid_press", "x": 2, "y": 3 }));
    runner.mark_config_dirty();
    let patch_dirty_revision = runner.dirty_revision;
    let patch = portable_patch_projection(&runner.config_payload()).unwrap();

    assert!(runner.menu.focus_item_key("sound.audioOutputBufferFrames"));
    let _ = input(
        &mut runner,
        json!({ "type": "encoder_press", "id": "main" }),
    );
    let _ = input(&mut runner, json!({ "type": "button_fn", "pressed": true }));
    let _ = input(
        &mut runner,
        json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
    );
    let _ = input(
        &mut runner,
        json!({ "type": "button_fn", "pressed": false }),
    );
    let apply = input(
        &mut runner,
        json!({ "type": "encoder_press", "id": "main" }),
    );
    let snapshot = apply.iter().find_map(|message| match message {
        RunnerMessage::Snapshot { snapshot } => Some(snapshot),
        _ => None,
    });
    assert!(snapshot.is_some_and(|snapshot| {
        snapshot["display"]["title"] == "Apply System"
            && snapshot["display"]["lines"]
                .as_array()
                .is_some_and(|lines| lines.iter().any(|line| line == "  Save this one"))
    }));

    let _ = input(
        &mut runner,
        json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
    );
    let save = input(
        &mut runner,
        json!({ "type": "encoder_press", "id": "main" }),
    );
    let effect = save
        .iter()
        .find_map(|message| match message {
            RunnerMessage::PlatformEffects { effects } => effects.iter().find_map(|effect| {
                matches!(effect, RuntimePlatformEffect::StoreSaveSystem { .. })
                    .then(|| effect.clone())
            }),
            _ => None,
        })
        .expect("System-only setting save");
    let RuntimePlatformEffect::StoreSaveSystem { payload } = &effect else {
        unreachable!()
    };
    assert_eq!(
        payload,
        &super::super::system_persistence::SystemPersistenceState::system_document(&runner)
            .unwrap()
    );
    assert!(!save.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects.iter().any(|effect| matches!(effect,
                RuntimePlatformEffect::StoreSaveDefault { .. }
                    | RuntimePlatformEffect::StoreSaveBackup { .. }))
    )));
    assert!(runner.config_dirty);
    assert_eq!(runner.dirty_revision, patch_dirty_revision);
    assert_eq!(
        portable_patch_projection(&runner.config_payload()).unwrap(),
        patch
    );

    let request = RuntimePlatformRequest::new(effect, "local-system-save".into(), None);
    runner.register_platform_request(&request);
    let _ = runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::SaveSystemResult { ok: true }
                .with_identity(request.request_id, request.revision),
        })
        .unwrap();
    assert!(runner.config_dirty);
    assert_eq!(runner.dirty_revision, patch_dirty_revision);
    assert_eq!(runner.engine.drain_held_notes(usize::MAX).len(), 1);
}
