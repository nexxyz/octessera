use super::*;
use std::collections::HashMap;
use std::sync::{mpsc, Arc, Mutex};

#[test]
fn prep_failure_returns_identified_fault_without_mutating_state() {
    let (request_tx, request_rx) = mpsc::channel();
    drop(request_tx);
    let (audio_tx, audio_rx) = mpsc::channel();
    let (result_tx, result_rx) = mpsc::channel();
    let state = DesktopAudioPrepState {
        config_revision: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        synth_slots: Arc::new(Mutex::new([true; INSTRUMENT_SLOT_COUNT])),
        sample_cache: Arc::new(Mutex::new(HashMap::new())),
        sample_bank_signature: Arc::new(Mutex::new("retained".into())),
    };

    handle_full_config_request_with_result(
        7,
        Some("audio-7".into()),
        serde_json::json!({ "instruments": "invalid" }),
        &request_rx,
        &audio_tx,
        &result_tx,
        &state,
    );

    assert!(audio_rx.try_recv().is_err());
    assert_eq!(*state.sample_bank_signature.lock().unwrap(), "retained");
    assert!(matches!(
        result_rx.recv().unwrap(),
        HostMessage::RuntimeResult {
            result: RuntimeStoreResult::Identified {
                request_id,
                revision: Some(7),
                result,
            }
        } if request_id == "audio-7" && matches!(result.as_ref(), RuntimeStoreResult::RuntimeFailure { error } if error.domain == RuntimeErrorDomain::Audio && error.code == playback_runtime::RuntimeErrorCode::InvalidPayload && error.operation == RuntimeOperation::AudioCommand)
    ));
}

#[test]
fn prep_success_returns_identified_audio_result() {
    let (request_tx, request_rx) = mpsc::channel();
    drop(request_tx);
    let (audio_tx, audio_rx) = mpsc::channel();
    let (result_tx, result_rx) = mpsc::channel();
    let state = DesktopAudioPrepState {
        config_revision: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        synth_slots: Arc::new(Mutex::new([true; INSTRUMENT_SLOT_COUNT])),
        sample_cache: Arc::new(Mutex::new(HashMap::new())),
        sample_bank_signature: Arc::new(Mutex::new(String::new())),
    };

    handle_full_config_request_with_result(
        8,
        Some("audio-8".into()),
        serde_json::json!({
            "masterVolume": 70,
            "panPositions": 33,
            "instruments": [{ "type": "synth" }],
            "mixer": { "buses": [], "master": { "slots": [] } }
        }),
        &request_rx,
        &audio_tx,
        &result_tx,
        &state,
    );

    assert!(matches!(
        audio_rx.try_recv(),
        Ok(QueuedAudioEvent::SetAudioConfig { .. })
    ));
    assert!(matches!(
        result_rx.recv().unwrap(),
        HostMessage::RuntimeResult {
            result: RuntimeStoreResult::Identified {
                request_id,
                revision: Some(8),
                result,
            }
        } if request_id == "audio-8" && matches!(result.as_ref(), RuntimeStoreResult::OperationSucceeded { operation: RuntimeOperation::AudioCommand, .. })
    ));
}
