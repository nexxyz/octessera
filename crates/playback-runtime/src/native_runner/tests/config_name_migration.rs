use super::*;

fn custom_named_current() -> Value {
    let mut current = native_factory_payload();
    current["runtimeConfig"]["instruments"][0]["name"] = json!("custom instrument");
    current["runtimeConfig"]["mixer"]["buses"][0]["name"] = json!("custom bus");
    current
}

#[test]
pub(crate) fn derived_names_are_canonicalized_during_preparation() {
    let runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let current = runner.config_payload();
    let legacy = json!({
        "runtimeConfig": {
            "instruments": [
                { "type": "sampler", "name": "sampler", "autoName": true },
                { "type": "midi", "name": "custom midi", "autoName": true },
                { "type": "midi", "name": "midi", "autoName": false },
                { "type": "midi", "autoName": true }
            ],
            "mixer": {
                "buses": [
                    {
                        "slot1": {
                            "type": "delay",
                            "params": {
                                "timeMode": "ms",
                                "timeNote": "1/8",
                                "timeMs": 250,
                                "feedback": 0.35,
                                "mixPct": 35,
                                "spreadPct": 0
                            }
                        },
                        "slot2": { "type": "duck" },
                        "slot3": { "type": "none" },
                        "name": "delay+duck",
                        "autoName": true
                    },
                    {
                        "slot1": { "type": "reverb" },
                        "name": "my reverb bus",
                        "autoName": true
                    },
                    {
                        "slot1": {
                            "type": "delay",
                            "params": {
                                "timeMode": "ms",
                                "timeNote": "1/8",
                                "timeMs": 250,
                                "feedback": 0.35,
                                "mixPct": 35,
                                "spreadPct": 0
                            }
                        },
                        "name": "delay",
                        "autoName": false
                    },
                    { "slot1": { "type": "reverb" }, "autoName": true }
                ]
            }
        }
    });

    let prepared = prepare_config_payload(legacy.clone(), &current).unwrap();
    assert_eq!(
        prepared.apply_payload["runtimeConfig"]["instruments"][0]["name"],
        "Sampler"
    );
    assert_eq!(
        prepared.apply_payload["runtimeConfig"]["instruments"][3]["name"],
        "MIDI"
    );
    assert_eq!(
        prepared.apply_payload["runtimeConfig"]["mixer"]["buses"][0]["name"],
        "Delay+Duck"
    );
    assert_eq!(
        prepared.apply_payload["runtimeConfig"]["mixer"]["buses"][1]["name"],
        "my reverb bus"
    );
    assert_eq!(
        prepared.apply_payload["runtimeConfig"]["mixer"]["buses"][2]["name"],
        "delay"
    );
    assert_eq!(
        prepared.apply_payload["runtimeConfig"]["mixer"]["buses"][3]["name"],
        "Reverb"
    );

    let mut restored = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    restored.apply_config_payload(legacy).unwrap();
    let canonical = restored.config_payload();
    assert_eq!(
        canonical["runtimeConfig"]["instruments"][0]["name"],
        "Sampler"
    );
    assert_eq!(
        canonical["runtimeConfig"]["instruments"][1]["name"],
        "custom midi"
    );
    assert_eq!(canonical["runtimeConfig"]["instruments"][2]["name"], "midi");
    assert_eq!(canonical["runtimeConfig"]["instruments"][3]["name"], "MIDI");
    assert_eq!(
        canonical["runtimeConfig"]["mixer"]["buses"][0]["name"],
        "Delay+Duck"
    );
    assert_eq!(
        canonical["runtimeConfig"]["mixer"]["buses"][1]["name"],
        "my reverb bus"
    );
    assert_eq!(
        canonical["runtimeConfig"]["mixer"]["buses"][2]["name"],
        "delay"
    );
    assert_eq!(
        canonical["runtimeConfig"]["mixer"]["buses"][3]["name"],
        "Reverb"
    );

    let mut round_trip = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    round_trip.apply_config_payload(canonical.clone()).unwrap();
    assert_eq!(round_trip.config_payload(), canonical);

    let mut v2 = native_factory_payload();
    v2["runtimeConfig"]["instruments"][0]["type"] = json!("sampler");
    v2["runtimeConfig"]["instruments"][0]["name"] = json!("sampler");
    v2["runtimeConfig"]["mixer"]["buses"][0]["name"] = json!("delay+duck");
    let prepared_v2 = prepare_config_payload(v2.clone(), &current).unwrap();
    assert_eq!(
        prepared_v2.payload["runtimeConfig"]["instruments"][0]["name"],
        "Sampler"
    );
    assert_eq!(
        prepared_v2.payload["runtimeConfig"]["mixer"]["buses"][0]["name"],
        "Delay+Duck"
    );
    let mut v2_loaded = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    v2_loaded.apply_config_payload(v2).unwrap();
    assert_eq!(
        v2_loaded.config_payload()["runtimeConfig"]["instruments"][0]["name"],
        "Sampler"
    );
    assert_eq!(
        v2_loaded.config_payload()["runtimeConfig"]["mixer"]["buses"][0]["name"],
        "Delay+Duck"
    );
}

#[test]
pub(crate) fn legacy_patch_names_are_canonicalized_before_patch_application() {
    let current = NativeRunner::new(NativeRunnerConfig::default())
        .unwrap()
        .config_payload();
    let patch = json!({
        "runtimeConfig": {
            "instruments": [
                { "type": "sampler", "name": "sampler", "autoName": true }
            ],
            "mixer": {
                "buses": [
                    {
                        "slot1": {
                            "type": "delay",
                            "params": {
                                "timeMode": "ms",
                                "timeNote": "1/8",
                                "timeMs": 250,
                                "feedback": 0.35,
                                "mixPct": 35,
                                "spreadPct": 0
                            }
                        },
                        "slot2": { "type": "duck" },
                        "slot3": { "type": "none" },
                        "name": "delay+duck",
                        "autoName": true
                    }
                ]
            }
        }
    });
    let prepared = prepare_patch_payload(patch.clone(), &current).unwrap();

    assert_eq!(
        prepared.apply_payload["runtimeConfig"]["instruments"][0]["name"],
        "Sampler"
    );
    assert_eq!(
        prepared.apply_payload["runtimeConfig"]["mixer"]["buses"][0]["name"],
        "Delay+Duck"
    );

    let mut v2_patch = patch.clone();
    v2_patch["kind"] = json!("octessera.patch");
    v2_patch["schemaVersion"] = json!(2);
    let prepared_v2 = prepare_patch_payload(v2_patch.clone(), &current).unwrap();
    assert_eq!(
        prepared_v2.payload["runtimeConfig"]["instruments"][0]["name"],
        "Sampler"
    );
    assert_eq!(
        prepared_v2.payload["runtimeConfig"]["mixer"]["buses"][0]["name"],
        "Delay+Duck"
    );
    let mut v2_loaded = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    v2_loaded
        .apply_patch_payload_preserving_device(v2_patch)
        .unwrap();
    assert_eq!(
        v2_loaded.config_payload()["runtimeConfig"]["instruments"][0]["name"],
        "Sampler"
    );
    assert_eq!(
        v2_loaded.config_payload()["runtimeConfig"]["mixer"]["buses"][0]["name"],
        "Delay+Duck"
    );

    let mut loaded = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    loaded.apply_patch_payload_preserving_device(patch).unwrap();
    let canonical = loaded.patch_payload().unwrap();

    let mut round_trip = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    round_trip
        .apply_patch_payload_preserving_device(canonical.clone())
        .unwrap();
    assert_eq!(round_trip.patch_payload().unwrap(), canonical);
}

#[test]
pub(crate) fn merged_full_config_preserves_custom_names_when_source_omits_names() {
    let current = custom_named_current();
    let mut source = current.clone();
    source["runtimeConfig"]["instruments"][0]
        .as_object_mut()
        .unwrap()
        .remove("name");
    source["runtimeConfig"]["mixer"]["buses"][0]
        .as_object_mut()
        .unwrap()
        .remove("name");

    let prepared = prepare_config_payload(source, &current).unwrap();
    assert_eq!(
        prepared.payload["runtimeConfig"]["instruments"][0]["name"],
        "custom instrument"
    );
    assert_eq!(
        prepared.payload["runtimeConfig"]["mixer"]["buses"][0]["name"],
        "custom bus"
    );
}

#[test]
pub(crate) fn merged_patch_preserves_custom_names_when_source_omits_names() {
    let current = custom_named_current();
    let patch = json!({
        "kind": "octessera.patch",
        "schemaVersion": 2,
        "runtimeConfig": {
            "instruments": [{ "autoName": true }],
            "mixer": { "buses": [{ "autoName": true }] }
        }
    });

    let prepared = prepare_patch_payload(patch, &current).unwrap();
    assert_eq!(
        prepared.payload["runtimeConfig"]["instruments"][0]["name"],
        "custom instrument"
    );
    assert_eq!(
        prepared.payload["runtimeConfig"]["mixer"]["buses"][0]["name"],
        "custom bus"
    );
}
