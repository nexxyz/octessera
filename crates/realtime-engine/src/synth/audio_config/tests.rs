use super::*;
use crate::synth::DrumSound;

#[test]
fn normalizes_shared_config_and_preserves_sample_paths() {
    let config = normalize_audio_config(&serde_json::json!({
        "masterVolume": 81,
        "voiceStealingMode": "fixed12",
        "instruments": [{
            "type": "sampler",
            "sample": { "slots": [{ "path": "kits/kick.wav" }] }
        }],
        "mixer": { "buses": [{ "slot3": { "type": "tremolo" } }] }
    }))
    .unwrap();

    assert_eq!(config.master_volume, 81.0);
    assert_eq!(config.voice_stealing_mode, Some(VoiceStealingMode::Fixed12));
    assert_eq!(
        config.instruments[0].active_sample().unwrap().slots[0],
        Some("kits/kick.wav".into())
    );
    assert!(matches!(
        config.mixer.as_ref().unwrap().buses[0].slots[2],
        FxBusSlotConfig::Config { ref kind, .. } if kind == "tremolo"
    ));
}

#[test]
fn rejects_malformed_and_unknown_fx_slots() {
    for value in [
        serde_json::json!({ "params": {} }),
        serde_json::json!({ "type": "unknown" }),
        serde_json::json!(42),
    ] {
        assert!(normalize_fx_slot(&value).is_err());
    }
}

#[test]
fn normalizes_instrument_slot_with_shared_defaults() {
    let slot = normalize_instrument_slot_config(&serde_json::json!({ "type": "synth" })).unwrap();
    assert_eq!(slot.slot.kind, "synth");
    assert!(slot.slot.fm.is_none());
    assert_eq!(slot.slot.mixer.unwrap().route, "direct");
}

#[test]
fn fm_slot_defaults_without_block_and_rejects_unsupported_present_kind() {
    let slot = normalize_instrument_slot_config(&serde_json::json!({ "type": "fm" })).unwrap();
    assert!(slot.slot.fm.is_none());
    let fm = slot.slot.fm.unwrap_or_default();
    assert_eq!(fm.ratio, super::super::types::FmRatio::Two);
    assert_eq!(fm.index, 50);
    assert_eq!(fm.ratio_fine_cents, 0);
    assert_eq!(fm.velocity_to_index_pct, 0);
    assert_eq!(fm.mod_shape_pct, 0);
    assert_eq!(fm.mod_mix_pct, 0);
    assert_eq!(fm.index_env.decay_ms, 250.0);
    assert_eq!(fm.amp_env.release_ms, 350.0);
    assert_eq!(fm.filter.cutoff_hz, default_synth_config().filter.cutoff_hz);
    assert!(normalize_instrument_slot_config(&serde_json::json!({"type":"unsupported"})).is_err());
    assert!(
        normalize_audio_config(&serde_json::json!({"instruments":[{"type":"unknown"}]})).is_err()
    );
}

#[test]
fn fm_new_numeric_parameters_default_and_reject_invalid_values() {
    let old = normalize_instrument_slot_config(&serde_json::json!({
        "type":"fm", "fm":{"ratio":"2","index":50}
    }))
    .unwrap()
    .slot
    .fm
    .unwrap();
    let neutral = normalize_instrument_slot_config(&serde_json::json!({
        "type":"fm", "fm":{"ratio":"2","index":50,"ratioFineCents":0,
        "velocityToIndexPct":0,"modShapePct":0,"modMixPct":0}
    }))
    .unwrap()
    .slot
    .fm
    .unwrap();
    assert_eq!(old.ratio_fine_cents, neutral.ratio_fine_cents);
    assert_eq!(old.velocity_to_index_pct, neutral.velocity_to_index_pct);
    assert_eq!(old.mod_shape_pct, neutral.mod_shape_pct);
    assert_eq!(old.mod_mix_pct, neutral.mod_mix_pct);
    for (key, value) in [
        ("ratioFineCents", serde_json::json!(-101)),
        ("ratioFineCents", serde_json::json!(101)),
        ("ratioFineCents", serde_json::json!(0.5)),
        ("ratioFineCents", serde_json::Value::Null),
        ("velocityToIndexPct", serde_json::json!(101)),
        ("velocityToIndexPct", serde_json::json!(null)),
        ("modShapePct", serde_json::json!(101)),
        ("modMixPct", serde_json::json!(101)),
        ("modMixPct", serde_json::json!(1e100)),
    ] {
        let mut fm = serde_json::Map::new();
        fm.insert(key.into(), value);
        assert!(
            normalize_instrument_slot_config(&serde_json::json!({
                "type":"fm", "fm":fm
            }))
            .is_err(),
            "accepted {key}"
        );
    }
}

#[test]
fn fm_ratio_and_index_are_bounded_and_legacy_synth_config_is_unchanged() {
    for ratio in ["0.5", "1", "2", "3", "4", "5", "6", "8"] {
        let slot = normalize_instrument_slot_config(&serde_json::json!({
            "type":"fm", "fm":{"ratio":ratio,"index":100}
        }))
        .unwrap();
        assert_eq!(slot.slot.fm.unwrap().index, 100);
    }
    for fm in [
        serde_json::json!({"ratio":"7"}),
        serde_json::json!({"index":101}),
        serde_json::json!({"index":0.5}),
        serde_json::json!({"amp":{"gainPct":1e100,"velocitySensitivityPct":100}}),
    ] {
        assert!(
            normalize_instrument_slot_config(&serde_json::json!({"type":"fm","fm":fm})).is_err()
        );
    }
    assert!(
        normalize_instrument_slot_config(&serde_json::json!({"type":"synth"}))
            .unwrap()
            .slot
            .fm
            .is_none()
    );
}

#[test]
fn pluck_slot_defaults_and_numeric_validation_preserve_other_sources() {
    let slot = normalize_instrument_slot_config(&serde_json::json!({"type":"pluck"})).unwrap();
    let pluck = slot.slot.pluck.unwrap_or_default();
    assert_eq!(pluck.decay_ms, 1500.0);
    assert_eq!(pluck.brightness_pct, 65.0);
    assert_eq!(pluck.pick_position_pct, 25.0);
    assert_eq!(pluck.pick_depth_pct, 65);
    assert_eq!(pluck.dispersion_pct, 0);
    assert_eq!(pluck.body_amount_pct, 0);
    assert_eq!(pluck.body_frequency_hz, 500);
    assert_eq!(pluck.amp.gain_pct, 80.0);
    assert_eq!(
        (
            pluck.amp_env.attack_ms,
            pluck.amp_env.decay_ms,
            pluck.amp_env.sustain_pct,
            pluck.amp_env.release_ms
        ),
        (0.0, 0.0, 100.0, 900.0)
    );
    assert_eq!(
        pluck.filter.cutoff_hz,
        default_synth_config().filter.cutoff_hz
    );
    for (field, value) in [
        ("decayMs", serde_json::json!(99)),
        ("decayMs", serde_json::json!(5001)),
        ("brightnessPct", serde_json::json!(101)),
        ("pickPositionPct", serde_json::json!(4)),
        ("pickPositionPct", serde_json::json!(51)),
        ("brightnessPct", serde_json::json!(1e100)),
    ] {
        assert!(
            normalize_instrument_slot_config(&serde_json::json!({
                "type":"pluck", "pluck":{(field): value}
            }))
            .is_err(),
            "{field}: {value}"
        );
    }
    assert!(
        normalize_instrument_slot_config(&serde_json::json!({"type":"synth"}))
            .unwrap()
            .slot
            .pluck
            .is_none()
    );
    assert!(
        normalize_instrument_slot_config(&serde_json::json!({"type":"fm"}))
            .unwrap()
            .slot
            .pluck
            .is_none()
    );
    assert!(normalize_instrument_slot_config(&serde_json::json!({"type":"unknown"})).is_err());
}

#[test]
fn pluck_new_parameters_default_for_old_payloads_and_reject_invalid_values() {
    let old: PluckConfig = serde_json::from_value(serde_json::json!({
        "decayMs":1500,"brightnessPct":65,"pickPositionPct":25
    }))
    .unwrap();
    let explicit: PluckConfig = serde_json::from_value(serde_json::json!({
        "decayMs":1500,"brightnessPct":65,"pickPositionPct":25,
        "pickDepthPct":65,"dispersionPct":0,"bodyAmountPct":0,"bodyFrequencyHz":500
    }))
    .unwrap();
    assert_eq!(old.pick_depth_pct, explicit.pick_depth_pct);
    assert_eq!(old.dispersion_pct, explicit.dispersion_pct);
    assert_eq!(old.body_amount_pct, explicit.body_amount_pct);
    assert_eq!(old.body_frequency_hz, explicit.body_frequency_hz);
    for (key, value) in [
        ("pickDepthPct", serde_json::json!(101)),
        ("pickDepthPct", serde_json::json!(null)),
        ("dispersionPct", serde_json::json!(101)),
        ("dispersionPct", serde_json::json!(0.5)),
        ("bodyAmountPct", serde_json::json!(101)),
        ("bodyAmountPct", serde_json::json!(null)),
        ("bodyFrequencyHz", serde_json::json!(99)),
        ("bodyFrequencyHz", serde_json::json!(2001)),
        ("bodyFrequencyHz", serde_json::json!(1e100)),
    ] {
        let mut pluck = serde_json::Map::new();
        pluck.insert(key.into(), value);
        assert!(
            normalize_instrument_slot_config(&serde_json::json!({
                "type":"pluck", "pluck":pluck
            }))
            .is_err(),
            "accepted {key}"
        );
    }
}

#[test]
fn drum_default_kit_and_invalid_present_fields_are_checked_without_changing_legacy_slots() {
    let slot = normalize_instrument_slot_config(&serde_json::json!({"type":"drum"})).unwrap();
    assert!(slot.slot.drum.is_none());
    let default = DrumConfig::default();
    assert_eq!(default.voices.len(), 8);
    assert!(default.assignments.is_empty());
    for (path, invalid) in [
        ("decayMs", serde_json::json!(19)),
        ("tonePct", serde_json::json!(101)),
        ("attackMs", serde_json::json!(51)),
        ("tuneSemis", serde_json::json!(-13)),
        ("sound", serde_json::json!("unknown")),
        ("sweepSemis", serde_json::json!(37)),
        ("sweepMs", serde_json::json!(201)),
        ("sweepMs", serde_json::json!(1e100)),
        ("noiseMixPct", serde_json::json!(101)),
        ("levelPct", serde_json::json!(-1)),
    ] {
        let mut drum = serde_json::to_value(&default).unwrap();
        drum["voices"][0][path] = invalid;
        assert!(
            normalize_instrument_slot_config(&serde_json::json!({"type":"drum", "drum":drum}))
                .is_err(),
            "{path}"
        );
    }
    let mut drum = serde_json::to_value(&default).unwrap();
    drum["assignments"] = serde_json::json!([
        {"x": 1, "y": 0, "voice": 0}, {"x": 1, "y": 0, "voice": 1}
    ]);
    assert!(
        normalize_instrument_slot_config(&serde_json::json!({"type":"drum", "drum": drum}))
            .is_err()
    );
    assert!(
        normalize_instrument_slot_config(&serde_json::json!({"type":"synth"}))
            .unwrap()
            .slot
            .drum
            .is_none()
    );
    assert!(
        normalize_instrument_slot_config(&serde_json::json!({"type":"fm"}))
            .unwrap()
            .slot
            .drum
            .is_none()
    );
}

#[test]
fn drum_legacy_voice_defaults_follow_sound_when_reordered_and_roundtrip() {
    let defaults = DrumConfig::default();
    let mut legacy = serde_json::to_value(&defaults).unwrap();
    for voice in legacy["voices"].as_array_mut().unwrap() {
        for key in ["sweepSemis", "sweepMs", "noiseMixPct", "levelPct"] {
            voice.as_object_mut().unwrap().remove(key);
        }
    }
    legacy["voices"] = serde_json::json!([
        legacy["voices"][7],
        legacy["voices"][6],
        legacy["voices"][5],
        legacy["voices"][4],
        legacy["voices"][3],
        legacy["voices"][2],
        legacy["voices"][1],
        legacy["voices"][0]
    ]);
    let decoded: DrumConfig = serde_json::from_value(legacy).unwrap();
    assert_eq!(
        DrumSound::ALL.map(|sound| sound.default_voice().sweep_semis),
        [24, 3, 0, 0, 8, 6, 0, 0]
    );
    assert_eq!(
        DrumSound::ALL.map(|sound| sound.default_voice().sweep_ms),
        [55.0, 25.0, 0.0, 0.0, 70.0, 50.0, 0.0, 0.0]
    );
    assert_eq!(
        DrumSound::ALL.map(|sound| sound.default_voice().noise_mix_pct),
        [12.0, 80.0, 95.0, 95.0, 15.0, 15.0, 95.0, 35.0]
    );
    for (voice, sound) in decoded.voices.iter().zip(DrumSound::ALL.into_iter().rev()) {
        assert_eq!(voice.sound, sound);
        let expected = sound.default_voice();
        assert_eq!(voice.sweep_semis, expected.sweep_semis);
        assert_eq!(voice.sweep_ms, expected.sweep_ms);
        assert_eq!(voice.noise_mix_pct, expected.noise_mix_pct);
        assert_eq!(voice.level_pct, 100.0);
    }
    let encoded = serde_json::to_value(&decoded).unwrap();
    let roundtrip: DrumConfig = serde_json::from_value(encoded).unwrap();
    assert_eq!(
        roundtrip.voices.map(|voice| voice.sweep_semis),
        decoded.voices.map(|voice| voice.sweep_semis)
    );
    assert_eq!(
        roundtrip.voices.map(|voice| voice.sweep_ms),
        decoded.voices.map(|voice| voice.sweep_ms)
    );
    assert_eq!(
        roundtrip.voices.map(|voice| voice.noise_mix_pct),
        decoded.voices.map(|voice| voice.noise_mix_pct)
    );
    assert_eq!(
        roundtrip.voices.map(|voice| voice.level_pct),
        decoded.voices.map(|voice| voice.level_pct)
    );

    let mut missing_legacy_field = serde_json::to_value(&defaults).unwrap();
    missing_legacy_field["voices"][0]
        .as_object_mut()
        .unwrap()
        .remove("attackMs");
    assert!(serde_json::from_value::<DrumConfig>(missing_legacy_field).is_err());

    let mut malformed_new_field = serde_json::to_value(&defaults).unwrap();
    malformed_new_field["voices"][0]["levelPct"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<DrumConfig>(malformed_new_field).is_err());
}
