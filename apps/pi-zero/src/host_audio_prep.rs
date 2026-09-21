use crate::audio::{AudioControlRequest, AudioService, AUDIO_PREP_QUEUE_CAPACITY};
#[path = "host_audio_prep_config.rs"]
mod config_prep;
#[path = "host_audio_prep_owner.rs"]
mod owner_prep;
#[path = "host_audio_prep_results.rs"]
mod prep_results;
#[path = "host_audio_preview_prep.rs"]
mod preview_prep;
use config_prep::{apply_prepared_audio_config, prepare_audio_config};
use owner_prep::{process_instrument_slot, process_owner_request, OwnerRequest};
use playback_runtime::HostMessage;
use prep_results::{
    audio_config_failure, audio_prep_failure, audio_prep_success, audio_queue_failure,
    sample_failure, send_audio_prep_result, AudioPrepError,
};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};

const PREP_DRAIN_BATCH: usize = 8;

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
    let mut pending_owners = VecDeque::new();
    let mut pending_preview = None;
    while let Ok(request) = rx.recv() {
        process_audio_control_request(
            request,
            &rx,
            &audio,
            &result_tx,
            &mut pending_owners,
            &mut pending_preview,
        );
    }
}

fn process_audio_control_request(
    request: AudioControlRequest,
    rx: &Receiver<AudioControlRequest>,
    audio: &AudioService,
    result_tx: &Sender<HostMessage>,
    pending_owners: &mut VecDeque<OwnerRequest>,
    pending_preview: &mut Option<preview_prep::PreviewRequest>,
) {
    match request {
        AudioControlRequest::FullConfig {
            sequence,
            revision,
            generation,
            request_id,
            config,
            samples_dir,
        } => {
            audio
                .latest_full_sequence
                .fetch_max(sequence, std::sync::atomic::Ordering::Release);
            handle_full_config_request(
                FullConfigState {
                    sequence,
                    revision,
                    generation,
                    request_id,
                    config,
                    samples_dir,
                },
                rx,
                audio,
                result_tx,
                pending_owners,
                pending_preview,
            )
        }
        AudioControlRequest::InstrumentSlot {
            sequence,
            instrument_slot,
            generation,
            config,
            samples_dir,
        } => process_instrument_slot(
            audio,
            sequence,
            instrument_slot,
            generation,
            config,
            samples_dir,
            result_tx,
        ),
        AudioControlRequest::FxBusSlot {
            sequence,
            bus_index,
            slot_index,
            generation,
            fx_type,
            params,
        } => process_owner_request(
            audio,
            OwnerRequest::FxBus {
                sequence,
                bus_index,
                slot_index,
                generation,
                fx_type,
                params,
            },
            result_tx,
        ),
        AudioControlRequest::GlobalFxSlot {
            sequence,
            slot_index,
            generation,
            fx_type,
            params,
        } => process_owner_request(
            audio,
            OwnerRequest::GlobalFx {
                sequence,
                slot_index,
                generation,
                fx_type,
                params,
            },
            result_tx,
        ),
        AudioControlRequest::SamplePreview {
            sequence: _,
            instrument_slot,
            path,
            velocity,
            samples_dir,
            preview_token,
        } => preview_prep::process_request(
            audio,
            instrument_slot,
            &path,
            velocity,
            &samples_dir,
            preview_token,
            result_tx,
        ),
    }
}

struct FullConfigState {
    sequence: u64,
    revision: u64,
    generation: u64,
    request_id: Option<String>,
    config: serde_json::Value,
    samples_dir: PathBuf,
}

fn handle_full_config_request(
    mut state: FullConfigState,
    rx: &Receiver<AudioControlRequest>,
    audio: &AudioService,
    result_tx: &Sender<HostMessage>,
    pending_owners: &mut VecDeque<OwnerRequest>,
    pending_preview: &mut Option<preview_prep::PreviewRequest>,
) {
    'prepare: loop {
        let prepared = match prepare_audio_config(
            audio,
            state.revision,
            state.generation,
            state.config.clone(),
            state.samples_dir.clone(),
        ) {
            Ok(prepared) => prepared,
            Err(AudioPrepError::Superseded) => {
                let _ = drain_pending_requests(
                    rx,
                    Some(audio),
                    &mut state,
                    pending_owners,
                    pending_preview,
                    Some(result_tx),
                );
                continue;
            }
            Err(AudioPrepError::InvalidConfig(error)) => {
                send_audio_prep_result(
                    result_tx,
                    audio_config_failure(state.revision, state.request_id.clone(), error),
                );
                return;
            }
            Err(AudioPrepError::Sample(error)) => {
                send_audio_prep_result(
                    result_tx,
                    sample_failure(
                        state.revision,
                        state.request_id.clone(),
                        error.code(),
                        error.message(),
                    ),
                );
                return;
            }
            Err(AudioPrepError::Failed(error)) => {
                send_audio_prep_result(
                    result_tx,
                    audio_prep_failure(state.revision, state.request_id.clone(), error),
                );
                return;
            }
        };

        if drain_pending_requests(
            rx,
            Some(audio),
            &mut state,
            pending_owners,
            pending_preview,
            Some(result_tx),
        ) {
            continue;
        }
        match apply_prepared_audio_config(audio, prepared, state.generation) {
            Ok(()) => send_audio_prep_result(
                result_tx,
                audio_prep_success(state.revision, state.request_id.clone()),
            ),
            Err(error) => send_audio_prep_result(
                result_tx,
                audio_prep_failure(state.revision, state.request_id.clone(), error),
            ),
        }

        if drain_pending_requests(
            rx,
            Some(audio),
            &mut state,
            pending_owners,
            pending_preview,
            Some(result_tx),
        ) {
            continue;
        }
        loop {
            if pending_owners.is_empty() && pending_preview.is_none() {
                break;
            }
            if pending_preview.as_ref().is_some_and(|preview| {
                pending_owners
                    .front()
                    .is_none_or(|owner| preview.sequence < owner_sequence(owner))
            }) {
                let preview = pending_preview.take().expect("preview request exists");
                preview_prep::process_request(
                    audio,
                    preview.instrument_slot,
                    &preview.path,
                    preview.velocity,
                    &preview.samples_dir,
                    preview.preview_token,
                    result_tx,
                );
            } else if let Some(owner) = pending_owners.pop_front() {
                process_owner_request(audio, owner, result_tx);
            }
            if drain_pending_requests(
                rx,
                Some(audio),
                &mut state,
                pending_owners,
                pending_preview,
                Some(result_tx),
            ) {
                continue 'prepare;
            }
        }
        if drain_pending_requests(
            rx,
            Some(audio),
            &mut state,
            pending_owners,
            pending_preview,
            Some(result_tx),
        ) {
            continue;
        }
        return;
    }
}

fn owner_sequence(owner: &OwnerRequest) -> u64 {
    match owner {
        OwnerRequest::Instrument { sequence, .. }
        | OwnerRequest::FxBus { sequence, .. }
        | OwnerRequest::GlobalFx { sequence, .. } => *sequence,
    }
}

fn drain_pending_requests(
    rx: &Receiver<AudioControlRequest>,
    audio: Option<&AudioService>,
    state: &mut FullConfigState,
    pending_owners: &mut VecDeque<OwnerRequest>,
    pending_preview: &mut Option<preview_prep::PreviewRequest>,
    result_tx: Option<&Sender<HostMessage>>,
) -> bool {
    let mut had_full = false;
    for _ in 0..PREP_DRAIN_BATCH {
        let Ok(request) = rx.try_recv() else {
            break;
        };
        match request {
            AudioControlRequest::FullConfig {
                sequence: next_sequence,
                revision: next_revision,
                generation: next_generation,
                request_id: next_request_id,
                config: next_config,
                samples_dir: next_samples_dir,
            } => {
                if let Some(audio) = audio {
                    audio
                        .latest_full_sequence
                        .fetch_max(next_sequence, std::sync::atomic::Ordering::Release);
                }
                state.revision = next_revision;
                state.sequence = next_sequence;
                state.generation = next_generation;
                state.request_id = next_request_id;
                state.config = next_config;
                state.samples_dir = next_samples_dir;
                pending_owners.clear();
                had_full = true;
            }
            AudioControlRequest::InstrumentSlot {
                sequence,
                instrument_slot,
                generation,
                config,
                samples_dir,
            } => queue_owner_request(
                pending_owners,
                pending_preview,
                OwnerRequest::Instrument {
                    sequence,
                    instrument_slot,
                    generation,
                    config,
                    samples_dir,
                },
                result_tx,
            ),
            AudioControlRequest::FxBusSlot {
                sequence,
                bus_index,
                slot_index,
                generation,
                fx_type,
                params,
            } => queue_owner_request(
                pending_owners,
                pending_preview,
                OwnerRequest::FxBus {
                    sequence,
                    bus_index,
                    slot_index,
                    generation,
                    fx_type,
                    params,
                },
                result_tx,
            ),
            AudioControlRequest::GlobalFxSlot {
                sequence,
                slot_index,
                generation,
                fx_type,
                params,
            } => queue_owner_request(
                pending_owners,
                pending_preview,
                OwnerRequest::GlobalFx {
                    sequence,
                    slot_index,
                    generation,
                    fx_type,
                    params,
                },
                result_tx,
            ),
            AudioControlRequest::SamplePreview {
                sequence,
                instrument_slot,
                path,
                velocity,
                samples_dir,
                preview_token,
            } => {
                if pending_work_len(pending_owners, pending_preview) >= AUDIO_PREP_QUEUE_CAPACITY
                    && pending_preview.is_none()
                {
                    if let Some(result_tx) = result_tx {
                        send_audio_prep_result(
                            result_tx,
                            audio_queue_failure("audio preparation queue capacity exceeded".into()),
                        );
                    }
                    continue;
                }
                *pending_preview = Some(preview_prep::PreviewRequest {
                    sequence,
                    instrument_slot,
                    path,
                    velocity,
                    samples_dir,
                    preview_token,
                });
            }
        }
    }
    had_full
}

fn queue_owner_request(
    pending_owners: &mut VecDeque<OwnerRequest>,
    pending_preview: &Option<preview_prep::PreviewRequest>,
    request: OwnerRequest,
    result_tx: Option<&Sender<HostMessage>>,
) {
    if pending_work_len(pending_owners, pending_preview) >= AUDIO_PREP_QUEUE_CAPACITY {
        if let Some(result_tx) = result_tx {
            send_audio_prep_result(
                result_tx,
                audio_queue_failure("audio preparation queue capacity exceeded".into()),
            );
        }
        return;
    }
    pending_owners.push_back(request);
}

fn pending_work_len(
    pending_owners: &VecDeque<OwnerRequest>,
    pending_preview: &Option<preview_prep::PreviewRequest>,
) -> usize {
    pending_owners.len() + usize::from(pending_preview.is_some())
}

#[cfg(test)]
#[path = "host_audio_prep_tests.rs"]
mod tests;
