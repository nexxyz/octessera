#[path = "audio_prep_config.rs"]
mod audio_prep_config;

use crate::types::QueuedAudioEvent;
use audio_prep_config::{apply_prepared_audio_config, prepare_full_audio_config, AudioPrepError};
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
