use super::*;
use crate::DspRuntimeConfig;
use serde_json::json;
use std::collections::BTreeMap;

#[test]
fn device_config_reboot_wire_name_deserializes_canonically() {
    let payload = json!({
        "runtimeConfig": {
            "audioOutputs": { "dac": true, "usb": false, "hdmi": false },
            "usb": { "midiOutEnabled": false }
        }
    });
    let request: RuntimePlatformRequest = serde_json::from_value(json!({
        "effect": { "type": "apply_device_config_reboot", "payload": payload },
        "requestId": "request-1"
    }))
    .unwrap();
    assert!(matches!(
        request.effect,
        RuntimePlatformEffect::ApplyDeviceConfigReboot { .. }
    ));
}

#[test]
fn device_config_reboot_wire_name_serializes_canonically() {
    assert_eq!(
        serde_json::to_value(RuntimePlatformEffect::ApplyDeviceConfigReboot {
            payload: json!({
                "runtimeConfig": {
                    "audioOutputs": { "dac": true, "usb": false, "hdmi": false },
                    "usb": { "midiOutEnabled": false }
                }
            }),
        })
        .unwrap(),
        json!({
            "type": "apply_device_config_reboot",
            "payload": {
                "runtimeConfig": {
                    "audioOutputs": { "dac": true, "usb": false, "hdmi": false },
                    "usb": { "midiOutEnabled": false }
                }
            }
        })
    );
}

#[test]
fn every_runtime_audio_command_round_trips_through_json() {
    let params = BTreeMap::from([(String::from("mixPct"), json!(35))]);
    let commands = vec![
        RuntimeAudioCommand::SetAudioConfig {
            revision: 4,
            request_id: Some("audio-4".into()),
            generation: 4,
            config: json!({ "instruments": [] }),
        },
        RuntimeAudioCommand::SetDspConfig {
            generation: 4,
            config: DspRuntimeConfig::default(),
        },
        RuntimeAudioCommand::SetMasterVolume {
            generation: 4,
            volume_pct: 82.0,
        },
        RuntimeAudioCommand::SetInstrumentMixer {
            instrument_slot: 1,
            generation: 1,
            volume_pct: Some(74.0),
            pan_pos: Some(16),
        },
        RuntimeAudioCommand::SetInstrumentSlot {
            instrument_slot: 2,
            generation: 1,
            config: json!({ "type": "synth" }),
        },
        RuntimeAudioCommand::SetFxBusMixer {
            bus_index: 2,
            generation: 1,
            pan_pos: Some(12),
            volume_pct: Some(66.0),
        },
        RuntimeAudioCommand::SetSynthParam {
            instrument_slot: 3,
            generation: 1,
            path: "synth.filter.cutoffHz".into(),
            value: 440.0,
        },
        RuntimeAudioCommand::SetSampleBankParam {
            instrument_slot: 4,
            generation: 1,
            path: "sample.tuneSemis".into(),
            value: 2.0,
        },
        RuntimeAudioCommand::SetFxBusParam {
            bus_index: 1,
            slot_index: 0,
            generation: 1,
            param: realtime_engine::synth::FxParamId::MixPct,
            value: 0.35,
        },
        RuntimeAudioCommand::SetFxBusSlot {
            bus_index: 1,
            slot_index: 0,
            generation: 1,
            fx_type: "delay".into(),
            params: params.clone(),
        },
        RuntimeAudioCommand::SetGlobalFxSlot {
            slot_index: 1,
            generation: 1,
            fx_type: "compressor".into(),
            params: params.clone(),
        },
        RuntimeAudioCommand::SetGlobalFxParam {
            slot_index: 1,
            generation: 1,
            param: realtime_engine::synth::FxParamId::MixPct,
            value: 0.35,
        },
        RuntimeAudioCommand::MomentaryFxStart {
            id: "spark:0".into(),
            epoch: 1,
            fx_type: "freeze".into(),
            params: params.clone(),
            target: RuntimeMomentaryFxTarget::Global,
        },
        RuntimeAudioCommand::MomentaryFxUpdate {
            id: "spark:0".into(),
            epoch: 1,
            params: params.clone(),
        },
        RuntimeAudioCommand::MomentaryFxStop {
            id: "spark:0".into(),
            epoch: 1,
        },
        RuntimeAudioCommand::SamplePreview {
            instrument_slot: 5,
            sample_slot: 2,
            path: "kits/hat.wav".into(),
            velocity: 96,
        },
    ];

    for command in commands {
        let encoded = serde_json::to_value(&command).unwrap();
        assert_eq!(
            serde_json::from_value::<RuntimeAudioCommand>(encoded).unwrap(),
            command
        );
    }
}

#[test]
fn global_fx_audio_commands_require_generation() {
    for payload in [
        json!({
            "type": "set_global_fx_slot",
            "slotIndex": 0,
            "fxType": "delay",
            "params": {}
        }),
        json!({
            "type": "set_global_fx_param",
            "slotIndex": 0,
            "param": "mixPct",
            "value": 0.5
        }),
    ] {
        assert!(serde_json::from_value::<RuntimeAudioCommand>(payload).is_err());
    }
}
