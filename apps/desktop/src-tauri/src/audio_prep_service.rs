use crate::audio_config::{
    decode_sample_file, normalize_config, sample_bank_signature, sample_banks, synth_payload,
    synth_slots,
};
use crate::samples::resolve_sample_file;
use crate::types::QueuedAudioEvent;
use playback_runtime::{
    HostMessage, RuntimeErrorCode, RuntimeErrorDomain, RuntimeErrorFacts, RuntimeOperation,
    RuntimeStoreResult,
};
use realtime_engine::synth::INSTRUMENT_SLOT_COUNT;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub(crate) struct DesktopAudioControl {
    tx: Sender<AudioControlRequest>,
    config_revision: Arc<AtomicU64>,
}

pub(crate) struct DesktopAudioPrepState {
    pub(crate) config_revision: Arc<AtomicU64>,
    pub(crate) synth_slots: Arc<Mutex<[bool; INSTRUMENT_SLOT_COUNT]>>,
    pub(crate) sample_cache: Arc<Mutex<HashMap<String, realtime_engine::synth::SampleBuffer>>>,
    pub(crate) sample_bank_signature: Arc<Mutex<String>>,
}

enum AudioControlRequest {
    FullConfig {
        revision: u64,
        request_id: Option<String>,
        config: Value,
    },
    Dynamic(QueuedAudioEvent),
}

struct PreparedAudioConfig {
    event: QueuedAudioEvent,
    synth_slots: [bool; INSTRUMENT_SLOT_COUNT],
    sample_signature: Option<String>,
}

pub(crate) fn spawn_desktop_audio_control(
    trigger_tx: Sender<QueuedAudioEvent>,
    state: DesktopAudioPrepState,
) -> (DesktopAudioControl, Receiver<HostMessage>) {
    let (tx, rx) = mpsc::channel();
    let (result_tx, result_rx) = mpsc::channel();
    let config_revision = state.config_revision.clone();
    std::thread::spawn(move || audio_control_loop(rx, trigger_tx, result_tx, state));
    (
        DesktopAudioControl {
            tx,
            config_revision,
        },
        result_rx,
    )
}

impl DesktopAudioControl {
    pub(crate) fn enqueue_full_config(
        &self,
        revision: u64,
        request_id: Option<String>,
        config: Value,
    ) -> Result<(), String> {
        self.config_revision.fetch_max(revision, Ordering::SeqCst);
        self.tx
            .send(AudioControlRequest::FullConfig {
                revision,
                request_id,
                config,
            })
            .map_err(|e| format!("audio prep queue send failed: {e}"))
    }

    pub(crate) fn enqueue_dynamic(&self, event: QueuedAudioEvent) -> Result<(), String> {
        self.tx
            .send(AudioControlRequest::Dynamic(event))
            .map_err(|e| format!("audio control queue send failed: {e}"))
    }
}

fn audio_control_loop(
    rx: Receiver<AudioControlRequest>,
    trigger_tx: Sender<QueuedAudioEvent>,
    result_tx: Sender<HostMessage>,
    state: DesktopAudioPrepState,
) {
    while let Ok(request) = rx.recv() {
        match request {
            AudioControlRequest::Dynamic(event) => {
                let _ = trigger_tx.send(event);
            }
            AudioControlRequest::FullConfig {
                revision,
                request_id,
                config,
            } => {
                handle_full_config_request_with_result(
                    revision,
                    request_id,
                    config,
                    &rx,
                    &trigger_tx,
                    &result_tx,
                    &state,
                );
            }
        }
    }
}

#[cfg(test)]
fn handle_full_config_request(
    revision: u64,
    request_id: Option<String>,
    config: Value,
    rx: &Receiver<AudioControlRequest>,
    trigger_tx: &Sender<QueuedAudioEvent>,
    state: &DesktopAudioPrepState,
) {
    let (result_tx, _result_rx) = mpsc::channel::<HostMessage>();
    handle_full_config_request_with_result(
        revision, request_id, config, rx, trigger_tx, &result_tx, state,
    );
}

fn handle_full_config_request_with_result(
    mut revision: u64,
    mut request_id: Option<String>,
    mut config: Value,
    rx: &Receiver<AudioControlRequest>,
    trigger_tx: &Sender<QueuedAudioEvent>,
    result_tx: &Sender<HostMessage>,
    state: &DesktopAudioPrepState,
) {
    let mut pending_dynamic = Vec::new();
    state.config_revision.fetch_max(revision, Ordering::SeqCst);
    let had_initial_full_config = drain_pending_requests(
        rx,
        &mut revision,
        &mut request_id,
        &mut config,
        &mut pending_dynamic,
    );
    if had_initial_full_config {
        state.config_revision.fetch_max(revision, Ordering::SeqCst);
    }
    loop {
        let prepared =
            match prepare_full_audio_config(revision, request_id.clone(), config.clone(), state) {
                Ok(prepared) => prepared,
                Err(AudioPrepError::Superseded) => {
                    send_dynamic_events(trigger_tx, pending_dynamic);
                    return;
                }
                Err(AudioPrepError::InvalidConfig(error)) => {
                    send_audio_prep_result(
                        result_tx,
                        audio_config_failure(revision, request_id.clone(), error),
                    );
                    send_dynamic_events(trigger_tx, pending_dynamic);
                    return;
                }
                Err(AudioPrepError::Failed(error)) => {
                    send_audio_prep_result(
                        result_tx,
                        audio_prep_failure(revision, request_id.clone(), error),
                    );
                    send_dynamic_events(trigger_tx, pending_dynamic);
                    return;
                }
            };
        let mut newer_revision = revision;
        let mut newer_request_id = request_id.clone();
        let mut newer_config = config.clone();
        let had_newer = drain_pending_requests(
            rx,
            &mut newer_revision,
            &mut newer_request_id,
            &mut newer_config,
            &mut pending_dynamic,
        );
        if had_newer {
            revision = newer_revision;
            state.config_revision.fetch_max(revision, Ordering::SeqCst);
            request_id = newer_request_id;
            config = newer_config;
            continue;
        }
        match apply_prepared_audio_config(prepared, revision, trigger_tx, state) {
            Ok(()) => {
                send_audio_prep_result(result_tx, audio_prep_success(revision, request_id.clone()))
            }
            Err(AudioPrepError::Superseded) => {}
            Err(AudioPrepError::InvalidConfig(error)) => send_audio_prep_result(
                result_tx,
                audio_config_failure(revision, request_id.clone(), error),
            ),
            Err(AudioPrepError::Failed(error)) => send_audio_prep_result(
                result_tx,
                audio_prep_failure(revision, request_id.clone(), error),
            ),
        }
        send_dynamic_events(trigger_tx, pending_dynamic);
        return;
    }
}

fn drain_pending_requests(
    rx: &Receiver<AudioControlRequest>,
    revision: &mut u64,
    request_id: &mut Option<String>,
    config: &mut Value,
    pending_dynamic: &mut Vec<QueuedAudioEvent>,
) -> bool {
    let mut had_full_config = false;
    while let Ok(request) = rx.try_recv() {
        match request {
            AudioControlRequest::FullConfig {
                revision: next_revision,
                request_id: next_request_id,
                config: next_config,
            } => {
                *revision = next_revision;
                *request_id = next_request_id;
                *config = next_config;
                had_full_config = true;
                pending_dynamic.retain(is_realtime_dynamic_event);
            }
            AudioControlRequest::Dynamic(event) => pending_dynamic.push(event),
        }
    }
    had_full_config
}

fn prepare_full_audio_config(
    revision: u64,
    request_id: Option<String>,
    config: Value,
    state: &DesktopAudioPrepState,
) -> Result<PreparedAudioConfig, AudioPrepError> {
    ensure_current_audio_revision(state, revision)?;
    let config = normalize_config(&config).map_err(AudioPrepError::InvalidConfig)?;
    let next_slots = synth_slots(&config);
    let next_sample_signature = sample_bank_signature(&config);
    let should_update_sample_banks = {
        let current = state
            .sample_bank_signature
            .lock()
            .map_err(|_| AudioPrepError::Failed("sample bank signature lock failed".into()))?;
        *current != next_sample_signature
    };
    let next_sample_banks = if should_update_sample_banks {
        Some(sample_banks(&config, resolve_sample_file, |path| {
            if let Ok(cache) = state.sample_cache.lock() {
                if let Some(buffer) = cache.get(path) {
                    return Some(buffer.clone());
                }
            } else {
                return None;
            }
            let buffer = decode_sample_file(path)?;
            if let Ok(mut cache) = state.sample_cache.lock() {
                cache.insert(path.to_string(), buffer.clone());
            }
            Some(buffer)
        }))
    } else {
        None
    };
    ensure_current_audio_revision(state, revision)?;
    Ok(PreparedAudioConfig {
        event: QueuedAudioEvent::SetAudioConfig {
            revision,
            request_id,
            instruments: synth_payload(&config),
            sample_banks: next_sample_banks,
            voice_stealing_mode: config.voice_stealing_mode,
        },
        synth_slots: next_slots,
        sample_signature: should_update_sample_banks.then_some(next_sample_signature),
    })
}

enum AudioPrepError {
    Superseded,
    InvalidConfig(String),
    Failed(String),
}

fn ensure_current_audio_revision(
    state: &DesktopAudioPrepState,
    revision: u64,
) -> Result<(), AudioPrepError> {
    (state.config_revision.load(Ordering::SeqCst) == revision)
        .then_some(())
        .ok_or(AudioPrepError::Superseded)
}

fn apply_prepared_audio_config(
    prepared: PreparedAudioConfig,
    revision: u64,
    trigger_tx: &Sender<QueuedAudioEvent>,
    state: &DesktopAudioPrepState,
) -> Result<(), AudioPrepError> {
    ensure_current_audio_revision(state, revision)?;
    if state.synth_slots.lock().is_err() {
        return Err(AudioPrepError::Failed(
            "synth slot state lock failed".into(),
        ));
    }
    if prepared.sample_signature.is_some() && state.sample_bank_signature.lock().is_err() {
        return Err(AudioPrepError::Failed(
            "sample bank signature lock failed".into(),
        ));
    }
    trigger_tx.send(prepared.event).map_err(|error| {
        AudioPrepError::Failed(format!("audio engine queue send failed: {error}"))
    })?;
    if let Ok(mut slots) = state.synth_slots.lock() {
        *slots = prepared.synth_slots;
    } else {
        return Err(AudioPrepError::Failed(
            "synth slot state lock failed".into(),
        ));
    }
    if let Some(signature) = prepared.sample_signature {
        if let Ok(mut current) = state.sample_bank_signature.lock() {
            *current = signature;
        } else {
            return Err(AudioPrepError::Failed(
                "sample bank signature lock failed".into(),
            ));
        }
    }
    Ok(())
}

fn send_audio_prep_result(result_tx: &Sender<HostMessage>, result: RuntimeStoreResult) {
    let _ = result_tx.send(HostMessage::RuntimeResult { result });
}

fn audio_prep_success(revision: u64, request_id: Option<String>) -> RuntimeStoreResult {
    let result = RuntimeStoreResult::OperationSucceeded {
        operation: RuntimeOperation::AudioCommand,
        request_id: None,
        revision: Some(revision),
    };
    identify_audio_prep_result(result, request_id, revision)
}

fn audio_prep_failure(
    revision: u64,
    request_id: Option<String>,
    message: String,
) -> RuntimeStoreResult {
    let result = RuntimeStoreResult::RuntimeFailure {
        error: RuntimeErrorFacts::new(
            RuntimeErrorDomain::Audio,
            RuntimeErrorCode::OperationFailed,
            RuntimeOperation::AudioCommand,
            Some(message),
        ),
    };
    identify_audio_prep_result(result, request_id, revision)
}

fn audio_config_failure(
    revision: u64,
    request_id: Option<String>,
    message: String,
) -> RuntimeStoreResult {
    let result = RuntimeStoreResult::RuntimeFailure {
        error: RuntimeErrorFacts::new(
            RuntimeErrorDomain::Audio,
            RuntimeErrorCode::InvalidPayload,
            RuntimeOperation::AudioCommand,
            Some(message),
        ),
    };
    identify_audio_prep_result(result, request_id, revision)
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

fn send_dynamic_events(trigger_tx: &Sender<QueuedAudioEvent>, events: Vec<QueuedAudioEvent>) {
    for event in events {
        let _ = trigger_tx.send(event);
    }
}

fn is_realtime_dynamic_event(event: &QueuedAudioEvent) -> bool {
    matches!(
        event,
        QueuedAudioEvent::AllNotesOff
            | QueuedAudioEvent::Note(_)
            | QueuedAudioEvent::NoteOff { .. }
            | QueuedAudioEvent::Cc { .. }
            | QueuedAudioEvent::PreviewSample { .. }
            | QueuedAudioEvent::MomentaryFxStart { .. }
            | QueuedAudioEvent::MomentaryFxUpdate { .. }
            | QueuedAudioEvent::MomentaryFxStop { .. }
    )
}

#[cfg(test)]
#[path = "audio_prep_service_tests.rs"]
mod extra_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::QueuedNote;
    use std::time::Duration;

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
}
