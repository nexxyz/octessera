use super::*;

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
    assert_eq!(fm.index_env.decay_ms, 250.0);
    assert_eq!(fm.amp_env.release_ms, 350.0);
    assert_eq!(fm.filter.cutoff_hz, default_synth_config().filter.cutoff_hz);
    assert!(normalize_instrument_slot_config(&serde_json::json!({"type":"unsupported"})).is_err());
    assert!(
        normalize_audio_config(&serde_json::json!({"instruments":[{"type":"unknown"}]})).is_err()
    );
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
