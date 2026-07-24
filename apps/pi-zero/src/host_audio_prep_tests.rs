use super::*;
use std::sync::mpsc;

#[test]
fn full_config_coalescing_preserves_queued_note_on_off_order() {
    let (tx, rx) = mpsc::channel();
    tx.send(AudioControlRequest::Dynamic(Box::new(
        EngineEvent::NoteOn {
            instrument_slot: 2,
            note: 64,
            velocity: 90,
            duration_ms: 150,
        },
    )))
    .unwrap();
    tx.send(AudioControlRequest::Dynamic(Box::new(
        EngineEvent::NoteOff {
            instrument_slot: 2,
            note: 64,
        },
    )))
    .unwrap();
    tx.send(AudioControlRequest::FullConfig {
        revision: 2,
        request_id: None,
        config: serde_json::json!({ "masterVolume": 91 }),
        samples_dir: PathBuf::from("new"),
    })
    .unwrap();

    let mut revision = 1;
    let mut request_id = None;
    let mut config = serde_json::json!({ "masterVolume": 70 });
    let mut samples_dir = PathBuf::from("old");
    let mut pending = Vec::new();
    let mut pending_previews = Vec::new();

    assert!(drain_pending_requests(
        &rx,
        &mut revision,
        &mut request_id,
        &mut config,
        &mut samples_dir,
        &mut pending,
        &mut pending_previews,
    ));
    assert_eq!(revision, 2);
    assert_eq!(samples_dir, PathBuf::from("new"));
    assert_eq!(pending.len(), 2);
    assert!(matches!(
        pending[0],
        EngineEvent::NoteOn {
            instrument_slot: 2,
            note: 64,
            ..
        }
    ));
    assert!(matches!(
        pending[1],
        EngineEvent::NoteOff {
            instrument_slot: 2,
            note: 64
        }
    ));
}

#[test]
fn full_config_coalescing_drops_stale_dynamic_config_delta() {
    let (tx, rx) = mpsc::channel();
    tx.send(AudioControlRequest::Dynamic(Box::new(
        EngineEvent::SetMasterVolume { volume_pct: 44.0 },
    )))
    .unwrap();
    tx.send(AudioControlRequest::FullConfig {
        revision: 2,
        request_id: None,
        config: serde_json::json!({ "masterVolume": 91 }),
        samples_dir: PathBuf::from("new"),
    })
    .unwrap();

    let mut revision = 1;
    let mut request_id = None;
    let mut config = serde_json::json!({ "masterVolume": 70 });
    let mut samples_dir = PathBuf::from("old");
    let mut pending = Vec::new();
    let mut pending_previews = Vec::new();

    assert!(drain_pending_requests(
        &rx,
        &mut revision,
        &mut request_id,
        &mut config,
        &mut samples_dir,
        &mut pending,
        &mut pending_previews,
    ));
    assert_eq!(revision, 2);
    assert_eq!(samples_dir, PathBuf::from("new"));
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
        ensure_current_audio_revision(4, 3),
        Err(AudioPrepError::Superseded)
    ));
}
