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
    assert!(resolve_sample_path(&root, r"samples\kit\kick.wav").is_some());
    for path in [
        "../kick.wav",
        "/tmp/kick.wav",
        r"C:\tmp\kick.wav",
        "kit",
        "missing.wav",
        "samples/kit/kick.aiff",
    ] {
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

#[test]
fn initial_sample_preparation_succeeds_for_a_valid_wav() {
    let root = temp_dir("sample-prep-success");
    std::fs::write(root.join("kick.wav"), wav_bytes()).unwrap();
    let audio = crate::audio::test_service_for_sample_prep();

    let result = sample_banks(&sample_config(), &root, &audio);

    assert!(result.is_ok());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn initial_sample_preparation_rejects_missing_root_and_file() {
    let missing_root = temp_dir("sample-prep-missing-root");
    std::fs::remove_dir_all(&missing_root).unwrap();
    let audio = crate::audio::test_service_for_sample_prep();
    let error = sample_banks(&sample_config(), &missing_root, &audio).unwrap_err();
    assert_eq!(
        error,
        SampleLoadError::Unresolved("samples/kick.wav".into())
    );

    let root = temp_dir("sample-prep-missing-file");
    let error = sample_banks(&sample_config(), &root, &audio).unwrap_err();
    assert_eq!(
        error,
        SampleLoadError::Unresolved("samples/kick.wav".into())
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn initial_sample_preparation_rejects_a_malformed_wav() {
    let root = temp_dir("sample-prep-malformed");
    std::fs::write(root.join("kick.wav"), b"not a wav").unwrap();
    let audio = crate::audio::test_service_for_sample_prep();

    let error = sample_banks(&sample_config(), &root, &audio).unwrap_err();

    assert_eq!(
        error,
        SampleLoadError::Undecodable("samples/kick.wav".into())
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn pi_sample_fixture_decodes_every_playable_default_sample() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../samples");
    let root = root.canonicalize().expect("Pi sample fixture root");
    let mut playable_wav_count = 0;
    let mut metadata_only_count = 0;

    for line in include_str!("../../../samples/MANIFEST.tsv")
        .lines()
        .skip(1)
    {
        let relative = line.split('\t').next().expect("inventory path");
        if relative.to_ascii_lowercase().ends_with(".wav") {
            playable_wav_count += 1;
            let canonical_id = format!("samples/{relative}");
            let resolved = resolve_sample_path(&root, &canonical_id)
                .expect("manifest WAV resolves from Pi sample root");
            let expected = root
                .join(relative)
                .canonicalize()
                .expect("manifest WAV path");
            assert_eq!(resolved, expected);
            let buffer = rodio_engine_source::decode_sample_file(&resolved)
                .expect("manifest WAV decodes with production decoder");
            assert!(buffer.channels > 0);
            assert!(buffer.sample_rate > 0);
            assert!(!buffer.samples.is_empty());
            assert_eq!(buffer.samples.len() % usize::from(buffer.channels), 0);
            assert!(buffer.samples.iter().all(|sample| sample.is_finite()));
            assert!(expected.starts_with(&root));
        } else {
            metadata_only_count += 1;
        }
    }

    assert_eq!(playable_wav_count, 318);
    assert_eq!(metadata_only_count, 2);
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

fn sample_config() -> NormalizedAudioConfig {
    parse_audio_config(&serde_json::json!({
        "instruments": [{
            "type": "sampler",
            "sample": { "slots": [{ "path": "samples/kick.wav" }] }
        }]
    }))
    .unwrap()
}

fn wav_bytes() -> Vec<u8> {
    let samples = [0_i16, 1_000_i16];
    let data_len = samples.len() * 2;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36_u32 + data_len as u32).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&44_100_u32.to_le_bytes());
    bytes.extend_from_slice(&88_200_u32.to_le_bytes());
    bytes.extend_from_slice(&2_u16.to_le_bytes());
    bytes.extend_from_slice(&16_u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&(data_len as u32).to_le_bytes());
    for sample in samples {
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    bytes
}
