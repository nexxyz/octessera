use super::*;
use crate::DspRuntimeConfig;
use realtime_engine::synth::{
    FxParamId, BUS_COUNT, INSTRUMENT_SLOT_COUNT, SAMPLE_SLOTS_PER_INSTRUMENT,
};
use serde_json::json;
use std::collections::BTreeMap;

fn invalid_code(command: &RuntimeAudioCommand) -> RuntimeErrorFacts {
    let error = validate_audio_command(command).unwrap_err();
    assert_eq!(error.facts.domain, RuntimeErrorDomain::Audio, "{command:?}");
    assert_eq!(
        error.facts.code,
        RuntimeErrorCode::InvalidPayload,
        "{command:?}"
    );
    assert_eq!(
        error.facts.operation,
        RuntimeOperation::AudioCommand,
        "{command:?}"
    );
    error.facts
}

fn fx_bus_slot(fx_type: &str, params: serde_json::Value) -> RuntimeAudioCommand {
    RuntimeAudioCommand::SetFxBusSlot {
        bus_index: 2,
        slot_index: 0,
        generation: 17,
        fx_type: fx_type.into(),
        params: serde_json::from_value(params).unwrap(),
    }
}

#[test]
fn accepts_every_valid_runtime_audio_command_shape() {
    let params = BTreeMap::new();
    let commands = vec![
        RuntimeAudioCommand::SetAudioConfig {
            revision: 1,
            request_id: None,
            generation: 0,
            config: json!({ "instruments": [] }),
        },
        RuntimeAudioCommand::SetDspConfig {
            generation: 0,
            config: DspRuntimeConfig::default(),
        },
        RuntimeAudioCommand::SetMasterVolume {
            generation: 0,
            volume_pct: 80.0,
        },
        RuntimeAudioCommand::SetInstrumentMixer {
            instrument_slot: 0,
            generation: 0,
            volume_pct: Some(80.0),
            pan_pos: Some(16),
        },
        RuntimeAudioCommand::SetInstrumentSlot {
            instrument_slot: 0,
            generation: 0,
            config: json!({ "type": "synth" }),
        },
        RuntimeAudioCommand::SetFxBusMixer {
            bus_index: 0,
            generation: 0,
            pan_pos: Some(16),
            volume_pct: Some(80.0),
        },
        RuntimeAudioCommand::SetSynthParam {
            instrument_slot: 0,
            generation: 0,
            path: "synth.filter.cutoffHz".into(),
            value: 440.0,
        },
        RuntimeAudioCommand::SetSynthParam {
            instrument_slot: 0,
            generation: 0,
            path: "synth.osc1.detuneCents".into(),
            value: -12.0,
        },
        RuntimeAudioCommand::SetFmParam {
            instrument_slot: 0,
            generation: 0,
            path: "fm.indexEnv.decayMs".into(),
            value: 250.0,
        },
        RuntimeAudioCommand::SetPluckParam {
            instrument_slot: 0,
            generation: 0,
            path: "pluck.brightnessPct".into(),
            value: 65.0,
        },
        RuntimeAudioCommand::SetDrumParam {
            instrument_slot: 0,
            voice: 4,
            generation: 0,
            path: "drum.decayMs".into(),
            value: 350.0,
        },
        RuntimeAudioCommand::SetSampleBankParam {
            instrument_slot: 0,
            generation: 0,
            path: "sample.tuneSemis".into(),
            value: 0.0,
        },
        RuntimeAudioCommand::SetFxBusParam {
            bus_index: 0,
            slot_index: 0,
            generation: 0,
            param: FxParamId::MixPct,
            value: 50.0,
        },
        RuntimeAudioCommand::SetFxBusSlot {
            bus_index: 0,
            slot_index: 0,
            generation: 0,
            fx_type: "delay".into(),
            params: params.clone(),
        },
        RuntimeAudioCommand::SetGlobalFxSlot {
            slot_index: 0,
            generation: 0,
            fx_type: "eq".into(),
            params: params.clone(),
        },
        RuntimeAudioCommand::SetGlobalFxParam {
            slot_index: 0,
            generation: 0,
            param: FxParamId::MixPct,
            value: 50.0,
        },
        RuntimeAudioCommand::MomentaryFxStart {
            id: "fx".into(),
            epoch: 1,
            fx_type: "freeze".into(),
            params: params.clone(),
            target: RuntimeMomentaryFxTarget::Global,
        },
        RuntimeAudioCommand::MomentaryFxUpdate {
            id: "fx".into(),
            epoch: 1,
            params: params.clone(),
        },
        RuntimeAudioCommand::MomentaryFxStop {
            id: "fx".into(),
            epoch: 1,
        },
        RuntimeAudioCommand::SamplePreview {
            instrument_slot: 0,
            sample_slot: SAMPLE_SLOTS_PER_INSTRUMENT - 1,
            path: "kick.wav".into(),
            velocity: 96,
        },
    ];
    for command in commands {
        validate_audio_command(&command).unwrap();
    }
}

#[test]
fn rejects_invalid_paths_and_nonfinite_scalar_values() {
    let commands = |path: &str, value: f32| {
        [
            RuntimeAudioCommand::SetSynthParam {
                instrument_slot: 0,
                generation: 0,
                path: path.into(),
                value,
            },
            RuntimeAudioCommand::SetFmParam {
                instrument_slot: 0,
                generation: 0,
                path: path.into(),
                value,
            },
            RuntimeAudioCommand::SetPluckParam {
                instrument_slot: 0,
                generation: 0,
                path: path.into(),
                value,
            },
            RuntimeAudioCommand::SetSampleBankParam {
                instrument_slot: 0,
                generation: 0,
                path: path.into(),
                value,
            },
        ]
    };
    for command in commands("synth.amp.gainPct.nope", 1.0) {
        invalid_code(&command);
    }
    for (index, path) in [
        "synth.filter.cutoffHz",
        "fm.index",
        "pluck.decayMs",
        "sample.tuneSemis",
    ]
    .into_iter()
    .enumerate()
    {
        let command = commands(path, f32::NAN).into_iter().nth(index).unwrap();
        let facts = invalid_code(&command);
        assert!(facts.message.unwrap().ends_with("must be finite"));
    }
    invalid_code(&RuntimeAudioCommand::SetMasterVolume {
        generation: 0,
        volume_pct: f32::NAN,
    });
}

#[test]
fn rejects_invalid_drum_voice_path_and_nonfinite_value() {
    for (voice, path, value) in [
        (8, "drum.decayMs", 250.0),
        (1, "drum.amp.gainPct", 70.0),
        (0, "drum.voices.0.decayMs", 250.0),
        (0, "drum.filter.cutoffHz", f32::NAN),
    ] {
        invalid_code(&RuntimeAudioCommand::SetDrumParam {
            instrument_slot: 0,
            voice,
            generation: 0,
            path: path.into(),
            value,
        });
    }
}

#[test]
fn rejects_out_of_range_slot_and_bus_indices() {
    let facts = invalid_code(&RuntimeAudioCommand::SetFxBusSlot {
        bus_index: BUS_COUNT,
        slot_index: 0,
        generation: 0,
        fx_type: "delay".into(),
        params: BTreeMap::new(),
    });
    assert_eq!(
        facts.message.as_deref(),
        Some(format!("FX bus index is out of range: {BUS_COUNT}").as_str())
    );
    invalid_code(&RuntimeAudioCommand::SetInstrumentMixer {
        instrument_slot: INSTRUMENT_SLOT_COUNT,
        generation: 0,
        volume_pct: Some(80.0),
        pan_pos: None,
    });
    invalid_code(&RuntimeAudioCommand::MomentaryFxStart {
        id: "fx".into(),
        epoch: 1,
        fx_type: "freeze".into(),
        params: BTreeMap::new(),
        target: RuntimeMomentaryFxTarget::FxBus { index: BUS_COUNT },
    });
}

#[test]
fn sample_preview_rejects_out_of_range_sample_slot() {
    let facts = invalid_code(&RuntimeAudioCommand::SamplePreview {
        instrument_slot: 0,
        sample_slot: SAMPLE_SLOTS_PER_INSTRUMENT,
        path: "kick.wav".into(),
        velocity: 96,
    });
    assert_eq!(
        facts.message.as_deref(),
        Some(format!("sample slot is out of range: {SAMPLE_SLOTS_PER_INSTRUMENT}").as_str())
    );
}

#[test]
fn instrument_slot_must_be_a_parseable_object() {
    for config in [
        json!([]),
        json!({ "type": 7 }),
        json!({ "volumePct": "loud" }),
    ] {
        invalid_code(&RuntimeAudioCommand::SetInstrumentSlot {
            instrument_slot: 0,
            generation: 0,
            config,
        });
    }
}

#[test]
fn unknown_fx_type_is_invalid_payload() {
    invalid_code(&fx_bus_slot("unknown", json!({})));
    invalid_code(&RuntimeAudioCommand::SetGlobalFxSlot {
        slot_index: 0,
        generation: 0,
        fx_type: "unknown".into(),
        params: BTreeMap::new(),
    });
}

#[test]
fn accepts_duck_slot_with_source_and_tap_strings() {
    validate_audio_command(&fx_bus_slot(
        "duck",
        json!({ "amountPct": 60, "attackMs": 8, "releaseMs": 160, "source": "I1", "sourceTap": "pre", "threshold": 0.08 }),
    ))
    .unwrap();
}

#[test]
fn rejects_invalid_duck_strings_and_strings_on_other_fx() {
    for (fx_type, params) in [
        ("duck", json!({ "source": "I99" })),
        ("duck", json!({ "source": "B9" })),
        ("duck", json!({ "sourceTap": "sideways" })),
        ("delay", json!({ "source": "I1" })),
    ] {
        invalid_code(&fx_bus_slot(fx_type, params));
    }
}
