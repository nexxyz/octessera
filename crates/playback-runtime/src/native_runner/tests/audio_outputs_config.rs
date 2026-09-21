use super::*;

#[test]
pub(crate) fn public_runtime_audio_outputs_decoder_accepts_canonical_form() {
    assert_eq!(
        AudioOutputSet::decode_runtime_config(&json!({
            "runtimeConfig": {
                "audioOutputs": { "dac": false, "usb": true, "hdmi": true },
                "usb": { "midiOutEnabled": true }
            }
        }))
        .unwrap(),
        AudioOutputSet::from_flags(false, true, true).unwrap()
    );
}

#[test]
pub(crate) fn public_runtime_audio_outputs_decoder_rejects_missing_and_empty_forms() {
    assert!(AudioOutputSet::decode_runtime_config(&json!({})).is_err());
    assert!(AudioOutputSet::decode_runtime_config(&json!({
        "runtimeConfig": {}
    }))
    .is_err());
}

#[test]
pub(crate) fn public_runtime_audio_outputs_decoder_rejects_legacy_field() {
    for payload in [
        json!({
            "runtimeConfig": { "usb": { "audioOut": "usb" } }
        }),
        json!({
            "runtimeConfig": {
                "audioOutputs": { "dac": true, "usb": false, "hdmi": true },
                "usb": { "audioOut": "jack" }
            }
        }),
    ] {
        let error = AudioOutputSet::decode_runtime_config(&payload).unwrap_err();
        assert!(error.contains("runtimeConfig.usb.audioOut"), "{error}");
    }
}

#[test]
pub(crate) fn all_valid_canonical_audio_output_sets_round_trip() {
    for (dac, usb, hdmi) in [
        (true, false, false),
        (false, true, false),
        (false, false, true),
        (true, true, false),
        (true, false, true),
        (false, true, true),
        (true, true, true),
    ] {
        let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
        let mut payload = runner.config_payload();
        payload["runtimeConfig"]["audioOutputs"] = json!({
            "dac": dac,
            "usb": usb,
            "hdmi": hdmi,
        });
        runner.apply_config_payload(payload).unwrap();
        assert_eq!(
            runner.config_payload()["runtimeConfig"]["audioOutputs"],
            json!({ "dac": dac, "usb": usb, "hdmi": hdmi })
        );
    }
}

#[test]
pub(crate) fn current_schema_rejects_malformed_empty_and_extra_audio_outputs() {
    for audio_outputs in [
        json!({}),
        json!({ "dac": true, "usb": false }),
        json!({ "dac": true, "usb": false, "hdmi": false, "other": false }),
        json!({ "dac": true, "usb": "false", "hdmi": false }),
        json!({ "dac": false, "usb": false, "hdmi": false }),
    ] {
        let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
        let mut payload = runner.config_payload();
        payload["runtimeConfig"]["audioOutputs"] = audio_outputs;
        assert_rejected_without_byte_changes(&mut runner, payload);
    }
}

#[test]
pub(crate) fn canonical_audio_outputs_and_usb_midi_are_independent() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["audioOutputs"] = json!({
        "dac": false,
        "usb": true,
        "hdmi": true
    });
    payload["runtimeConfig"]["usb"]["midiOutEnabled"] = json!(true);
    runner.apply_config_payload(payload).unwrap();
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["audioOutputs"],
        json!({ "dac": false, "usb": true, "hdmi": true })
    );
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["usb"]["midiOutEnabled"],
        true
    );

    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["audioOutputs"] = json!({
        "dac": true,
        "usb": false,
        "hdmi": true
    });
    runner.apply_config_payload(payload).unwrap();
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["audioOutputs"],
        json!({ "dac": true, "usb": false, "hdmi": true })
    );
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["usb"]["midiOutEnabled"],
        true
    );
}

#[test]
pub(crate) fn legacy_audio_out_maps_and_preserves_usb_fields() {
    for (legacy, expected) in [
        ("jack", json!({ "dac": true, "usb": false, "hdmi": false })),
        ("usb", json!({ "dac": false, "usb": true, "hdmi": false })),
        ("both", json!({ "dac": true, "usb": true, "hdmi": false })),
    ] {
        let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
        let mut payload = json!({
            "runtimeConfig": {
                "usb": { "dataRole": "gadget", "midiOutEnabled": true }
            }
        });
        payload["runtimeConfig"]["usb"]["audioOut"] = json!(legacy);

        runner.apply_config_payload(payload).unwrap();

        assert_eq!(
            runner.config_payload()["runtimeConfig"]["audioOutputs"],
            expected
        );
        assert!(runner.config_payload()["runtimeConfig"]["usb"]
            .get("audioOut")
            .is_none());
        assert_eq!(
            runner.config_payload()["runtimeConfig"]["usb"]["midiOutEnabled"],
            true
        );
        assert_eq!(
            runner.config_payload()["runtimeConfig"]["usb"]["dataRole"],
            "gadget"
        );
    }
}

#[test]
pub(crate) fn legacy_audio_out_agrees_with_canonical_and_preserves_hdmi() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut payload = legacy_payload(runner.config_payload());
    payload["runtimeConfig"]["audioOutputs"] = json!({
        "dac": true,
        "usb": false,
        "hdmi": true
    });
    payload["runtimeConfig"]["usb"]["audioOut"] = json!("jack");

    runner.apply_config_payload(payload).unwrap();

    assert_eq!(
        runner.config_payload()["runtimeConfig"]["audioOutputs"],
        json!({ "dac": true, "usb": false, "hdmi": true })
    );
    assert!(runner.config_payload()["runtimeConfig"]["usb"]
        .get("audioOut")
        .is_none());
}

#[test]
pub(crate) fn legacy_audio_out_rejects_disagreement_and_invalid_values_without_changes() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut disagreement = legacy_payload(runner.config_payload());
    disagreement["runtimeConfig"]["audioOutputs"] = json!({
        "dac": true,
        "usb": false,
        "hdmi": true
    });
    disagreement["runtimeConfig"]["usb"]["audioOut"] = json!("usb");
    assert_rejected_without_byte_changes(&mut runner, disagreement);

    for value in [json!("dac"), json!(false), json!(null)] {
        let mut payload = legacy_payload(runner.config_payload());
        payload["runtimeConfig"]
            .as_object_mut()
            .unwrap()
            .remove("audioOutputs");
        payload["runtimeConfig"]["usb"]["audioOut"] = value;
        assert_rejected_without_byte_changes(&mut runner, payload);
    }
}

#[test]
pub(crate) fn schema_v2_legacy_audio_out_remains_strictly_rejected() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut full = runner.config_payload();
    full["runtimeConfig"]["usb"]["audioOut"] = json!("jack");
    assert_rejected_without_byte_changes(&mut runner, full);

    let error = prepare_patch_payload(
        json!({
            "kind": "octessera.patch",
            "schemaVersion": 2,
            "runtimeConfig": { "usb": { "audioOut": "both" } }
        }),
        &runner.config_payload(),
    )
    .unwrap_err();
    assert!(error.contains("runtimeConfig.usb.audioOut"), "{error}");
}

#[test]
pub(crate) fn legacy_audio_out_preparation_preserves_local_patch_audio_and_applies_device_audio() {
    let mut patch_runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    patch_runner.audio_outputs = AudioOutputSet::from_flags(false, true, true).unwrap();
    patch_runner.usb_midi_out_enabled = true;
    let patch = legacy_payload(json!({
        "runtimeConfig": {
            "usb": { "audioOut": "jack", "midiOutEnabled": false },
            "transport": { "bpm": 42 }
        }
    }));
    patch_runner
        .apply_patch_payload_preserving_device(patch)
        .unwrap();
    assert_eq!(
        patch_runner.config_payload()["runtimeConfig"]["audioOutputs"],
        json!({ "dac": false, "usb": true, "hdmi": true })
    );
    assert_eq!(patch_runner.transport.bpm, 42.0);
    assert_eq!(
        patch_runner.config_payload()["runtimeConfig"]["usb"]["midiOutEnabled"],
        true
    );

    let mut device_runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let device = legacy_payload(json!({
        "runtimeConfig": { "usb": { "audioOut": "both" } }
    }));
    device_runner
        .apply_device_config_payload_preserving_patch(device)
        .unwrap();
    assert_eq!(
        device_runner.config_payload()["runtimeConfig"]["audioOutputs"],
        json!({ "dac": true, "usb": true, "hdmi": false })
    );
    assert!(device_runner.config_payload()["runtimeConfig"]["usb"]
        .get("audioOut")
        .is_none());
}

#[test]
pub(crate) fn patch_device_audio_fields_are_ignored_and_local_outputs_survive() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.audio_outputs = AudioOutputSet::from_flags(false, true, true).unwrap();
    runner.usb_midi_out_enabled = true;
    let before = runner.config_payload();
    runner
        .apply_patch_payload_preserving_device(json!({
            "kind": "octessera.patch",
            "schemaVersion": 2,
            "runtimeConfig": {
                "audioOutputs": "obsolete",
                "usb": { "midiOutEnabled": false },
                "transport": { "bpm": 42 }
            }
        }))
        .unwrap();
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["audioOutputs"],
        before["runtimeConfig"]["audioOutputs"]
    );
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["usb"]["midiOutEnabled"],
        true
    );
    assert_eq!(runner.transport.bpm, 42.0);
}

#[test]
pub(crate) fn device_payload_split_keeps_audio_outputs_local() {
    let payload = json!({
        "runtimeConfig": {
            "audioOutputs": { "dac": true, "usb": true, "hdmi": false },
            "masterVolume": 81
        }
    });

    let patch = patch_payload_from_payload(payload.clone()).unwrap();
    assert!(patch["runtimeConfig"]["audioOutputs"].is_null());

    let device = device_config_payload_from_payload(payload).unwrap();
    assert_eq!(
        device["runtimeConfig"]["audioOutputs"],
        json!({ "dac": true, "usb": true, "hdmi": false })
    );
}
