use super::*;

#[test]
pub(crate) fn public_runtime_audio_outputs_decoder_accepts_canonical_and_legacy_forms() {
    assert_eq!(
        AudioOutputSet::decode_runtime_config(&json!({
            "runtimeConfig": {
                "audioOutputs": { "dac": false, "usb": true, "hdmi": true }
            }
        }))
        .unwrap(),
        AudioOutputSet::from_flags(false, true, true).unwrap()
    );
    assert_eq!(
        AudioOutputSet::decode_runtime_config(&json!({
            "usb": { "audioOut": "both" }
        }))
        .unwrap(),
        AudioOutputSet::from_flags(true, true, false).unwrap()
    );
}

#[test]
pub(crate) fn public_runtime_audio_outputs_decoder_rejects_missing_or_conflicting_forms() {
    assert!(AudioOutputSet::decode_runtime_config(&json!({})).is_err());
    assert!(AudioOutputSet::decode_runtime_config(&json!({
        "runtimeConfig": {
            "audioOutputs": { "dac": true, "usb": false, "hdmi": true },
            "usb": { "audioOut": "usb" }
        }
    }))
    .is_err());
}

#[test]
pub(crate) fn all_valid_audio_output_sets_round_trip_without_legacy_audio_out() {
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
        assert!(runner.config_payload()["runtimeConfig"]["usb"]
            .as_object()
            .unwrap()
            .get("audioOut")
            .is_none());
    }
}

#[test]
pub(crate) fn current_schema_rejects_malformed_device_audio_outputs() {
    for audio_outputs in [
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
pub(crate) fn current_schema_rejects_conflicting_device_audio_outputs() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["audioOutputs"] = json!({
        "dac": true,
        "usb": false,
        "hdmi": false
    });
    payload["runtimeConfig"]["usb"]["audioOut"] = json!("usb");

    assert_rejected_without_byte_changes(&mut runner, payload);
}

#[test]
pub(crate) fn legacy_audio_out_migrates_strictly_and_rejects_conflicts() {
    for (legacy, expected) in [
        ("jack", json!({ "dac": true, "usb": false, "hdmi": false })),
        ("usb", json!({ "dac": false, "usb": true, "hdmi": false })),
        ("both", json!({ "dac": true, "usb": true, "hdmi": false })),
    ] {
        let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
        let mut payload = runner.config_payload();
        payload["runtimeConfig"]
            .as_object_mut()
            .unwrap()
            .remove("audioOutputs");
        payload["runtimeConfig"]["usb"]["audioOut"] = json!(legacy);
        runner.apply_config_payload(payload).unwrap();
        assert_eq!(
            runner.config_payload()["runtimeConfig"]["audioOutputs"],
            expected
        );
    }
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut legacy_only = json!({
        "runtimeConfig": { "usb": { "audioOut": "usb" } }
    });
    runner.apply_config_payload(legacy_only.take()).unwrap();
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["audioOutputs"],
        json!({ "dac": false, "usb": true, "hdmi": false })
    );
    for legacy in ["unknown", "uac2"] {
        let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
        let mut payload = runner.config_payload();
        payload["runtimeConfig"]["usb"]["audioOut"] = json!(legacy);
        assert_rejected_without_byte_changes(&mut runner, payload);
    }
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["audioOutputs"] = json!({ "dac": false, "usb": false, "hdmi": true });
    payload["runtimeConfig"]["usb"]["audioOut"] = json!("jack");
    assert_rejected_without_byte_changes(&mut runner, payload);
}

#[test]
pub(crate) fn patch_device_audio_fields_are_ignored_and_local_outputs_survive() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.audio_outputs = AudioOutputSet::from_flags(false, true, true).unwrap();
    let before = runner.config_payload();
    runner
        .apply_patch_payload_preserving_device(json!({
            "kind": "octessera.patch",
            "schemaVersion": 2,
            "runtimeConfig": {
                "audioOutputs": "obsolete",
                "usb": { "audioOut": "jack" },
                "transport": { "bpm": 42 }
            }
        }))
        .unwrap();
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["audioOutputs"],
        before["runtimeConfig"]["audioOutputs"]
    );
    assert_eq!(runner.transport.bpm, 42.0);
}

#[test]
pub(crate) fn mixed_audio_outputs_compare_only_dac_usb_projection() {
    for (canonical, legacy) in [
        (json!({ "dac": true, "usb": false, "hdmi": true }), "jack"),
        (json!({ "dac": false, "usb": true, "hdmi": true }), "usb"),
        (json!({ "dac": true, "usb": true, "hdmi": true }), "both"),
    ] {
        let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
        let mut payload = runner.config_payload();
        payload["runtimeConfig"]["audioOutputs"] = canonical;
        payload["runtimeConfig"]["usb"]["audioOut"] = json!(legacy);
        runner.apply_config_payload(payload).unwrap();
    }
    for (canonical, legacy) in [
        (json!({ "dac": false, "usb": true, "hdmi": false }), "jack"),
        (json!({ "dac": true, "usb": false, "hdmi": false }), "usb"),
        (json!({ "dac": false, "usb": false, "hdmi": true }), "both"),
    ] {
        let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
        let mut payload = runner.config_payload();
        payload["runtimeConfig"]["audioOutputs"] = canonical;
        payload["runtimeConfig"]["usb"]["audioOut"] = json!(legacy);
        assert_rejected_without_byte_changes(&mut runner, payload);
    }
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
