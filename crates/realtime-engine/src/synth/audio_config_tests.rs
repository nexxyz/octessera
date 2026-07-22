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
    assert_eq!(slot.slot.mixer.unwrap().route, "direct");
}
