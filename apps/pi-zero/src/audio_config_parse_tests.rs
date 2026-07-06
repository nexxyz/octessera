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
            "buses": [{ "slot1": { "type": "none" }, "slot2": { "type": "none" }, "panPos": 16 }],
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
    assert_eq!(config.instruments.mixer.unwrap().buses.len(), 1);
    assert_eq!(config.voice_stealing_mode.as_deref(), Some("fixed12"));
    assert_eq!(
        parse_voice_stealing_mode(config.voice_stealing_mode.as_deref().unwrap()),
        VoiceStealingMode::Fixed12
    );
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
        "cellsymphony-pi-{name}-{}-{}",
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
