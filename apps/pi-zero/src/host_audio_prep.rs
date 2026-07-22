use crate::audio::{AudioControlRequest, AudioService};
use crate::audio_config_parse::{parse_audio_config, sample_banks, sample_signature};
use playback_runtime::{
    HostMessage, RuntimeErrorCode, RuntimeErrorDomain, RuntimeErrorFacts, RuntimeOperation,
    RuntimeStoreResult,
};
use realtime_engine::synth::{
    prepare_audio_config as prepare_engine_audio_config, DEFAULT_AUDIO_SAMPLE_RATE,
};
use rodio_engine_source::EngineEvent;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{Receiver, Sender};

pub fn spawn_audio_control_worker(
    rx: Receiver<AudioControlRequest>,
    audio: AudioService,
    result_tx: Sender<HostMessage>,
) {
    std::thread::spawn(move || audio_control_loop(rx, audio, result_tx));
}

fn audio_control_loop(
    rx: Receiver<AudioControlRequest>,
    audio: AudioService,
    result_tx: Sender<HostMessage>,
) {
    while let Ok(request) = rx.recv() {
        match request {
            AudioControlRequest::Dynamic(event) => {
                if let Err(error) = audio.broadcast(*event) {
                    send_audio_prep_result(&result_tx, audio_queue_failure(error));
                }
            }
            AudioControlRequest::FullConfig {
                revision,
                request_id,
                config,
                samples_dir,
            } => handle_full_config_request(
                revision,
                request_id,
                config,
                samples_dir,
                &rx,
                &audio,
                &result_tx,
            ),
        }
    }
}

fn handle_full_config_request(
    mut revision: u64,
    mut request_id: Option<String>,
    mut config: serde_json::Value,
    mut samples_dir: PathBuf,
    rx: &Receiver<AudioControlRequest>,
    audio: &AudioService,
    result_tx: &Sender<HostMessage>,
) {
    let mut pending_dynamic = Vec::new();
    drain_pending_requests(
        rx,
        &mut revision,
        &mut request_id,
        &mut config,
        &mut samples_dir,
        &mut pending_dynamic,
    );
    loop {
        let prepared =
            match prepare_audio_config(audio, revision, config.clone(), samples_dir.clone()) {
                Ok(prepared) => prepared,
                Err(AudioPrepError::Superseded) => {
                    send_dynamic_events(audio, pending_dynamic, result_tx);
                    return;
                }
                Err(AudioPrepError::InvalidConfig(error)) => {
                    send_audio_prep_result(
                        result_tx,
                        audio_config_failure(revision, request_id.clone(), error),
                    );
                    send_dynamic_events(audio, pending_dynamic, result_tx);
                    return;
                }
                Err(AudioPrepError::Failed(error)) => {
                    send_audio_prep_result(
                        result_tx,
                        audio_prep_failure(revision, request_id.clone(), error),
                    );
                    send_dynamic_events(audio, pending_dynamic, result_tx);
                    return;
                }
            };
        let mut next_revision = revision;
        let mut next_request_id = request_id.clone();
        let mut next_config = config.clone();
        let mut next_samples_dir = samples_dir.clone();
        if drain_pending_requests(
            rx,
            &mut next_revision,
            &mut next_request_id,
            &mut next_config,
            &mut next_samples_dir,
            &mut pending_dynamic,
        ) {
            revision = next_revision;
            request_id = next_request_id;
            config = next_config;
            samples_dir = next_samples_dir;
            continue;
        }
        match apply_prepared_audio_config(audio, prepared) {
            Ok(()) => {
                send_audio_prep_result(result_tx, audio_prep_success(revision, request_id.clone()))
            }
            Err(error) => send_audio_prep_result(
                result_tx,
                audio_prep_failure(revision, request_id.clone(), error),
            ),
        }
        send_dynamic_events(audio, pending_dynamic, result_tx);
        return;
    }
}

fn drain_pending_requests(
    rx: &Receiver<AudioControlRequest>,
    revision: &mut u64,
    request_id: &mut Option<String>,
    config: &mut serde_json::Value,
    samples_dir: &mut PathBuf,
    pending_dynamic: &mut Vec<EngineEvent>,
) -> bool {
    let mut had_full_config = false;
    while let Ok(request) = rx.try_recv() {
        match request {
            AudioControlRequest::FullConfig {
                revision: next_revision,
                request_id: next_request_id,
                config: next_config,
                samples_dir: next_samples_dir,
            } => {
                *revision = next_revision;
                *request_id = next_request_id;
                *config = next_config;
                *samples_dir = next_samples_dir;
                had_full_config = true;
                pending_dynamic.retain(is_realtime_dynamic_event);
            }
            AudioControlRequest::Dynamic(event) => pending_dynamic.push(*event),
        }
    }
    had_full_config
}

struct PreparedAudioConfig {
    event: EngineEvent,
    sample_signature: Option<String>,
}

enum AudioPrepError {
    Superseded,
    InvalidConfig(String),
    Failed(String),
}

fn prepare_audio_config(
    audio: &AudioService,
    revision: u64,
    config: serde_json::Value,
    samples_dir: PathBuf,
) -> Result<PreparedAudioConfig, AudioPrepError> {
    let parsed = parse_audio_config(&config).map_err(AudioPrepError::InvalidConfig)?;
    let next_signature = sample_signature(&parsed);
    let should_update_sample_banks = {
        let current = audio
            .sample_bank_signature
            .lock()
            .map_err(|_| AudioPrepError::Failed("sample bank signature lock failed".into()))?;
        *current != next_signature
    };
    ensure_current_audio_revision(audio.config_revision.load(Ordering::SeqCst), revision)?;
    let sample_banks = if should_update_sample_banks {
        Some(sample_banks(&parsed, &samples_dir, audio))
    } else {
        None
    };
    ensure_current_audio_revision(audio.config_revision.load(Ordering::SeqCst), revision)?;
    Ok(PreparedAudioConfig {
        event: EngineEvent::SetPreparedAudioConfig(prepare_engine_audio_config(
            parsed.instruments_config(),
            sample_banks,
            parsed.voice_stealing_mode,
            DEFAULT_AUDIO_SAMPLE_RATE,
        )),
        sample_signature: should_update_sample_banks.then_some(next_signature),
    })
}

fn ensure_current_audio_revision(current: u64, expected: u64) -> Result<(), AudioPrepError> {
    (current == expected)
        .then_some(())
        .ok_or(AudioPrepError::Superseded)
}

fn apply_prepared_audio_config(
    audio: &AudioService,
    prepared: PreparedAudioConfig,
) -> Result<(), String> {
    if prepared.sample_signature.is_some() && audio.sample_bank_signature.lock().is_err() {
        return Err("sample bank signature lock failed".into());
    }
    audio.broadcast(prepared.event)?;
    if let Some(signature) = prepared.sample_signature {
        if let Ok(mut current) = audio.sample_bank_signature.lock() {
            *current = signature;
        } else {
            return Err("sample bank signature lock failed".into());
        }
    }
    Ok(())
}

fn send_audio_prep_result(result_tx: &Sender<HostMessage>, result: RuntimeStoreResult) {
    let _ = result_tx.send(HostMessage::RuntimeResult { result });
}

fn audio_prep_success(revision: u64, request_id: Option<String>) -> RuntimeStoreResult {
    identify_audio_prep_result(
        RuntimeStoreResult::OperationSucceeded {
            operation: RuntimeOperation::AudioCommand,
            request_id: None,
            revision: Some(revision),
        },
        request_id,
        revision,
    )
}

fn audio_prep_failure(
    revision: u64,
    request_id: Option<String>,
    message: String,
) -> RuntimeStoreResult {
    identify_audio_prep_result(
        RuntimeStoreResult::RuntimeFailure {
            error: RuntimeErrorFacts::new(
                RuntimeErrorDomain::Audio,
                RuntimeErrorCode::OperationFailed,
                RuntimeOperation::AudioCommand,
                Some(message),
            ),
        },
        request_id,
        revision,
    )
}

fn audio_config_failure(
    revision: u64,
    request_id: Option<String>,
    message: String,
) -> RuntimeStoreResult {
    identify_audio_prep_result(
        RuntimeStoreResult::RuntimeFailure {
            error: RuntimeErrorFacts::new(
                RuntimeErrorDomain::Audio,
                RuntimeErrorCode::InvalidPayload,
                RuntimeOperation::AudioCommand,
                Some(message),
            ),
        },
        request_id,
        revision,
    )
}

fn identify_audio_prep_result(
    result: RuntimeStoreResult,
    request_id: Option<String>,
    revision: u64,
) -> RuntimeStoreResult {
    match request_id {
        Some(request_id) => result.with_identity(request_id, Some(revision)),
        None => result,
    }
}

fn send_dynamic_events(
    audio: &AudioService,
    events: Vec<EngineEvent>,
    result_tx: &Sender<HostMessage>,
) {
    for event in events {
        if let Err(error) = audio.broadcast(event) {
            send_audio_prep_result(result_tx, audio_queue_failure(error));
        }
    }
}

fn audio_queue_failure(message: String) -> RuntimeStoreResult {
    RuntimeStoreResult::RuntimeFailure {
        error: RuntimeErrorFacts::new(
            RuntimeErrorDomain::Audio,
            RuntimeErrorCode::OperationFailed,
            RuntimeOperation::AudioCommand,
            Some(message),
        ),
    }
}

fn is_realtime_dynamic_event(event: &EngineEvent) -> bool {
    matches!(
        event,
        EngineEvent::AllNotesOff
            | EngineEvent::NoteOn { .. }
            | EngineEvent::NoteOff { .. }
            | EngineEvent::Cc { .. }
            | EngineEvent::PreviewSample { .. }
            | EngineEvent::MomentaryFxStart { .. }
            | EngineEvent::MomentaryFxUpdate { .. }
            | EngineEvent::MomentaryFxStop { .. }
            | EngineEvent::ProbeMark { .. }
    )
}

#[cfg(test)]
mod tests {
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

        assert!(drain_pending_requests(
            &rx,
            &mut revision,
            &mut request_id,
            &mut config,
            &mut samples_dir,
            &mut pending,
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

        assert!(drain_pending_requests(
            &rx,
            &mut revision,
            &mut request_id,
            &mut config,
            &mut samples_dir,
            &mut pending,
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
}
