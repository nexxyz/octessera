use super::*;

#[test]
fn parses_full_audio_config_payload_for_engine() {
    let config = parse_audio_config(&serde_json::json!({
        "masterVolume": 81,
        "voiceStealingMode": "fixed12",
        "panPositions": 33,
        "instruments": [{
            "type": "synth",
            "mixer": { "route": "fx_bus_1", "panPos": 12, "volume": 76 }
        }],
        "mixer": {
            "buses": [{ "slot1": { "type": "none" }, "slot3": { "type": "tremolo" }, "panPos": 16 }],
            "master": { "slots": [{ "type": "none" }] }
        }
    }))
    .unwrap();

    assert_eq!(config.instruments.master_volume, 81.0);
    assert_eq!(config.instruments.pan_positions, 33);
    assert_eq!(config.instruments.instruments[0].kind, "synth");
    let mixer = config.instruments.instruments[0].mixer.as_ref().unwrap();
    assert_eq!(mixer.route, "fx_bus_1");
    assert_eq!(mixer.pan_pos, 12);
    assert_eq!(mixer.volume, 76.0);
    let mixer_config = config.instruments.mixer.unwrap();
    assert_eq!(mixer_config.buses.len(), 1);
    assert!(
        matches!(mixer_config.buses[0].slots[1], FxBusSlotConfig::Kind(ref kind) if kind == "none")
    );
    assert!(
        matches!(mixer_config.buses[0].slots[2], FxBusSlotConfig::Config { ref kind, .. } if kind == "tremolo")
    );
    assert_eq!(config.voice_stealing_mode.as_deref(), Some("fixed12"));
    assert_eq!(
        parse_voice_stealing_mode(config.voice_stealing_mode.as_deref().unwrap()),
        VoiceStealingMode::Fixed12
    );
}

#[test]
fn malformed_fx_slot_payload_fails_parse() {
    let error = match parse_audio_config(&serde_json::json!({
        "instruments": [{ "type": "synth" }],
        "mixer": { "buses": [{ "slot1": { "params": {} } }] }
    })) {
        Ok(_) => panic!("malformed slot unexpectedly parsed"),
        Err(error) => error,
    };

    assert!(error.contains("invalid mixer bus 1 slot 1"), "{error}");
}

#[test]
fn missing_fx_slots_default_to_positional_none() {
    let config = parse_audio_config(&serde_json::json!({
        "instruments": [{ "type": "synth" }],
        "mixer": { "buses": [{}] }
    }))
    .unwrap();
    let mixer_config = config.instruments.mixer.unwrap();

    assert_eq!(mixer_config.buses[0].slots.len(), 3);
    assert!(mixer_config.buses[0]
        .slots
        .iter()
        .all(|slot| matches!(slot, FxBusSlotConfig::Kind(kind) if kind == "none")));
}

#[test]
fn explicit_none_fx_slot_still_parses() {
    let config = parse_audio_config(&serde_json::json!({
        "instruments": [{ "type": "synth" }],
        "mixer": { "buses": [{ "slot1": { "type": "none" } }] }
    }))
    .unwrap();
    let mixer_config = config.instruments.mixer.unwrap();

    assert!(matches!(
        mixer_config.buses[0].slots[0],
        FxBusSlotConfig::Config { ref kind, .. } if kind == "none"
    ));
}

#[test]
fn sample_signature_tracks_sampler_param_changes() {
    let first = vec![sampler_source(AudioSamplePayload {
        slots: vec![AudioSampleSlotPayload {
            path: Some("kick.wav".into()),
        }],
        tune_semis: Some(0.0),
        amp: Some(AudioSampleAmpPayload {
            gain_pct: Some(100.0),
            velocity_sensitivity_pct: Some(100.0),
        }),
        filter: Some(AudioSampleFilterPayload {
            cutoff_hz: Some(8000.0),
            resonance: Some(20.0),
        }),
    })];
    let changed = vec![sampler_source(AudioSamplePayload {
        slots: vec![AudioSampleSlotPayload {
            path: Some("kick.wav".into()),
        }],
        tune_semis: Some(2.0),
        amp: Some(AudioSampleAmpPayload {
            gain_pct: Some(80.0),
            velocity_sensitivity_pct: Some(70.0),
        }),
        filter: Some(AudioSampleFilterPayload {
            cutoff_hz: Some(4400.0),
            resonance: Some(42.0),
        }),
    })];

    assert_ne!(sample_signature(&first), sample_signature(&changed));
}

#[test]
fn sample_signature_tracks_sampler_filter_changes() {
    let first = vec![sampler_source(AudioSamplePayload {
        slots: vec![AudioSampleSlotPayload {
            path: Some("kick.wav".into()),
        }],
        tune_semis: None,
        amp: None,
        filter: Some(AudioSampleFilterPayload {
            cutoff_hz: Some(8000.0),
            resonance: Some(20.0),
        }),
    })];
    let changed = vec![sampler_source(AudioSamplePayload {
        slots: vec![AudioSampleSlotPayload {
            path: Some("kick.wav".into()),
        }],
        tune_semis: None,
        amp: None,
        filter: Some(AudioSampleFilterPayload {
            cutoff_hz: Some(3200.0),
            resonance: Some(55.0),
        }),
    })];

    assert_ne!(sample_signature(&first), sample_signature(&changed));
}

#[test]
fn sample_signature_ignores_sample_payload_for_non_sampler_slots() {
    let synth_with_sample_payload = vec![SampleSource {
        kind: "synth".into(),
        sample: Some(AudioSamplePayload {
            slots: vec![AudioSampleSlotPayload {
                path: Some("kick.wav".into()),
            }],
            tune_semis: Some(12.0),
            amp: Some(AudioSampleAmpPayload {
                gain_pct: Some(50.0),
                velocity_sensitivity_pct: Some(60.0),
            }),
            filter: Some(AudioSampleFilterPayload {
                cutoff_hz: Some(1200.0),
                resonance: Some(80.0),
            }),
        }),
    }];

    assert_eq!(sample_signature(&synth_with_sample_payload), "-");
}

#[test]
fn sample_path_resolution_rejects_escape_and_missing_paths() {
    let root = temp_dir("sample-paths");
    std::fs::create_dir_all(root.join("kit")).unwrap();
    std::fs::write(root.join("kit").join("kick.wav"), b"wav").unwrap();

    assert!(resolve_sample_path(&root, "kit/kick.wav").is_some());
    for path in ["../kick.wav", "/tmp/kick.wav", "missing.wav"] {
        assert!(resolve_sample_path(&root, path).is_none(), "{path}");
    }
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn sample_path_resolution_strips_desktop_samples_prefix() {
    let root = temp_dir("sample-path-prefix");
    std::fs::write(root.join("kick.wav"), b"wav").unwrap();
    let expected = root.join("kick.wav").canonicalize().unwrap();

    assert_eq!(
        resolve_sample_path(&root, "samples/kick.wav"),
        Some(expected.clone())
    );
    assert_eq!(
        resolve_sample_path(&root, r"samples\kick.wav"),
        Some(expected)
    );
    assert!(resolve_sample_path(&root, "samples/../kick.wav").is_none());
    assert!(resolve_sample_path(&root, r"samples\..\kick.wav").is_none());
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn sample_path_resolution_rejects_symlink_escape() {
    let root = temp_dir("sample-symlink-root");
    let outside = temp_dir("sample-symlink-outside");
    std::fs::write(outside.join("escape.wav"), b"wav").unwrap();
    std::os::unix::fs::symlink(outside.join("escape.wav"), root.join("escape.wav")).unwrap();

    assert!(resolve_sample_path(&root, "escape.wav").is_none());
    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_dir_all(outside);
}

fn sampler_source(sample: AudioSamplePayload) -> SampleSource {
    SampleSource {
        kind: "sampler".into(),
        sample: Some(sample),
    }
}

fn temp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "octessera-pi-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}
