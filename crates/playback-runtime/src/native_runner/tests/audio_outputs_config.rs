use super::*;

#[test]
pub(crate) fn public_runtime_audio_outputs_decoder_accepts_canonical_form() {
    assert_eq!(
        AudioOutputSet::decode(&json!({ "dac": false, "usb": true, "hdmi": true })).unwrap(),
        AudioOutputSet::from_flags(false, true, true).unwrap()
    );
}

#[test]
pub(crate) fn public_runtime_audio_outputs_decoder_rejects_malformed_forms() {
    assert!(AudioOutputSet::decode(&json!({})).is_err());
    assert!(AudioOutputSet::decode(&json!({ "dac": true, "usb": false })).is_err());
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
