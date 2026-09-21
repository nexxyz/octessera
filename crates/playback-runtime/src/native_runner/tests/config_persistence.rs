use super::*;

fn fresh_factory_runner() -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner
        .apply_config_payload(native_factory_payload())
        .unwrap();
    runner
}

fn canonical_factory_payload() -> Value {
    fresh_factory_runner().config_payload()
}

#[test]
pub(crate) fn canonical_full_config_round_trip_through_fresh_runner_is_stable() {
    let canonical = canonical_factory_payload();
    let mut restored = NativeRunner::new(NativeRunnerConfig::default()).unwrap();

    restored.apply_config_payload(canonical.clone()).unwrap();

    assert_eq!(restored.config_payload(), canonical);
}

#[test]
pub(crate) fn unversioned_envelopes_are_reemitted_as_canonical_v2() {
    let canonical = canonical_factory_payload();
    let mut unversioned = canonical.clone();
    let unversioned_object = unversioned.as_object_mut().unwrap();
    unversioned_object.remove("kind");
    unversioned_object.remove("schemaVersion");
    unversioned_object.remove("revision");

    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.apply_config_payload(unversioned).unwrap();
    let output = runner.config_payload();

    assert_eq!(output, canonical);
    assert_eq!(output["kind"], "octessera.config");
    assert_eq!(output["schemaVersion"], 2);
}

#[test]
pub(crate) fn unknown_fields_are_tolerated_and_removed_from_canonical_output() {
    let mut runner = fresh_factory_runner();
    let canonical = runner.config_payload();
    let mut input = canonical.clone();
    input["unknownEnvelope"] = json!({ "value": "discard me" });
    input["runtimeConfig"]["unknownRuntime"] = json!({ "value": "discard me" });
    input["runtimeConfig"]["layers"][0]["build"]["unknownWorld"] = json!({ "value": "discard me" });
    input["runtimeConfig"]["instruments"][0]["sample"]["unknownSample"] =
        json!({ "value": "discard me" });
    input["runtimeConfig"]["layers"][0]["build"]["behaviorConfigHistory"]["opaque-extension"] =
        json!({ "sentinel": "keep me" });

    runner.apply_config_payload(input).unwrap();

    let output = runner.config_payload();
    let mut expected = canonical;
    expected["runtimeConfig"]["layers"][0]["build"]["behaviorConfigHistory"]["opaque-extension"] =
        json!({ "sentinel": "keep me" });
    assert_eq!(output, expected);
    assert!(output.as_object().unwrap().get("unknownEnvelope").is_none());
    assert!(output["runtimeConfig"]
        .as_object()
        .unwrap()
        .get("unknownRuntime")
        .is_none());
    assert!(output["runtimeConfig"]["layers"][0]["build"]
        .as_object()
        .unwrap()
        .get("unknownWorld")
        .is_none());
    assert!(output["runtimeConfig"]["instruments"][0]["sample"]
        .as_object()
        .unwrap()
        .get("unknownSample")
        .is_none());
    assert_eq!(
        output["runtimeConfig"]["layers"][0]["build"]["behaviorConfigHistory"]["opaque-extension"]
            ["sentinel"],
        "keep me"
    );
}

#[test]
pub(crate) fn patch_and_device_payloads_preserve_the_other_owner() {
    let mut runner = fresh_factory_runner();
    let mut musical_change = runner.config_payload();
    musical_change["runtimeConfig"]["instruments"][0]["mixer"]["volume"] = json!(41);
    runner.apply_config_payload(musical_change).unwrap();

    let device_before = runner.config_payload()["runtimeConfig"].clone();
    let mut patch = runner.patch_payload().unwrap();
    patch["runtimeConfig"]["instruments"][0]["mixer"]["volume"] = json!(63);
    patch["runtimeConfig"]["masterVolume"] = json!(12);
    patch["runtimeConfig"]["displayBrightness"] = json!(1);
    runner.apply_patch_payload_preserving_device(patch).unwrap();

    assert_eq!(
        runner.config_payload()["runtimeConfig"]["instruments"][0]["mixer"]["volume"],
        63
    );
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["masterVolume"],
        device_before["masterVolume"]
    );
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["displayBrightness"],
        device_before["displayBrightness"]
    );

    let musical_before_device = runner.patch_payload().unwrap();
    let mut device = runner.device_config_payload().unwrap();
    device["runtimeConfig"]["masterVolume"] = json!(22);
    device["runtimeConfig"]["displayBrightness"] = json!(17);
    device["runtimeConfig"]["audioOutputs"] = json!({
        "dac": false,
        "usb": true,
        "hdmi": false
    });
    device["runtimeConfig"]["instruments"] = json!([
        { "mixer": { "volume": 1 } }
    ]);
    device["runtimeConfig"]["layers"] = json!([
        { "build": { "behaviorId": "brain" } }
    ]);
    device["runtimeConfig"]["mixer"] = json!({
        "buses": [{ "volumePct": 1 }]
    });
    runner
        .apply_device_config_payload_preserving_patch(device)
        .unwrap();

    assert_eq!(runner.patch_payload().unwrap(), musical_before_device);
    assert_eq!(runner.config_payload()["runtimeConfig"]["masterVolume"], 22);
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["displayBrightness"],
        17
    );
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["audioOutputs"],
        json!({ "dac": false, "usb": true, "hdmi": false })
    );
}

#[test]
pub(crate) fn prepared_unversioned_envelopes_decode_as_current_typed_envelopes() {
    let canonical = canonical_factory_payload();
    let mut unversioned = canonical.clone();
    let unversioned_object = unversioned.as_object_mut().unwrap();
    unversioned_object.remove("kind");
    unversioned_object.remove("schemaVersion");
    unversioned_object.remove("revision");

    let prepared = prepare_config_payload(unversioned, &canonical).unwrap();
    assert_eq!(prepared.envelope.kind(), "octessera.config");
    assert_eq!(prepared.envelope.schema_version(), 2);
    assert_eq!(prepared.envelope.revision(), canonical["revision"].as_u64());
    assert_eq!(
        prepared.envelope.runtime_config(),
        &prepared.payload["runtimeConfig"]
    );
    assert_eq!(
        prepared.envelope.mapping_config(),
        prepared.payload.as_object().unwrap().get("mappingConfig")
    );
    assert_eq!(
        prepared.envelope.system(),
        prepared.payload.as_object().unwrap().get("system")
    );
}

#[test]
pub(crate) fn typed_envelope_exposes_root_extensions_without_serializing_them() {
    let canonical = canonical_factory_payload();
    let mut input = canonical.clone();
    input["rootExtension"] = json!({ "sentinel": [1, 2, 3] });

    let prepared = prepare_config_payload(input.clone(), &canonical).unwrap();
    assert_eq!(
        prepared.envelope.extensions().get("rootExtension"),
        input.as_object().unwrap().get("rootExtension")
    );

    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.apply_config_payload(input).unwrap();
    assert!(runner.config_payload().get("rootExtension").is_none());
}

#[test]
pub(crate) fn application_view_rejects_missing_runtime_config_without_fallback() {
    let canonical = canonical_factory_payload();
    let envelope = ConfigDto::decode(&canonical).unwrap();
    let malformed = json!({
        "kind": "octessera.config",
        "schemaVersion": 2
    });

    assert_eq!(
        envelope.application_view(&malformed).unwrap_err(),
        "configuration payload is missing runtimeConfig"
    );
}

#[test]
pub(crate) fn typed_decode_keeps_opaque_state_and_canonical_bytes_unchanged() {
    let canonical = canonical_factory_payload();
    let mut input = canonical.clone();
    let build = input["runtimeConfig"]["layers"][0]["build"]
        .as_object_mut()
        .unwrap();
    let mut behavior_config = match build.get("behaviorConfig") {
        Some(value) if value.is_object() => value.clone(),
        _ => json!({}),
    };
    behavior_config
        .as_object_mut()
        .unwrap()
        .insert("opaque".into(), json!({ "nested": [true, null, "value"] }));
    build.insert("behaviorConfig".into(), behavior_config.clone());
    let mut behavior_history = build
        .get("behaviorConfigHistory")
        .cloned()
        .unwrap_or_else(|| json!({}));
    behavior_history
        .as_object_mut()
        .unwrap()
        .insert("opaque".into(), json!({ "history": [1, 2, 3] }));
    build.insert("behaviorConfigHistory".into(), behavior_history.clone());
    let fx_params = input["runtimeConfig"]["mixer"]["buses"][0]["slot1"]["params"].clone();
    let canonical_bytes = serde_json::to_vec(&input).unwrap();

    let prepared = prepare_config_payload(input.clone(), &canonical).unwrap();
    assert_eq!(prepared.payload, input);
    let prepared_bytes = serde_json::to_vec(&prepared.payload).unwrap();
    let decoded = &prepared.envelope;
    assert_eq!(
        decoded.runtime_config()["layers"][0]["build"]["behaviorConfig"],
        behavior_config
    );
    assert_eq!(
        decoded.runtime_config()["layers"][0]["build"]["behaviorConfigHistory"],
        behavior_history
    );
    assert_eq!(
        decoded.runtime_config()["mixer"]["buses"][0]["slot1"]["params"],
        fx_params
    );
    assert_eq!(
        serde_json::to_vec(&decoded.runtime_config()["layers"][0]["build"]["behaviorConfig"])
            .unwrap(),
        serde_json::to_vec(&behavior_config).unwrap()
    );
    assert_eq!(
        serde_json::to_vec(
            &decoded.runtime_config()["layers"][0]["build"]["behaviorConfigHistory"]
        )
        .unwrap(),
        serde_json::to_vec(&behavior_history).unwrap()
    );
    assert_eq!(
        serde_json::to_vec(&decoded.runtime_config()["mixer"]["buses"][0]["slot1"]["params"])
            .unwrap(),
        serde_json::to_vec(&fx_params).unwrap()
    );
    assert_eq!(
        decoded.runtime_config()["layers"][0]["build"]["behaviorConfig"]["opaque"],
        input["runtimeConfig"]["layers"][0]["build"]["behaviorConfig"]["opaque"]
    );
    assert_eq!(
        decoded.runtime_config()["layers"][0]["build"]["behaviorConfigHistory"]["opaque"],
        input["runtimeConfig"]["layers"][0]["build"]["behaviorConfigHistory"]["opaque"]
    );
    assert_eq!(
        serde_json::to_vec(&prepared.payload).unwrap(),
        canonical_bytes
    );
    assert_eq!(
        serde_json::to_vec(&prepared.payload).unwrap(),
        prepared_bytes
    );
}

#[test]
pub(crate) fn invalid_v2_is_rejected_before_typed_decode() {
    let canonical = canonical_factory_payload();
    let mut invalid = canonical.clone();
    invalid["runtimeConfig"]["masterVolume"] = json!(101);

    assert!(ConfigDto::decode(&invalid).is_ok());
    assert_eq!(
        prepare_config_payload(invalid, &canonical).unwrap_err(),
        "configuration.runtimeConfig.masterVolume is outside the supported range"
    );
}

#[test]
pub(crate) fn full_config_to_portable_conversion_validates_and_emits_canonical_patch() {
    let canonical = canonical_factory_payload();
    let mut malformed = canonical.clone();
    malformed["runtimeConfig"]["masterVolume"] = json!("broken");
    let error = normalize_user_data_patch_payload(malformed, &canonical).unwrap_err();
    assert!(error.contains("masterVolume"), "{error}");

    let patch = normalize_user_data_patch_payload(canonical.clone(), &canonical).unwrap();
    assert_eq!(patch["kind"], "octessera.patch");
    assert_eq!(patch["schemaVersion"], 2);
    assert_eq!(
        patch["runtimeConfig"]["linkLfos"].as_array().unwrap().len(),
        8
    );
    assert!(patch["runtimeConfig"]["masterVolume"].is_null());
}

fn opaque_scalar_collision_fixture() -> Value {
    json!({
        "revision": "behavior-owned revision",
        "enabled": "behavior-owned enabled",
        "durationMs": "behavior-owned duration",
        "channel": "behavior-owned channel",
        "masterVolume": "behavior-owned volume",
        "params": {
            "revision": "parameter revision",
            "enabled": "parameter enabled",
            "durationMs": "parameter duration",
            "channel": "parameter channel"
        }
    })
}

#[test]
pub(crate) fn opaque_subtrees_accept_colliding_scalar_names_but_canonical_paths_remain_strict() {
    let canonical = canonical_factory_payload();
    let mut payload = canonical.clone();
    let build = payload["runtimeConfig"]["layers"][0]["build"]
        .as_object_mut()
        .unwrap();
    let collision = opaque_scalar_collision_fixture();
    build.insert("behaviorConfig".into(), collision.clone());
    build.insert(
        "behaviorConfigHistory".into(),
        json!({ "collision": collision.clone() }),
    );
    build.insert("savedState".into(), collision.clone());

    validate_config_payload(&payload).unwrap();
    let prepared = prepare_config_payload(payload.clone(), &canonical).unwrap();
    for key in ["behaviorConfig", "savedState"] {
        let output = prepared.payload["runtimeConfig"]["layers"][0]["build"][key]
            .as_object()
            .unwrap();
        for (field, value) in collision.as_object().unwrap() {
            assert_eq!(output.get(field), Some(value));
        }
    }
    let history = prepared.payload["runtimeConfig"]["layers"][0]["build"]["behaviorConfigHistory"]
        ["collision"]
        .as_object()
        .unwrap();
    for (field, value) in collision.as_object().unwrap() {
        assert_eq!(history.get(field), Some(value));
    }
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.apply_config_payload(payload).unwrap();
    let output = runner.config_payload();
    for (field, value) in collision.as_object().unwrap() {
        assert_eq!(
            output["runtimeConfig"]["layers"][0]["build"]["behaviorConfig"].get(field),
            Some(value)
        );
    }
    assert_eq!(
        output["runtimeConfig"]["layers"][0]["build"]["behaviorConfigHistory"]["collision"],
        collision
    );

    let mut unknown_state = canonical.clone();
    unknown_state["runtimeConfig"]["layers"][0]["build"]["state"] = collision.clone();
    assert!(prepare_config_payload(unknown_state, &runner.config_payload()).is_err());

    let mut invalid = canonical;
    invalid["runtimeConfig"]["masterVolume"] = json!(101);
    let error = prepare_config_payload(invalid, &runner.config_payload()).unwrap_err();
    assert!(
        error.contains("configuration.runtimeConfig.masterVolume"),
        "{error}"
    );
}
