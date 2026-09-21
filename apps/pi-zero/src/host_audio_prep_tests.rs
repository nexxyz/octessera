use super::*;
use playback_runtime::{
    HostMessage, RuntimeErrorCode, RuntimeErrorDomain, RuntimeOperation, RuntimeStoreResult,
};
use rodio_engine_source::EngineEvent;
use std::path::{Path, PathBuf};
use std::sync::mpsc;

#[test]
fn pending_owner_requests_preserve_order() {
    let (tx, rx) = mpsc::channel();
    tx.send(AudioControlRequest::InstrumentSlot {
        sequence: 1,
        instrument_slot: 2,
        generation: 1,
        config: serde_json::json!({ "type": "synth" }),
        samples_dir: PathBuf::from("samples"),
    })
    .unwrap();
    tx.send(AudioControlRequest::GlobalFxSlot {
        sequence: 2,
        slot_index: 0,
        generation: 2,
        fx_type: "eq".into(),
        params: Default::default(),
    })
    .unwrap();

    let mut state = FullConfigState {
        sequence: 0,
        revision: 1,
        generation: 1,
        request_id: None,
        config: serde_json::json!({ "masterVolume": 70 }),
        samples_dir: PathBuf::from("old"),
    };
    let mut pending = std::collections::VecDeque::new();
    let mut pending_preview = None;

    assert!(!drain_pending_requests(
        &rx,
        None,
        &mut state,
        &mut pending,
        &mut pending_preview,
        None,
    ));
    assert_eq!(pending.len(), 2);
    assert!(matches!(
        pending[0],
        OwnerRequest::Instrument {
            instrument_slot: 2,
            generation: 1,
            ..
        }
    ));
    assert!(matches!(
        pending[1],
        OwnerRequest::GlobalFx {
            slot_index: 0,
            generation: 2,
            ..
        }
    ));
}

#[test]
fn full_config_coalescing_drops_stale_dynamic_config_delta() {
    let (tx, rx) = mpsc::channel();
    tx.send(AudioControlRequest::InstrumentSlot {
        sequence: 1,
        instrument_slot: 0,
        generation: 1,
        config: serde_json::json!({ "type": "synth" }),
        samples_dir: PathBuf::from("samples"),
    })
    .unwrap();
    tx.send(AudioControlRequest::FullConfig {
        sequence: 2,
        revision: 2,
        generation: 2,
        request_id: None,
        config: serde_json::json!({ "masterVolume": 91 }),
        samples_dir: PathBuf::from("new"),
    })
    .unwrap();

    let mut state = FullConfigState {
        sequence: 0,
        revision: 1,
        generation: 1,
        request_id: None,
        config: serde_json::json!({ "masterVolume": 70 }),
        samples_dir: PathBuf::from("old"),
    };
    let mut pending = std::collections::VecDeque::new();
    let mut pending_preview = None;

    assert!(drain_pending_requests(
        &rx,
        None,
        &mut state,
        &mut pending,
        &mut pending_preview,
        None,
    ));
    assert_eq!(state.revision, 2);
    assert_eq!(state.generation, 2);
    assert_eq!(state.samples_dir, PathBuf::from("new"));
    assert!(pending.is_empty());
}

#[test]
fn prep_failure_is_identified_and_typed() {
    let result = audio_prep_failure(9, Some("audio-9".into()), "bad samples".into());
    assert!(matches!(
        result,
        RuntimeStoreResult::Identified {
            request_id,
            revision: Some(9),
            result,
        } if request_id == "audio-9" && matches!(result.as_ref(), RuntimeStoreResult::RuntimeFailure { error } if error.domain == RuntimeErrorDomain::Audio && error.code == RuntimeErrorCode::OperationFailed && error.operation == RuntimeOperation::AudioCommand)
    ));
}

#[test]
fn unresolved_sample_failure_is_typed_and_not_success() {
    let result = sample_failure(
        11,
        Some("audio-11".into()),
        RuntimeErrorCode::NotFound,
        "sample not found: missing.wav".into(),
    );
    assert!(matches!(
        result,
        RuntimeStoreResult::Identified {
            result,
            request_id,
            revision: Some(11)
        } if request_id == "audio-11"
            && matches!(result.as_ref(), RuntimeStoreResult::RuntimeFailure { error }
                if error.domain == RuntimeErrorDomain::Sample
                    && error.code == RuntimeErrorCode::NotFound
                    && error.operation == RuntimeOperation::AudioCommand)
    ));
}

#[test]
fn prep_success_is_identified_as_audio_command_success() {
    let result = audio_prep_success(10, Some("audio-10".into()));
    assert!(matches!(
        result,
        RuntimeStoreResult::Identified {
            request_id,
            revision: Some(10),
            result,
        } if request_id == "audio-10" && matches!(result.as_ref(), RuntimeStoreResult::OperationSucceeded { operation: RuntimeOperation::AudioCommand, .. })
    ));
}

#[test]
fn stale_audio_revision_is_cancellation() {
    assert!(matches!(
        config_prep::ensure_current_audio_revision(4, 3),
        Err(AudioPrepError::Superseded)
    ));
}

#[test]
fn prep_queue_reports_typed_overload_at_explicit_capacity() {
    let (tx, rx) = mpsc::channel();
    for sequence in 0..(AUDIO_PREP_QUEUE_CAPACITY + 1) as u64 {
        tx.send(AudioControlRequest::InstrumentSlot {
            sequence,
            instrument_slot: 0,
            generation: sequence,
            config: serde_json::json!({ "type": "synth" }),
            samples_dir: PathBuf::from("samples"),
        })
        .unwrap();
    }
    let (result_tx, result_rx) = mpsc::channel();
    let mut state = FullConfigState {
        sequence: 0,
        revision: 1,
        generation: 1,
        request_id: None,
        config: serde_json::json!({}),
        samples_dir: PathBuf::from("samples"),
    };
    let mut pending = std::collections::VecDeque::new();
    let mut pending_preview = None;
    for _ in 0..5 {
        assert!(!drain_pending_requests(
            &rx,
            None,
            &mut state,
            &mut pending,
            &mut pending_preview,
            Some(&result_tx),
        ));
    }
    assert_eq!(pending.len(), AUDIO_PREP_QUEUE_CAPACITY);
    assert!(matches!(
        result_rx.try_recv().unwrap(),
        HostMessage::RuntimeResult {
            result: RuntimeStoreResult::RuntimeFailure { error }
        } if error.domain == RuntimeErrorDomain::Audio
            && error.code == RuntimeErrorCode::OperationFailed
    ));
}

#[test]
fn stale_preview_is_dropped_without_a_failure_result() {
    let audio = crate::audio::test_service_for_sample_prep();
    audio
        .preview_generation
        .store(2, std::sync::atomic::Ordering::Release);
    let (result_tx, result_rx) = mpsc::channel();
    preview_prep::process_request(
        &audio,
        0,
        "missing.wav",
        100,
        Path::new("samples"),
        1,
        &result_tx,
    );
    assert!(result_rx.try_recv().is_err());
}

#[test]
fn preview_event_uses_sample_owner_generation_not_preview_token() {
    let root = std::env::temp_dir().join(format!(
        "octessera-pi-preview-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("kick.wav"), owner_wav_bytes()).unwrap();
    let audio = crate::audio::test_service_for_sample_prep();
    audio
        .preview_generation
        .store(1, std::sync::atomic::Ordering::Release);
    audio.generations.lock().unwrap().sample[0] = 7;
    let event =
        crate::audio_config_parse::prepare_sample_preview(&audio, 0, "kick.wav", 100, &root, 7)
            .unwrap();
    assert!(matches!(
        event,
        EngineEvent::PreviewSample {
            instrument_slot: 0,
            generation: 7,
            ..
        }
    ));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn preview_keeps_sample_generation_after_non_sampler_replacement() {
    let root = std::env::temp_dir().join(format!(
        "octessera-pi-preview-owner-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("kick.wav"), owner_wav_bytes()).unwrap();
    let (audio, _, mut event_rx, _) =
        crate::audio::test_service_with_recording_dir(root.join("recordings"));
    let (result_tx, _) = mpsc::channel();

    owner_prep::process_instrument_slot(
        &audio,
        1,
        0,
        11,
        serde_json::json!({
            "type": "sampler",
            "sample": { "slots": [{ "path": "kick.wav" }] }
        }),
        root.clone(),
        &result_tx,
    );
    assert!(matches!(
        event_rx.try_recv().unwrap(),
        EngineEvent::SetPreparedInstrumentOwner {
            generation: 11,
            sample_bank: Some(_),
            ..
        }
    ));
    owner_prep::process_instrument_slot(
        &audio,
        2,
        0,
        12,
        serde_json::json!({ "type": "synth" }),
        root.clone(),
        &result_tx,
    );
    assert!(matches!(
        event_rx.try_recv().unwrap(),
        EngineEvent::SetPreparedInstrumentOwner {
            generation: 12,
            sample_bank: None,
            ..
        }
    ));
    assert_eq!(audio.sample_owner_generation(0).unwrap(), 11);
    let event = crate::audio_config_parse::prepare_sample_preview(
        &audio,
        0,
        "kick.wav",
        100,
        &root,
        audio.sample_owner_generation(0).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        event,
        EngineEvent::PreviewSample { generation: 11, .. }
    ));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn instrument_owner_replacement_publishes_one_atomic_owner_event() {
    let root = std::env::temp_dir().join(format!(
        "octessera-pi-owner-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("kick.wav"), owner_wav_bytes()).unwrap();
    let (audio, _, mut event_rx, _) =
        crate::audio::test_service_with_recording_dir(root.join("recordings"));
    let (result_tx, result_rx) = mpsc::channel();

    owner_prep::process_instrument_slot(
        &audio,
        1,
        0,
        4,
        serde_json::json!({
            "type": "sampler",
            "sample": { "slots": [{ "path": "kick.wav" }] }
        }),
        root.clone(),
        &result_tx,
    );
    assert!(matches!(
        event_rx.try_recv().unwrap(),
        EngineEvent::SetPreparedInstrumentOwner {
            generation: 4,
            sample_bank: Some(ref bank),
            ..
        }
            if bank.slots[0].buffer.is_some()
    ));
    {
        let generations = audio.generations.lock().unwrap();
        assert_eq!(generations.instrument[0], 4);
        assert_eq!(generations.sample[0], 4);
    }
    *audio.sample_bank_signature.lock().unwrap() = "keep".into();

    owner_prep::process_instrument_slot(
        &audio,
        2,
        0,
        5,
        serde_json::json!({ "type": "synth" }),
        root.clone(),
        &result_tx,
    );
    assert!(matches!(
        event_rx.try_recv().unwrap(),
        EngineEvent::SetPreparedInstrumentOwner {
            generation: 5,
            sample_bank: None,
            ..
        }
    ));
    let generations = audio.generations.lock().unwrap();
    assert_eq!(generations.instrument[0], 5);
    assert_eq!(generations.sample[0], 4);
    drop(generations);
    assert_eq!(*audio.sample_bank_signature.lock().unwrap(), "keep");
    owner_prep::process_instrument_slot(
        &audio,
        3,
        0,
        6,
        serde_json::json!({
            "type": "sampler",
            "sample": { "slots": [{ "path": "missing.wav" }] }
        }),
        root.clone(),
        &result_tx,
    );
    assert!(matches!(
        result_rx.try_recv().unwrap(),
        HostMessage::RuntimeResult {
            result: RuntimeStoreResult::RuntimeFailure { error }
        } if error.domain == RuntimeErrorDomain::Sample
            && error.code == RuntimeErrorCode::NotFound
    ));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn failed_atomic_owner_broadcast_does_not_commit_owner_state() {
    let root = std::env::temp_dir().join(format!(
        "octessera-pi-owner-failure-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let (audio, _, event_rx, _) =
        crate::audio::test_service_with_recording_dir(root.join("recordings"));
    *audio.sample_bank_signature.lock().unwrap() = "previous".into();
    drop(event_rx);
    let (result_tx, result_rx) = mpsc::channel();

    owner_prep::process_instrument_slot(
        &audio,
        1,
        0,
        4,
        serde_json::json!({ "type": "synth" }),
        root.clone(),
        &result_tx,
    );

    let generations = audio.generations.lock().unwrap();
    assert_eq!(generations.instrument[0], 0);
    assert_eq!(generations.sample[0], 0);
    assert_eq!(*audio.sample_bank_signature.lock().unwrap(), "previous");
    assert!(result_rx.try_recv().is_ok());
    drop(generations);
    let _ = std::fs::remove_dir_all(root);
}

fn owner_wav_bytes() -> Vec<u8> {
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
