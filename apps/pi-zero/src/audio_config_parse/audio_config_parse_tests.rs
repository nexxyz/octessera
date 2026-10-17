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
fn initial_sample_preparation_loads_a_valid_wav() {
    let root = temp_dir("sample-prep-success");
    std::fs::write(root.join("kick.wav"), wav_bytes()).unwrap();
    let audio = crate::audio::test_service_for_sample_prep();
    let mut missing = Vec::new();

    let banks = sample_banks(&sample_config(), &root, &audio, &mut missing);

    assert!(missing.is_empty());
    assert!(banks
        .iter()
        .any(|bank| bank.slots.iter().any(|slot| slot.buffer.is_some())));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn missing_or_undecodable_samples_leave_their_slot_silent_and_are_reported() {
    let audio = crate::audio::test_service_for_sample_prep();
    let missing_root = temp_dir("sample-prep-missing-root");
    std::fs::remove_dir_all(&missing_root).unwrap();
    let missing_file_root = temp_dir("sample-prep-missing-file");
    let malformed_root = temp_dir("sample-prep-malformed");
    std::fs::write(malformed_root.join("kick.wav"), b"not a wav").unwrap();

    for root in [&missing_root, &missing_file_root, &malformed_root] {
        let mut missing = Vec::new();
        let banks = sample_banks(&sample_config(), root, &audio, &mut missing);
        assert_eq!(missing, vec!["samples/kick.wav".to_string()]);
        assert!(banks
            .iter()
            .all(|bank| bank.slots.iter().all(|slot| slot.buffer.is_none())));
    }
    let _ = std::fs::remove_dir_all(missing_file_root);
    let _ = std::fs::remove_dir_all(malformed_root);
}

#[test]
fn every_library_wav_resolves_from_the_pi_sample_root() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../samples");
    let root = root.canonicalize().expect("Pi sample fixture root");
    let mut directories = vec![root.clone()];
    let mut resolved_count = 0;
    while let Some(directory) = directories.pop() {
        for entry in std::fs::read_dir(directory).expect("sample directory") {
            let path = entry.expect("sample entry").path();
            if path.is_dir() {
                directories.push(path);
                continue;
            }
            if !path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("wav"))
            {
                continue;
            }
            let relative = path
                .strip_prefix(&root)
                .unwrap()
                .to_str()
                .unwrap()
                .replace('\\', "/");
            let resolved = resolve_sample_path(&root, &format!("samples/{relative}"))
                .expect("library WAV resolves from Pi sample root");
            assert_eq!(resolved, path.canonicalize().unwrap());
            resolved_count += 1;
        }
    }
    assert!(resolved_count > 0);
}

fn temp_dir(name: &str) -> std::path::PathBuf {
    let dir = crate::test_temp_dir::unique_temp_path(&format!("octessera-pi-{name}"));
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
