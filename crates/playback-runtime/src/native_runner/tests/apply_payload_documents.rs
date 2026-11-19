use super::*;

fn saved_documents() -> [Value; 3] {
    [
        include_str!("../../../../../config/generated/desktop/default.json"),
        include_str!("../../../../../config/generated/pi/default.json"),
        include_str!(
            "../../../../../apps/desktop/src-tauri/tests/e2e/fixtures/scenario_patch.json"
        ),
    ]
    .map(|text| serde_json::from_str(text).unwrap())
}

fn loaded(document: &Value) -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.apply_config_payload(document.clone()).unwrap();
    runner
}

/// Saving what a save loads gives the same document again.
fn assert_saves_are_stable() {
    for document in saved_documents() {
        let saved = loaded(&document).config_payload();
        assert_eq!(loaded(&saved).config_payload(), saved);
    }
}

fn local_patch() -> Value {
    let runner = loaded(&saved_documents()[0]);
    split_local_system_patch_documents(&runner.config_payload())
        .unwrap()
        .patch
}

fn assert_local_patch_rejected(mutate: impl FnOnce(&mut Value), label: &str) {
    let mut patch = local_patch();
    mutate(&mut patch);
    let mut runner = loaded(&saved_documents()[0]);
    let before = runner.config_payload();
    runner
        .apply_store_result(RuntimeStoreResult::LoadDefaultResult {
            payload: Some(patch),
        })
        .unwrap();
    assert_eq!(runner.config_payload(), before, "{label}");
    assert!(
        runner.display.runtime_error_presentation.is_some(),
        "{label}"
    );
}

#[test]
fn mixer_loads_save_stably_and_reject_missing_or_unknown_keys() {
    assert_saves_are_stable();
    assert_local_patch_rejected(
        |patch| {
            patch["runtimeConfig"]["mixer"]
                .as_object_mut()
                .unwrap()
                .remove("master");
        },
        "missing master",
    );
    assert_local_patch_rejected(
        |patch| patch["runtimeConfig"]["mixer"]["buses"][0]["slotGain"] = json!(1),
        "unknown bus key",
    );
}

fn assert_system_document_rejected(mutate: impl FnOnce(&mut Value), label: &str) {
    let mut runner = loaded(&saved_documents()[0]);
    let before = runner.config_payload();
    let mut system = split_local_system_patch_documents(&before).unwrap().system;
    mutate(&mut system);
    assert!(
        runner
            .apply_system_document_preserving_patch(&system)
            .is_err(),
        "{label}"
    );
    assert_eq!(runner.config_payload(), before, "{label}");
}

#[test]
fn sound_display_and_device_loads_save_stably_and_reject_missing_or_unknown_keys() {
    assert_saves_are_stable();
    assert_local_patch_rejected(
        |patch| {
            patch["runtimeConfig"]["transport"]
                .as_object_mut()
                .unwrap()
                .remove("swingA");
        },
        "missing swing",
    );
    assert_local_patch_rejected(
        |patch| patch["runtimeConfig"]["sound"]["noteLength"] = json!(100),
        "unknown sound key",
    );
    assert_system_document_rejected(
        |system| {
            system["runtimeConfig"]["recording"]
                .as_object_mut()
                .unwrap()
                .remove("maxMinutes");
        },
        "missing recording length",
    );
    assert_system_document_rejected(
        |system| system["runtimeConfig"]["midi"]["clockOut"] = json!(true),
        "unknown MIDI key",
    );
}

#[test]
fn layer_xy_and_lfo_loads_save_stably_and_reject_missing_or_unknown_keys() {
    assert_saves_are_stable();
    assert_local_patch_rejected(
        |patch| {
            patch["runtimeConfig"]["xy"]
                .as_object_mut()
                .unwrap()
                .remove("smoothingMs");
        },
        "missing XY smoothing",
    );
    assert_local_patch_rejected(
        |patch| {
            patch["runtimeConfig"]["layers"][2]
                .as_object_mut()
                .unwrap()
                .remove("link");
        },
        "missing layer link",
    );
    assert_local_patch_rejected(
        |patch| patch["runtimeConfig"]["linkLfos"][0]["shape"] = json!("sine"),
        "unknown LFO key",
    );
    assert_local_patch_rejected(
        |patch| patch["runtimeConfig"]["layers"][0]["build"]["speed"] = json!(2),
        "unknown build key",
    );
}
