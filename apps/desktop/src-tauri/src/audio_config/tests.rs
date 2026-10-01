use super::*;
use realtime_engine::synth::FxBusSlotConfig;

#[test]
fn desktop_uses_shared_audio_normalization_and_sample_metadata() {
    let config = normalize_config(&serde_json::json!({
        "masterVolume": 81,
        "voiceStealingMode": "fixed12",
        "instruments": [{
            "type": "sampler",
            "sample": {
                "slots": [{ "path": "kick.wav" }],
                "tuneSemis": -5,
                "amp": { "gainPct": 70, "velocitySensitivityPct": 40 },
                "filter": { "cutoffHz": 6400, "resonance": 35 }
            }
        }],
        "mixer": { "buses": [{ "slot3": { "type": "tremolo" } }] }
    }))
    .unwrap();

    assert_eq!(config.master_volume, 81.0);
    assert_eq!(
        config.instruments[0].active_sample().unwrap().tune_semis,
        -5.0
    );
    assert!(matches!(
        config.mixer.as_ref().unwrap().buses[0].slots[2],
        FxBusSlotConfig::Config { ref kind, .. } if kind == "tremolo"
    ));

    let banks = sample_banks(
        &config,
        |path| Some(format!("resolved/{path}")),
        |path| {
            assert_eq!(path, "resolved/kick.wav");
            Some(SampleBuffer {
                samples: vec![0.5, -0.5].into(),
                channels: 1,
                sample_rate: 48_000,
            })
        },
    )
    .unwrap();
    assert_eq!(banks[0].tune_semis, -5.0);
    assert!(banks[0].slots[0].buffer.is_some());
}

#[test]
fn desktop_rejects_the_same_malformed_fx_payload_as_pi() {
    let error = normalize_config(&serde_json::json!({
        "instruments": [{ "type": "synth" }],
        "mixer": { "buses": [{ "slot1": { "params": {} } }] }
    }))
    .unwrap_err();

    assert!(error.contains("invalid mixer bus 1 slot 1"), "{error}");
}

#[test]
fn desktop_reports_undecodable_sample_as_typed_failure() {
    let config = normalize_config(&serde_json::json!({
        "instruments": [{
            "type": "sampler",
            "sample": { "slots": [{ "path": "kick.wav" }] }
        }]
    }))
    .unwrap();

    let error = sample_banks(&config, |_| Some("resolved/kick.wav".into()), |_| None).unwrap_err();
    assert_eq!(
        error.code(),
        playback_runtime::RuntimeErrorCode::OperationFailed
    );
    assert_eq!(error.message(), "sample decode failed: kick.wav");
}

#[test]
fn sample_bank_signature_tracks_only_sampler_changes() {
    let first = normalize_config(&serde_json::json!({
        "instruments": [{ "type": "synth" }, {
            "type": "sampler",
            "sample": { "slots": [{ "path": "kick.wav" }] }
        }]
    }))
    .unwrap();
    let changed = normalize_config(&serde_json::json!({
        "instruments": [{ "type": "synth", "synth": {
            "osc1": { "waveform": "saw", "levelPct": 80, "octave": 0, "detuneCents": 0, "pulseWidthPct": 50 },
            "osc2": { "waveform": "square", "levelPct": 80, "octave": 0, "detuneCents": 0, "pulseWidthPct": 50 },
            "amp": { "gainPct": 80, "velocitySensitivityPct": 100 },
            "ampEnv": { "attackMs": 5, "decayMs": 120, "sustainPct": 70, "releaseMs": 180 },
            "filter": { "type": "lowpass", "cutoffHz": 120, "resonance": 20, "envAmountPct": 0, "keyTrackingPct": 0 },
            "filterEnv": { "attackMs": 5, "decayMs": 120, "sustainPct": 70, "releaseMs": 180 }
        } }, {
            "type": "sampler",
            "sample": { "slots": [{ "path": "kick.wav" }], "tuneSemis": 2 }
        }]
    }))
    .unwrap();

    assert_ne!(
        sample_bank_signature(&first),
        sample_bank_signature(&changed)
    );
}
