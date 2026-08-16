use super::*;

fn generated_pi_default() -> Value {
    serde_json::from_str(include_str!(
        "../../../../../config/generated/pi/default.json"
    ))
    .unwrap()
}

fn generated_base_default() -> Value {
    serde_json::from_str(include_str!("../../../../../config/defaults/base.json")).unwrap()
}

fn generated_desktop_default() -> Value {
    serde_json::from_str(include_str!(
        "../../../../../config/generated/desktop/default.json"
    ))
    .unwrap()
}

fn explicit_orange_default_payload(mut payload: Value) -> Value {
    payload["revision"] = json!(91);
    payload["system"]["sparksMode"] = json!("pan");
    payload["runtimeConfig"]["displayBrightness"] = json!(13);
    payload["runtimeConfig"]["gridBrightness"] = json!(17);
    payload["runtimeConfig"]["buttonBrightness"] = json!(19);
    payload["runtimeConfig"]["sound"]["audioOutputBufferFrames"] = json!(2048);
    payload["runtimeConfig"]["audioOutputs"] = json!({
        "dac": false,
        "usb": true,
        "hdmi": false
    });
    payload["runtimeConfig"]["auxBindings"]["aux1"] = json!({
        "turnKey": "displayBrightness"
    });
    payload["runtimeConfig"]["shiftAuxBindings"]["aux1"] = json!({
        "turnKey": "gridBrightness"
    });
    payload
}

fn assert_same_portable_bytes(expected: (&str, &Value), actual: (&str, &Value)) {
    let expected_bytes = portable_patch_bytes(expected.1).unwrap();
    let actual_bytes = portable_patch_bytes(actual.1).unwrap();
    if expected_bytes == actual_bytes {
        return;
    }
    let expected_patch = portable_patch_projection(expected.1);
    let actual_patch = portable_patch_projection(actual.1);
    let path = first_json_difference(&expected_patch, &actual_patch, "$")
        .unwrap_or_else(|| "$.<serialized-bytes>".into());
    panic!(
        "portable patch bytes for {} and {} differ at {path}: expected {:?}, actual {:?}",
        expected.0, actual.0, expected_patch, actual_patch
    );
}

fn first_json_difference(left: &Value, right: &Value, path: &str) -> Option<String> {
    if left == right {
        return None;
    }
    match (left, right) {
        (Value::Object(left), Value::Object(right)) => {
            for key in left.keys().chain(right.keys()) {
                let next_path = format!("{path}.{key}");
                match (left.get(key), right.get(key)) {
                    (Some(left), Some(right)) => {
                        if let Some(path) = first_json_difference(left, right, &next_path) {
                            return Some(path);
                        }
                    }
                    _ => return Some(next_path),
                }
            }
            None
        }
        (Value::Array(left), Value::Array(right)) => {
            if left.len() != right.len() {
                return Some(path.into());
            }
            left.iter()
                .zip(right)
                .enumerate()
                .find_map(|(index, (left, right))| {
                    first_json_difference(left, right, &format!("{path}[{index}]"))
                })
        }
        _ => Some(path.into()),
    }
}

#[test]
pub(crate) fn unmodified_defaults_have_identical_portable_patch_bytes() {
    let base = generated_base_default();
    let desktop = generated_desktop_default();
    let pi = generated_pi_default();
    let orange = explicit_orange_default_payload(pi.clone());
    let expected = ("base", &base);

    for actual in [("desktop", &desktop), ("pi", &pi), ("orange", &orange)] {
        assert_same_portable_bytes(expected, actual);
    }
}

#[test]
pub(crate) fn portable_projection_excludes_history_and_device_fields_without_number_normalization()
{
    let pi = generated_pi_default();
    let orange = explicit_orange_default_payload(pi.clone());
    let pi_patch = portable_patch_projection(&pi);
    let orange_patch = portable_patch_projection(&orange);

    assert_eq!(
        portable_patch_bytes(&pi).unwrap(),
        portable_patch_bytes(&orange).unwrap()
    );
    assert_ne!(
        serde_json::to_vec(&pi).unwrap(),
        serde_json::to_vec(&orange).unwrap()
    );
    assert!(orange_patch.get("revision").is_none());
    assert!(orange_patch.get("system").is_none());
    assert!(orange_patch["runtimeConfig"]
        .as_object()
        .unwrap()
        .get("displayBrightness")
        .is_none());
    assert!(orange_patch["runtimeConfig"]
        .as_object()
        .unwrap()
        .get("audioOutputs")
        .is_none());
    assert!(orange_patch["runtimeConfig"]["sound"]
        .as_object()
        .unwrap()
        .get("audioOutputBufferFrames")
        .is_none());
    assert!(orange_patch["runtimeConfig"]["auxBindings"]["aux1"].is_null());
    assert!(orange_patch["runtimeConfig"]["shiftAuxBindings"]["aux1"].is_null());
    assert_eq!(pi_patch, orange_patch);
}

#[test]
pub(crate) fn lfo_aux_bindings_are_musical_in_both_aux_banks() {
    let mut payload = generated_pi_default();
    payload["runtimeConfig"]["auxBindings"]["aux1"] = json!({
        "turnKey": "linkLfos.0.depthPct"
    });
    payload["runtimeConfig"]["shiftAuxBindings"]["aux2"] = json!({
        "turnKey": "linkLfos.1.period"
    });

    let patch = portable_patch_projection(&payload);

    assert_eq!(
        patch["runtimeConfig"]["auxBindings"]["aux1"]["turnKey"],
        "linkLfos.0.depthPct"
    );
    assert_eq!(
        patch["runtimeConfig"]["shiftAuxBindings"]["aux2"]["turnKey"],
        "linkLfos.1.period"
    );
}

#[test]
pub(crate) fn portable_save_load_round_trip_preserves_canonical_bytes_and_projection() {
    let mut source = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    source.aux_bindings[0] = Some(NativeAuxBinding {
        turn_key: Some("linkLfos.0.depthPct".into()),
        press_action: None,
    });
    source.shift_aux_bindings[1] = Some(NativeAuxBinding {
        turn_key: Some("linkLfos.1.period".into()),
        press_action: None,
    });
    let source_payload = source.config_payload();
    let saved_patch = portable_patch_projection(&source_payload);
    let saved_bytes = portable_patch_bytes(&source_payload).unwrap();

    let mut loaded = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    loaded.aux_bindings[0] = Some(NativeAuxBinding {
        turn_key: Some("displayBrightness".into()),
        press_action: None,
    });
    loaded.shift_aux_bindings[1] = Some(NativeAuxBinding {
        turn_key: Some("gridBrightness".into()),
        press_action: None,
    });
    let saved_payload: Value = serde_json::from_slice(&saved_bytes).unwrap();
    loaded
        .apply_patch_payload_preserving_device(saved_payload)
        .unwrap();

    assert_eq!(loaded.patch_payload(), saved_patch);
    assert_eq!(
        portable_patch_bytes(&loaded.config_payload()).unwrap(),
        saved_bytes
    );
    assert_eq!(
        loaded.config_payload()["runtimeConfig"]["auxBindings"]["aux1"]["turnKey"],
        "linkLfos.0.depthPct"
    );
    assert_eq!(
        loaded.config_payload()["runtimeConfig"]["shiftAuxBindings"]["aux2"]["turnKey"],
        "linkLfos.1.period"
    );
}

#[test]
pub(crate) fn v2_portable_patch_unknown_fields_report_json_paths() {
    let runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut unknown_runtime = runner.patch_payload();
    unknown_runtime["runtimeConfig"]["futureField"] = json!(true);
    let error = prepare_patch_payload(unknown_runtime, &runner.config_payload()).unwrap_err();
    assert!(error.contains("$.runtimeConfig.futureField"), "{error}");

    let mut unknown_lfo = runner.patch_payload();
    unknown_lfo["runtimeConfig"]["linkLfos"][0]["futureField"] = json!(true);
    let error = prepare_patch_payload(unknown_lfo, &runner.config_payload()).unwrap_err();
    assert!(
        error.contains("$.runtimeConfig.linkLfos[0].futureField"),
        "{error}"
    );

    let mut unknown_system = runner.patch_payload();
    unknown_system["system"] = json!({ "futureField": true });
    let error = prepare_patch_payload(unknown_system, &runner.config_payload()).unwrap_err();
    assert!(error.contains("$.system.futureField"), "{error}");
}

#[test]
pub(crate) fn orange_uses_pi_default_source_and_preserves_portable_projection() {
    let pi = generated_pi_default();
    let orange = explicit_orange_default_payload(pi.clone());
    assert_eq!(
        portable_patch_bytes(&pi).unwrap(),
        portable_patch_bytes(&orange).unwrap()
    );

    let orange_startup = include_str!("../../../../../apps/pi-zero/src/orange_host_adapter.rs");
    assert!(orange_startup.contains("load_json(&self.store_dir.join(\"default.json\"))"));
    assert!(orange_startup.contains("RuntimeStoreResult::LoadDefaultResult { payload }"));
    assert!(orange_startup.contains("payload: payload.clone(),"));

    let orange_apply = include_str!("../../../../../apps/pi-zero/src/orange_device_apply.rs");
    assert!(orange_apply.contains("serde_json::to_vec_pretty(payload)"));
}

#[test]
pub(crate) fn portable_bytes_preserve_numeric_representation() {
    let integer = json!({ "runtimeConfig": { "transport": { "bpm": 120 } } });
    let decimal = json!({ "runtimeConfig": { "transport": { "bpm": 120.0 } } });

    assert_ne!(
        portable_patch_bytes(&integer).unwrap(),
        portable_patch_bytes(&decimal).unwrap()
    );
}
