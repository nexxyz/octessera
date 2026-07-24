use super::*;
use realtime_engine::synth::FxBusSlotConfig;

#[test]
fn pi_uses_shared_audio_normalization_and_preserves_sample_paths() {
    let config = parse_audio_config(&serde_json::json!({
        "masterVolume": 81,
        "voiceStealingMode": "fixed12",
        "instruments": [{
            "type": "sampler",
            "sample": { "slots": [{ "path": "samples/kick.wav" }] }
        }],
        "mixer": { "buses": [{ "slot3": { "type": "tremolo" } }] }
    }))
    .unwrap();

    assert_eq!(config.master_volume, 81.0);
    assert_eq!(
        config.instruments[0].active_sample().unwrap().slots[0],
        Some("samples/kick.wav".into())
    );
    assert!(matches!(
        config.mixer.as_ref().unwrap().buses[0].slots[2],
        FxBusSlotConfig::Config { ref kind, .. } if kind == "tremolo"
    ));
}

#[test]
fn pi_rejects_malformed_fx_slot_payload() {
    let error = parse_audio_config(&serde_json::json!({
        "instruments": [{ "type": "synth" }],
        "mixer": { "buses": [{ "slot1": { "params": {} } }] }
    }))
    .unwrap_err();

    assert!(error.contains("invalid mixer bus 1 slot 1"), "{error}");
}

#[test]
fn pi_sample_paths_remain_host_resolved() {
    let root = temp_dir("sample-paths");
    std::fs::create_dir_all(root.join("kit")).unwrap();
    std::fs::write(root.join("kit").join("kick.wav"), b"wav").unwrap();

    assert!(resolve_sample_path(&root, "kit/kick.wav").is_some());
    assert!(resolve_sample_path(&root, "samples/kit/kick.wav").is_some());
    for path in ["../kick.wav", "/tmp/kick.wav", "missing.wav"] {
        assert!(resolve_sample_path(&root, path).is_none(), "{path}");
    }
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn pi_sample_failures_use_shared_typed_facts() {
    let error = SampleLoadError::Unresolved("missing.wav".into());

    assert_eq!(error.code(), playback_runtime::RuntimeErrorCode::NotFound);
    assert_eq!(error.message(), "sample not found: missing.wav");

    let undecodable = SampleLoadError::Undecodable("kick.wav".into());
    assert_eq!(
        undecodable.code(),
        playback_runtime::RuntimeErrorCode::OperationFailed
    );
    assert_eq!(undecodable.message(), "sample decode failed: kick.wav");
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
