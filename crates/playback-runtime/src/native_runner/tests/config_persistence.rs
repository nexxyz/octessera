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

fn legacy_modulation_fixture() -> Value {
    serde_json::from_str(include_str!(
        "fixtures/config_persistence/legacy_modulation_v1.json"
    ))
    .unwrap()
}

#[test]
pub(crate) fn canonical_full_config_round_trip_through_fresh_runner_is_stable() {
    let canonical = canonical_factory_payload();
    let mut restored = NativeRunner::new(NativeRunnerConfig::default()).unwrap();

    restored.apply_config_payload(canonical.clone()).unwrap();

    assert_eq!(restored.config_payload(), canonical);
}

#[test]
pub(crate) fn legacy_envelopes_are_reemitted_as_canonical_v2() {
    let canonical = canonical_factory_payload();
    let mut unversioned = canonical.clone();
    let unversioned_object = unversioned.as_object_mut().unwrap();
    unversioned_object.remove("kind");
    unversioned_object.remove("schemaVersion");
    unversioned_object.remove("revision");

    let mut v1 = canonical.clone();
    v1["schemaVersion"] = json!(1);

    for input in [unversioned, v1] {
        let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
        runner.apply_config_payload(input).unwrap();
        let output = runner.config_payload();

        assert_eq!(output, canonical);
        assert_eq!(output["kind"], "octessera.config");
        assert_eq!(output["schemaVersion"], 2);
    }
}

#[test]
pub(crate) fn unknown_fields_are_tolerated_and_removed_from_canonical_output() {
    let mut runner = fresh_factory_runner();
    let canonical = runner.config_payload();
    let mut input = canonical.clone();
    input["unknownEnvelope"] = json!({ "value": "discard me" });
    input["runtimeConfig"]["unknownRuntime"] = json!({ "value": "discard me" });
    input["runtimeConfig"]["layers"][0]["worlds"]["unknownWorld"] =
        json!({ "value": "discard me" });
    input["runtimeConfig"]["instruments"][0]["sample"]["unknownSample"] =
        json!({ "value": "discard me" });
    input["runtimeConfig"]["layers"][0]["worlds"]["behaviorConfigHistory"]["opaque-extension"] =
        json!({ "sentinel": "keep me" });

    runner.apply_config_payload(input).unwrap();

    let output = runner.config_payload();
    let mut expected = canonical;
    expected["runtimeConfig"]["layers"][0]["worlds"]["behaviorConfigHistory"]["opaque-extension"] =
        json!({ "sentinel": "keep me" });
    assert_eq!(output, expected);
    assert!(output.as_object().unwrap().get("unknownEnvelope").is_none());
    assert!(output["runtimeConfig"]
        .as_object()
        .unwrap()
        .get("unknownRuntime")
        .is_none());
    assert!(output["runtimeConfig"]["layers"][0]["worlds"]
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
        output["runtimeConfig"]["layers"][0]["worlds"]["behaviorConfigHistory"]["opaque-extension"]
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
    let mut patch = runner.patch_payload();
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

    let musical_before_device = runner.patch_payload();
    let mut device = runner.device_config_payload();
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
        { "worlds": { "behaviorId": "brain" } }
    ]);
    device["runtimeConfig"]["mixer"] = json!({
        "buses": [{ "volumePct": 1 }]
    });
    runner
        .apply_device_config_payload_preserving_patch(device)
        .unwrap();

    assert_eq!(runner.patch_payload(), musical_before_device);
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
pub(crate) fn legacy_modulation_fixture_migrates_to_canonical_v2_without_phase_pulses() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();

    runner
        .apply_config_payload(legacy_modulation_fixture())
        .unwrap();

    let output = runner.config_payload();
    let lfo = &output["runtimeConfig"]["linkLfos"][0];
    assert_eq!(lfo["enabled"], true);
    assert_eq!(lfo["period"], "1/4");
    assert_eq!(lfo["depthPct"], 37);
    assert_eq!(lfo["target"]["key"], "instruments.0.mixer.volume");
    assert_eq!(
        output["runtimeConfig"]["xy"]["x"]["key"],
        "instruments.0.mixer.panPos"
    );
    assert_eq!(output["runtimeConfig"]["xy"]["xInvert"], true);
    assert_eq!(
        output["runtimeConfig"]["auxBindings"]["aux1"]["turnKey"],
        "linkLfos.0.depthPct"
    );
    assert_eq!(
        output["runtimeConfig"]["shiftAuxBindings"]["aux2"]["turnKey"],
        "linkLfos.1.period"
    );
    let canonical_lfos = output["runtimeConfig"]["linkLfos"].as_array().unwrap();
    assert_eq!(canonical_lfos.len(), 8);
    for lfo in canonical_lfos {
        assert!(lfo.as_object().unwrap().get("phasePulses").is_none());
    }
    assert_eq!(output["kind"], "octessera.config");
    assert_eq!(output["schemaVersion"], 2);
    let mut restored = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    restored.apply_config_payload(output.clone()).unwrap();
    assert_eq!(restored.config_payload(), output);
}

#[test]
pub(crate) fn prepared_legacy_envelopes_decode_as_current_typed_envelopes() {
    let canonical = canonical_factory_payload();
    let mut unversioned = canonical.clone();
    let unversioned_object = unversioned.as_object_mut().unwrap();
    unversioned_object.remove("kind");
    unversioned_object.remove("schemaVersion");
    unversioned_object.remove("revision");

    let mut v1 = canonical.clone();
    v1["schemaVersion"] = json!(1);

    for input in [unversioned, v1] {
        let prepared = prepare_config_payload(input, &canonical).unwrap();
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
    let worlds = input["runtimeConfig"]["layers"][0]["worlds"]
        .as_object_mut()
        .unwrap();
    let mut behavior_config = match worlds.get("behaviorConfig") {
        Some(value) if value.is_object() => value.clone(),
        _ => json!({}),
    };
    behavior_config
        .as_object_mut()
        .unwrap()
        .insert("opaque".into(), json!({ "nested": [true, null, "value"] }));
    worlds.insert("behaviorConfig".into(), behavior_config.clone());
    let mut behavior_history = worlds
        .get("behaviorConfigHistory")
        .cloned()
        .unwrap_or_else(|| json!({}));
    behavior_history
        .as_object_mut()
        .unwrap()
        .insert("opaque".into(), json!({ "history": [1, 2, 3] }));
    worlds.insert("behaviorConfigHistory".into(), behavior_history.clone());
    let fx_params = input["runtimeConfig"]["mixer"]["buses"][0]["slot1"]["params"].clone();
    let canonical_bytes = serde_json::to_vec(&input).unwrap();

    let prepared = prepare_config_payload(input.clone(), &canonical).unwrap();
    assert_eq!(prepared.payload, input);
    let prepared_bytes = serde_json::to_vec(&prepared.payload).unwrap();
    let decoded = &prepared.envelope;
    assert_eq!(
        decoded.runtime_config()["layers"][0]["worlds"]["behaviorConfig"],
        behavior_config
    );
    assert_eq!(
        decoded.runtime_config()["layers"][0]["worlds"]["behaviorConfigHistory"],
        behavior_history
    );
    assert_eq!(
        decoded.runtime_config()["mixer"]["buses"][0]["slot1"]["params"],
        fx_params
    );
    assert_eq!(
        serde_json::to_vec(&decoded.runtime_config()["layers"][0]["worlds"]["behaviorConfig"])
            .unwrap(),
        serde_json::to_vec(&behavior_config).unwrap()
    );
    assert_eq!(
        serde_json::to_vec(
            &decoded.runtime_config()["layers"][0]["worlds"]["behaviorConfigHistory"]
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
        decoded.runtime_config()["layers"][0]["worlds"]["behaviorConfig"]["opaque"],
        input["runtimeConfig"]["layers"][0]["worlds"]["behaviorConfig"]["opaque"]
    );
    assert_eq!(
        decoded.runtime_config()["layers"][0]["worlds"]["behaviorConfigHistory"]["opaque"],
        input["runtimeConfig"]["layers"][0]["worlds"]["behaviorConfigHistory"]["opaque"]
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
