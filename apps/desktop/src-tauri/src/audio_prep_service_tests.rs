use super::*;
use crate::types::QueuedNote;
use std::collections::HashMap;
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

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

#[test]
fn unresolved_sample_retains_prior_bank_state_and_returns_typed_failure() {
    let (request_tx, request_rx) = mpsc::channel();
    drop(request_tx);
    let (audio_tx, audio_rx) = mpsc::channel();
    let (result_tx, result_rx) = mpsc::channel();
    let state = DesktopAudioPrepState {
        config_revision: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        synth_slots: Arc::new(Mutex::new([true; INSTRUMENT_SLOT_COUNT])),
        sample_cache: Arc::new(Mutex::new(HashMap::new())),
        sample_bank_signature: Arc::new(Mutex::new("last-good".into())),
    };

    handle_full_config_request_with_result(
        9,
        Some("audio-9".into()),
        serde_json::json!({
            "instruments": [{
                "type": "sampler",
                "sample": { "slots": [{ "path": "missing.wav" }] }
            }]
        }),
        &request_rx,
        &audio_tx,
        &result_tx,
        &state,
    );

    assert!(audio_rx.try_recv().is_err());
    assert_eq!(*state.sample_bank_signature.lock().unwrap(), "last-good");
    assert!(matches!(
        result_rx.recv().unwrap(),
        HostMessage::RuntimeResult {
            result: RuntimeStoreResult::Identified {
                result,
                request_id,
                revision: Some(9)
            }
        } if request_id == "audio-9"
            && matches!(result.as_ref(), RuntimeStoreResult::RuntimeFailure { error }
                if error.domain == RuntimeErrorDomain::Sample
                    && error.code == playback_runtime::RuntimeErrorCode::NotFound
                    && error.operation == RuntimeOperation::AudioCommand)
    ));
}

#[test]
fn preview_decode_failure_is_reported_by_prep_worker_without_audio_event() {
    let (trigger_tx, trigger_rx) = mpsc::channel();
    let (control, result_rx) = spawn_desktop_audio_control(trigger_tx, test_state());

    control
        .enqueue_sample_preview(0, "../missing.wav".into(), 96)
        .unwrap();

    assert!(trigger_rx.recv_timeout(Duration::from_millis(100)).is_err());
    let result = result_rx.recv_timeout(Duration::from_secs(1)).unwrap();
    assert!(matches!(
        result,
        HostMessage::RuntimeResult {
            result: RuntimeStoreResult::RuntimeFailure { error }
        } if error.domain == RuntimeErrorDomain::Sample
            && error.operation == RuntimeOperation::SamplePreview
            && error.code == playback_runtime::RuntimeErrorCode::NotFound
    ));
}

#[test]
fn full_config_replays_queued_dynamic_after_prepared_config() {
    let (request_tx, request_rx) = mpsc::channel();
    let (audio_tx, audio_rx) = mpsc::channel();
    let state = test_state();
    request_tx
        .send(AudioControlRequest::Dynamic(
            QueuedAudioEvent::SetMasterVolume { volume_pct: 44.0 },
        ))
        .unwrap();

    handle_full_config_request(1, None, audio_config(70), &request_rx, &audio_tx, &state);

    assert!(matches!(
        audio_rx.recv_timeout(Duration::from_secs(1)).unwrap(),
        QueuedAudioEvent::SetAudioConfig { instruments, .. } if instruments.master_volume == 70.0
    ));
    assert!(matches!(
        audio_rx.recv_timeout(Duration::from_secs(1)).unwrap(),
        QueuedAudioEvent::SetMasterVolume { volume_pct } if volume_pct == 44.0
    ));
}

#[test]
fn newer_full_config_wins_before_prepare_starts() {
    let (request_tx, request_rx) = mpsc::channel();
    let (audio_tx, audio_rx) = mpsc::channel();
    let state = test_state();
    request_tx
        .send(AudioControlRequest::FullConfig {
            revision: 2,
            request_id: None,
            config: audio_config(91),
        })
        .unwrap();

    handle_full_config_request(1, None, audio_config(70), &request_rx, &audio_tx, &state);

    assert!(matches!(
        audio_rx.recv_timeout(Duration::from_secs(1)).unwrap(),
        QueuedAudioEvent::SetAudioConfig { instruments, .. } if instruments.master_volume == 91.0
    ));
    assert!(audio_rx.try_recv().is_err());
}

#[test]
fn newer_full_config_preserves_queued_note_on_off_order() {
    let (request_tx, request_rx) = mpsc::channel();
    let (audio_tx, audio_rx) = mpsc::channel();
    let state = test_state();
    request_tx
        .send(AudioControlRequest::Dynamic(QueuedAudioEvent::Note(
            QueuedNote {
                instrument_slot: 2,
                note: 64,
                velocity: 90,
                duration_ms: 150,
            },
        )))
        .unwrap();
    request_tx
        .send(AudioControlRequest::Dynamic(QueuedAudioEvent::NoteOff {
            instrument_slot: 2,
            note: 64,
        }))
        .unwrap();
    request_tx
        .send(AudioControlRequest::FullConfig {
            revision: 2,
            request_id: None,
            config: audio_config(91),
        })
        .unwrap();

    handle_full_config_request(1, None, audio_config(70), &request_rx, &audio_tx, &state);

    assert!(matches!(
        audio_rx.recv_timeout(Duration::from_secs(1)).unwrap(),
        QueuedAudioEvent::SetAudioConfig { instruments, .. } if instruments.master_volume == 91.0
    ));
    assert!(matches!(
        audio_rx.recv_timeout(Duration::from_secs(1)).unwrap(),
        QueuedAudioEvent::Note(note) if note.instrument_slot == 2 && note.note == 64
    ));
    assert!(matches!(
        audio_rx.recv_timeout(Duration::from_secs(1)).unwrap(),
        QueuedAudioEvent::NoteOff {
            instrument_slot: 2,
            note: 64
        }
    ));
    assert!(audio_rx.try_recv().is_err());
}

#[test]
fn newer_full_config_drops_stale_dynamic_config_delta() {
    let (request_tx, request_rx) = mpsc::channel();
    let (audio_tx, audio_rx) = mpsc::channel();
    let state = test_state();
    request_tx
        .send(AudioControlRequest::Dynamic(
            QueuedAudioEvent::SetMasterVolume { volume_pct: 44.0 },
        ))
        .unwrap();
    request_tx
        .send(AudioControlRequest::FullConfig {
            revision: 2,
            request_id: None,
            config: audio_config(91),
        })
        .unwrap();

    handle_full_config_request(1, None, audio_config(70), &request_rx, &audio_tx, &state);

    assert!(matches!(
        audio_rx.recv_timeout(Duration::from_secs(1)).unwrap(),
        QueuedAudioEvent::SetAudioConfig { instruments, .. } if instruments.master_volume == 91.0
    ));
    assert!(audio_rx.try_recv().is_err());
}

fn test_state() -> DesktopAudioPrepState {
    DesktopAudioPrepState {
        synth_slots: Arc::new(Mutex::new([true; INSTRUMENT_SLOT_COUNT])),
        sample_cache: Arc::new(Mutex::new(HashMap::new())),
        config_revision: Arc::new(AtomicU64::new(0)),
        sample_bank_signature: Arc::new(Mutex::new(String::new())),
    }
}

fn audio_config(master_volume: u8) -> Value {
    serde_json::json!({
        "masterVolume": master_volume,
        "panPositions": 33,
        "instruments": [{ "type": "synth" }],
        "mixer": { "buses": [], "master": { "slots": [] } }
    })
}
