use super::*;
use crate::audio_config_parse::{resolve_sample_path, SampleLoadError};
use playback_runtime::{
    RuntimeErrorCode, RuntimeErrorDomain, RuntimeMomentaryFxTarget, RuntimeOperation,
};
use realtime_engine::synth::{BUS_COUNT, INSTRUMENT_SLOT_COUNT};
use rodio_engine_source::decode_sample_file;
use std::collections::BTreeMap;

#[test]
fn pi_sample_preview_resolves_decodes_and_queues_preview_event() {
    let root = std::env::temp_dir().join(format!(
        "octessera-pi-preview-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("kick.wav"), wav_bytes()).unwrap();

    let path = resolve_sample_path(&root, "kick.wav").unwrap();
    assert!(!decode_sample_file(&path).unwrap().samples.is_empty());
    assert!(resolve_sample_path(&root, "../kick.wav").is_none());
    let error = SampleLoadError::Unresolved("../kick.wav".into());
    assert_eq!(error.code(), RuntimeErrorCode::NotFound);
    assert_eq!(error.message(), "sample not found: ../kick.wav");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn pi_accepts_every_valid_runtime_audio_command_at_the_adapter_boundary() {
    let root = std::env::temp_dir().join(format!(
        "octessera-pi-command-contract-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("kick.wav"), b"placeholder").unwrap();
    let params = BTreeMap::new();
    let commands = vec![
        RuntimeAudioCommand::SetAudioConfig {
            revision: 1,
            request_id: None,
            generation: 0,
            config: serde_json::json!({ "instruments": [] }),
        },
        RuntimeAudioCommand::SetDspConfig {
            generation: 0,
            config: realtime_engine::synth::DspRuntimeConfig::default(),
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
            config: serde_json::json!({ "type": "synth" }),
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
            param: realtime_engine::synth::FxParamId::MixPct,
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
            param: realtime_engine::synth::FxParamId::MixPct,
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
            sample_slot: 0,
            path: "kick.wav".into(),
            velocity: 96,
        },
    ];

    for command in commands {
        send_audio_command(None, &command, &root).unwrap();
    }
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn pi_fm_scalar_rejects_invalid_path_and_nonfinite_value() {
    for (path, value) in [("synth.amp.gainPct", 50.0), ("fm.index", f32::NAN)] {
        let error = send_audio_command(
            None,
            &RuntimeAudioCommand::SetFmParam {
                instrument_slot: 0,
                generation: 0,
                path: path.into(),
                value,
            },
            Path::new("samples"),
        )
        .unwrap_err();
        assert_eq!(error.facts.code, RuntimeErrorCode::InvalidPayload);
    }
}

#[test]
fn pi_pluck_scalar_rejects_invalid_path_and_nonfinite_value() {
    for (path, value) in [("synth.amp.gainPct", 50.0), ("pluck.decayMs", f32::NAN)] {
        let error = send_audio_command(
            None,
            &RuntimeAudioCommand::SetPluckParam {
                instrument_slot: 0,
                generation: 0,
                path: path.into(),
                value,
            },
            Path::new("samples"),
        )
        .unwrap_err();
        assert_eq!(error.facts.code, RuntimeErrorCode::InvalidPayload);
    }
}

#[test]
fn pi_drum_scalar_rejects_invalid_voice_path_and_nonfinite_value() {
    for (voice, path, value) in [
        (8, "drum.decayMs", 250.0),
        (1, "drum.amp.gainPct", 70.0),
        (0, "drum.voices.0.decayMs", 250.0),
        (0, "drum.filter.cutoffHz", f32::NAN),
    ] {
        let error = send_audio_command(
            None,
            &RuntimeAudioCommand::SetDrumParam {
                instrument_slot: 0,
                voice,
                generation: 0,
                path: path.into(),
                value,
            },
            Path::new("samples"),
        )
        .unwrap_err();
        assert_eq!(error.facts.code, RuntimeErrorCode::InvalidPayload);
    }
}

#[test]
fn pi_pluck_scalar_command_accepts_coalesced_audio_lane() {
    let (audio, _, _event_rx, _) = crate::audio::test_service_with_recording_dir(
        std::env::temp_dir().join("octessera-pi-pluck-command-recordings"),
    );
    send_audio_command(
        Some(audio.clone()),
        &RuntimeAudioCommand::SetPluckParam {
            instrument_slot: 2,
            generation: 42,
            path: "pluck.decayMs".into(),
            value: 220.0,
        },
        Path::new("samples"),
    )
    .unwrap();
}

#[test]
fn pi_invalid_fx_command_has_typed_invalid_payload_failure() {
    let error = send_audio_command(
        None,
        &RuntimeAudioCommand::SetFxBusSlot {
            bus_index: 0,
            slot_index: 0,
            generation: 0,
            fx_type: "unknown".into(),
            params: BTreeMap::new(),
        },
        Path::new("samples"),
    )
    .unwrap_err();
    assert_eq!(error.facts.domain, RuntimeErrorDomain::Audio);
    assert_eq!(error.facts.code, RuntimeErrorCode::InvalidPayload);
    assert_eq!(error.facts.operation, RuntimeOperation::AudioCommand);
}

#[test]
fn stale_momentary_stop_does_not_remove_newer_epoch() {
    let (audio, _, mut event_rx, _) = crate::audio::test_service_with_recording_dir(
        std::env::temp_dir().join("octessera-pi-momentary-recordings"),
    );
    let params = BTreeMap::new();
    send_audio_command(
        Some(audio.clone()),
        &RuntimeAudioCommand::MomentaryFxStart {
            id: "fx".into(),
            epoch: 2,
            fx_type: "freeze".into(),
            params: params.clone(),
            target: RuntimeMomentaryFxTarget::Global,
        },
        Path::new("samples"),
    )
    .unwrap();
    assert!(matches!(
        event_rx.try_recv().unwrap(),
        EngineEvent::PreparedMomentaryFxStart { .. }
    ));
    send_audio_command(
        Some(audio.clone()),
        &RuntimeAudioCommand::MomentaryFxStart {
            id: "other-fx".into(),
            epoch: 4,
            fx_type: "stutter".into(),
            params: params.clone(),
            target: RuntimeMomentaryFxTarget::Global,
        },
        Path::new("samples"),
    )
    .unwrap();
    assert!(matches!(
        event_rx.try_recv().unwrap(),
        EngineEvent::PreparedMomentaryFxStart { .. }
    ));
    let error = send_audio_command(
        Some(audio.clone()),
        &RuntimeAudioCommand::MomentaryFxStop {
            id: "fx".into(),
            epoch: 4,
        },
        Path::new("samples"),
    )
    .unwrap_err();
    assert_eq!(error.facts.domain, RuntimeErrorDomain::Audio);
    assert_eq!(error.facts.code, RuntimeErrorCode::InvalidPayload);
    assert_eq!(error.facts.operation, RuntimeOperation::AudioCommand);
    assert!(event_rx.try_recv().is_err());
    assert_eq!(audio.momentary_fx_type("fx").unwrap().unwrap().0, 2);
    send_audio_command(
        Some(audio.clone()),
        &RuntimeAudioCommand::MomentaryFxStop {
            id: "fx".into(),
            epoch: 2,
        },
        Path::new("samples"),
    )
    .unwrap();
    assert!(matches!(
        event_rx.try_recv().unwrap(),
        EngineEvent::MomentaryFxStop { epoch: 2 }
    ));
    assert!(audio.momentary_fx_type("fx").unwrap().is_none());
}

#[test]
fn unknown_momentary_stop_is_typed_invalid_payload_and_silent() {
    let (audio, _, mut event_rx, _) = crate::audio::test_service_with_recording_dir(
        std::env::temp_dir().join("octessera-pi-unknown-momentary-recordings"),
    );
    let error = send_audio_command(
        Some(audio),
        &RuntimeAudioCommand::MomentaryFxStop {
            id: "missing-fx".into(),
            epoch: 1,
        },
        Path::new("samples"),
    )
    .unwrap_err();
    assert_eq!(error.facts.domain, RuntimeErrorDomain::Audio);
    assert_eq!(error.facts.code, RuntimeErrorCode::InvalidPayload);
    assert_eq!(error.facts.operation, RuntimeOperation::AudioCommand);
    assert!(event_rx.try_recv().is_err());
}

#[test]
fn failed_momentary_stop_keeps_current_state() {
    let (audio, _, mut event_rx, _) = crate::audio::test_service_with_recording_dir(
        std::env::temp_dir().join("octessera-pi-momentary-failure-recordings"),
    );
    send_audio_command(
        Some(audio.clone()),
        &RuntimeAudioCommand::MomentaryFxStart {
            id: "fx".into(),
            epoch: 3,
            fx_type: "freeze".into(),
            params: BTreeMap::new(),
            target: RuntimeMomentaryFxTarget::Global,
        },
        Path::new("samples"),
    )
    .unwrap();
    assert!(matches!(
        event_rx.try_recv().unwrap(),
        EngineEvent::PreparedMomentaryFxStart { .. }
    ));
    drop(event_rx);

    let error = send_audio_command(
        Some(audio.clone()),
        &RuntimeAudioCommand::MomentaryFxStop {
            id: "fx".into(),
            epoch: 3,
        },
        Path::new("samples"),
    )
    .unwrap_err();
    assert_eq!(error.facts.code, RuntimeErrorCode::AudioThreadFailed);
    assert_eq!(audio.momentary_fx_type("fx").unwrap().unwrap().0, 3);
}

#[test]
fn pi_rejects_invalid_audio_slot_indices_at_the_adapter_boundary() {
    let error = send_audio_command(
        None,
        &RuntimeAudioCommand::SetFxBusSlot {
            bus_index: BUS_COUNT,
            slot_index: 0,
            generation: 0,
            fx_type: "delay".into(),
            params: BTreeMap::new(),
        },
        Path::new("samples"),
    )
    .unwrap_err();
    assert_eq!(error.facts.code, RuntimeErrorCode::InvalidPayload);

    let error = send_audio_command(
        None,
        &RuntimeAudioCommand::SetInstrumentMixer {
            instrument_slot: INSTRUMENT_SLOT_COUNT,
            generation: 0,
            volume_pct: Some(80.0),
            pan_pos: None,
        },
        Path::new("samples"),
    )
    .unwrap_err();
    assert_eq!(error.facts.code, RuntimeErrorCode::InvalidPayload);
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
