use super::*;

fn assert_validation_error<F>(field: &str, expected: &str, mutate: F)
where
    F: FnOnce(&mut Value),
{
    let mut payload: Value = serde_json::from_str(include_str!(
        "../../../../../config/generated/desktop/default.json"
    ))
    .unwrap();
    mutate(&mut payload);
    let error = validate_config_payload(&payload).unwrap_err();
    assert_eq!(error, expected, "validation field: {field}");
}

#[test]
pub(crate) fn validation_matrix_preserves_domain_fields_and_error_paths() {
    assert_validation_error(
        "canonical masterVolume",
        "configuration.runtimeConfig.masterVolume is outside the supported range",
        |payload| payload["runtimeConfig"]["masterVolume"] = json!(101),
    );
    assert_validation_error(
        "instrument type",
        "runtimeConfig.instruments[0].type has unknown value `broken`",
        |payload| payload["runtimeConfig"]["instruments"][0]["type"] = json!("broken"),
    );
    assert_validation_error(
        "sample slot path",
        "configuration.runtimeConfig.instruments[0].sample.slots[0].path must be a string or null",
        |payload| {
            payload["runtimeConfig"]["instruments"][0]["sample"]["slots"][0]["path"] = json!(1)
        },
    );
    assert_validation_error(
        "synth waveform",
        "runtimeConfig.instruments[0].synth.osc1.waveform has unknown value `broken`",
        |payload| {
            payload["runtimeConfig"]["instruments"][0]["synth"]["osc1"]["waveform"] =
                json!("broken")
        },
    );
    assert_validation_error(
        "sound note length",
        "runtimeConfig.sound.noteLengthMs is outside the supported range",
        |payload| payload["runtimeConfig"]["sound"]["noteLengthMs"] = json!(29),
    );
    assert_validation_error(
        "mixer bus volume",
        "runtimeConfig.mixer.buses[0].volumePct is outside the supported range",
        |payload| payload["runtimeConfig"]["mixer"]["buses"][0]["volumePct"] = json!(101),
    );
    assert_validation_error(
        "mixer FX slot",
        "runtimeConfig.mixer.buses[0].slot1.type has unknown FX slot `broken`",
        |payload| payload["runtimeConfig"]["mixer"]["buses"][0]["slot1"]["type"] = json!("broken"),
    );
    assert_validation_error(
        "MIDI sync mode",
        "runtimeConfig.midi.syncMode has unknown value `broken`",
        |payload| payload["runtimeConfig"]["midi"]["syncMode"] = json!("broken"),
    );
    assert_validation_error(
        "USB Device MIDI",
        "runtimeConfig.usb.midiOutEnabled must be a boolean",
        |payload| payload["runtimeConfig"]["usb"]["midiOutEnabled"] = json!("yes"),
    );
    assert_validation_error(
        "USB data role",
        "runtimeConfig.usb.dataRole has unknown value `invalid`",
        |payload| payload["runtimeConfig"]["usb"]["dataRole"] = json!("invalid"),
    );
    assert_validation_error(
        "Host USB audio",
        "runtimeConfig.audioOutputs.usb must be false when runtimeConfig.usb.dataRole is host",
        |payload| {
            payload["runtimeConfig"]["usb"]["dataRole"] = json!("host");
            payload["runtimeConfig"]["audioOutputs"]["usb"] = json!(true);
        },
    );
    assert_validation_error(
        "Host USB MIDI",
        "runtimeConfig.usb.midiOutEnabled must be false when runtimeConfig.usb.dataRole is host",
        |payload| {
            payload["runtimeConfig"]["usb"]["dataRole"] = json!("host");
            payload["runtimeConfig"]["usb"]["midiOutEnabled"] = json!(true);
        },
    );
    assert_validation_error(
        "audio outputs",
        "runtimeConfig.audioOutputs must contain exactly boolean dac, usb, and hdmi fields",
        |payload| payload["runtimeConfig"]["audioOutputs"] = json!({"dac": true}),
    );
    assert_validation_error(
        "HDMI mode",
        "runtimeConfig.hdmi.mode has unknown value `broken`",
        |payload| payload["runtimeConfig"]["hdmi"]["mode"] = json!("broken"),
    );
    assert_validation_error(
        "recording duration",
        "runtimeConfig.recording.maxMinutes is outside the supported range",
        |payload| payload["runtimeConfig"]["recording"]["maxMinutes"] = json!(121),
    );
    assert_validation_error(
        "global modulation transient",
        "runtimeConfig.linkLfos[0].phasePulses is transient and cannot be serialized",
        |payload| payload["runtimeConfig"]["linkLfos"][0]["phasePulses"] = json!(1),
    );
    assert_validation_error(
        "global modulation target",
        "runtimeConfig.linkLfos[0].target is not additive and live-safe",
        |payload| {
            payload["runtimeConfig"]["linkLfos"][0]["target"] = json!({
                "kind": "number",
                "key": "mixer.buses.0.slot1.params.timeMs"
            })
        },
    );
    assert_validation_error(
        "layer parameter modulation",
        "runtimeConfig.layers[0].paramMods.x[0].key is unsupported",
        |payload| {
            payload["runtimeConfig"]["layers"][0]["paramMods"]["x"][0]["key"] = json!("unsupported")
        },
    );
    assert_validation_error(
        "aux binding",
        "runtimeConfig.auxBindings.aux1.turnKey is unsupported",
        |payload| payload["runtimeConfig"]["auxBindings"]["aux1"]["turnKey"] = json!("unsupported"),
    );
    assert_validation_error(
        "pulse mapping",
        "runtimeConfig.layers[0].link.mapping.activate.action has unknown value `broken`",
        |payload| {
            payload["runtimeConfig"]["layers"][0]["link"]["mapping"]["activate"]["action"] =
                json!("broken")
        },
    );
    assert_validation_error(
        "root mapping scale",
        "mappingConfig.scale[0] is outside the supported range",
        |payload| payload["mappingConfig"]["scale"][0] = json!(12),
    );
    assert_validation_error(
        "active behavior",
        "runtimeConfig.activeBehavior has unknown behavior `broken`",
        |payload| payload["runtimeConfig"]["activeBehavior"] = json!("broken"),
    );
    assert_validation_error(
        "layer behavior",
        "runtimeConfig.layers[0].build.behaviorId has unknown behavior `broken`",
        |payload| payload["runtimeConfig"]["layers"][0]["build"]["behaviorId"] = json!("broken"),
    );
    assert_validation_error(
        "pulse scan sections",
        "runtimeConfig.layers[0].link.scanSections is unsupported",
        |payload| payload["runtimeConfig"]["layers"][0]["link"]["scanSections"] = json!(3),
    );
    assert_validation_error(
        "transport swing",
        "configuration.runtimeConfig.transport.swingPct is outside the supported range",
        |payload| payload["runtimeConfig"]["transport"]["swingPct"] = json!(76),
    );
    assert_validation_error(
        "Play FX type",
        "runtimeConfig.playFx.selected.fxType has unknown value `broken`",
        |payload| payload["runtimeConfig"]["playFx"]["selected"]["fxType"] = json!("broken"),
    );
    assert_validation_error(
        "Pitch slide-in range",
        "runtimeConfig.playFx.selected.params.slideInMs is outside the supported range",
        |payload| {
            payload["runtimeConfig"]["playFx"]["selected"]["fxType"] = json!("pitch_shift");
            payload["runtimeConfig"]["playFx"]["selected"]["params"] = json!({ "slideInMs": 9 });
        },
    );
    assert_validation_error(
        "system Play mode",
        "configuration.system.playMode has unknown value `broken`",
        |payload| payload["system"]["playMode"] = json!("broken"),
    );
}
