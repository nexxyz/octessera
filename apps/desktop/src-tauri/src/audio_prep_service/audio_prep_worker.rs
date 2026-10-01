use super::audio_prep_config::{
    commit_full_audio_config, prepare_full_audio_config, prepare_fx_bus_slot_event,
    prepare_global_fx_slot_event, prepare_instrument_slot_event, prepare_sample_preview,
    AudioPrepError,
};
use super::audio_prep_results::{
    full_prep_failure, full_prep_failure_from_queue, full_prep_success, owner_prep_failure,
    owner_prep_failure_from_queue, sample_preview_failure, sample_preview_queue_failure,
    send_audio_prep_result,
};
use super::{
    AudioControlRequest, DesktopAudioControl, DesktopAudioPrepState, PreviewRequest,
    AUDIO_PREP_QUEUE_CAPACITY,
};
use playback_runtime::{HostMessage, RuntimeOperation, RuntimeStoreResult};
use rodio_engine_source::{EngineEvent, EngineEventSender, QueueKind, QueueSendError};
use std::collections::VecDeque;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{Receiver, Sender};
use std::time::Duration;

pub(super) fn audio_control_loop(
    rx: Receiver<AudioControlRequest>,
    engine_tx: EngineEventSender,
    result_tx: Sender<HostMessage>,
    state: DesktopAudioPrepState,
    control: DesktopAudioControl,
) {
    let mut pending = VecDeque::with_capacity(AUDIO_PREP_QUEUE_CAPACITY);
    loop {
        if let Some(request) = take_preview_latest(&control) {
            process_preview(
                request,
                &rx,
                &mut pending,
                &engine_tx,
                &result_tx,
                &state,
                &control,
            );
            continue;
        }
        let request = match receive_request(&rx, &mut pending) {
            Ok(Some(request)) => request,
            Ok(None) => continue,
            Err(()) => break,
        };
        match request {
            AudioControlRequest::FullConfig { .. } => handle_full_config_request(
                request,
                &rx,
                &mut pending,
                &engine_tx,
                &result_tx,
                &state,
            ),
            AudioControlRequest::SamplePreview(request) => process_preview(
                request,
                &rx,
                &mut pending,
                &engine_tx,
                &result_tx,
                &state,
                &control,
            ),
            request => process_owner_request(request, &engine_tx, &result_tx, &state, &control),
        }
    }
}

fn receive_request(
    rx: &Receiver<AudioControlRequest>,
    pending: &mut VecDeque<AudioControlRequest>,
) -> Result<Option<AudioControlRequest>, ()> {
    if let Some(request) = pending.pop_front() {
        return Ok(Some(request));
    }
    match rx.recv_timeout(Duration::from_millis(10)) {
        Ok(request) => Ok(Some(request)),
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => Ok(None),
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => Err(()),
    }
}

fn take_preview_latest(control: &DesktopAudioControl) -> Option<PreviewRequest> {
    control
        .preview_latest
        .lock()
        .ok()
        .and_then(|mut latest| latest.take())
}

fn drain_into_pending(
    rx: &Receiver<AudioControlRequest>,
    pending: &mut VecDeque<AudioControlRequest>,
) {
    while pending.len() < AUDIO_PREP_QUEUE_CAPACITY {
        match rx.try_recv() {
            Ok(request) => pending.push_back(request),
            Err(_) => break,
        }
    }
}

pub(super) fn handle_full_config_request(
    request: AudioControlRequest,
    rx: &Receiver<AudioControlRequest>,
    pending: &mut VecDeque<AudioControlRequest>,
    engine_tx: &EngineEventSender,
    result_tx: &Sender<HostMessage>,
    state: &DesktopAudioPrepState,
) {
    let mut current = request;
    loop {
        let (latest, retained) = coalesce_full_request(current, rx, pending);
        *pending = retained;
        current = latest;
        let AudioControlRequest::FullConfig {
            sequence,
            revision,
            generation,
            request_id,
            config,
            ..
        } = current
        else {
            return;
        };
        let prepared = match prepare_full_audio_config(
            revision,
            generation,
            request_id.clone(),
            config,
            state,
        ) {
            Ok(prepared) => prepared,
            Err(AudioPrepError::Superseded) => {
                let Some(next) = take_newer_full_after(sequence, rx, pending) else {
                    return;
                };
                current = next;
                continue;
            }
            Err(error) => {
                send_audio_prep_result(result_tx, full_prep_failure(revision, request_id, error));
                return;
            }
        };
        if let Some(next) = take_newer_full_after(sequence, rx, pending) {
            current = next;
            continue;
        }
        match send_prepared_event(engine_tx, prepared.event.clone()) {
            Ok(()) => {
                let _ = commit_full_audio_config(&prepared, state, generation);
                send_audio_prep_result(
                    result_tx,
                    full_prep_success(revision, request_id, generation),
                );
            }
            Err(error) => send_audio_prep_result(
                result_tx,
                full_prep_failure_from_queue(revision, request_id, error),
            ),
        }
        return;
    }
}

fn coalesce_full_request(
    initial: AudioControlRequest,
    rx: &Receiver<AudioControlRequest>,
    pending: &mut VecDeque<AudioControlRequest>,
) -> (AudioControlRequest, VecDeque<AudioControlRequest>) {
    drain_into_pending(rx, pending);
    let mut requests = VecDeque::with_capacity(AUDIO_PREP_QUEUE_CAPACITY * 2);
    requests.push_back(initial);
    requests.extend(pending.drain(..));
    let mut latest_full = None;
    let mut retained = VecDeque::with_capacity(AUDIO_PREP_QUEUE_CAPACITY);
    for request in requests {
        if matches!(request, AudioControlRequest::FullConfig { .. }) {
            latest_full = Some(request);
            retained.clear();
        } else if latest_full
            .as_ref()
            .is_some_and(|full| request_sequence(&request) > request_sequence(full))
            && retained.len() < AUDIO_PREP_QUEUE_CAPACITY
        {
            retained.push_back(request);
        }
    }
    (latest_full.expect("full request is present"), retained)
}

fn request_sequence(request: &AudioControlRequest) -> u64 {
    match request {
        AudioControlRequest::FullConfig { sequence, .. }
        | AudioControlRequest::InstrumentSlot { sequence, .. }
        | AudioControlRequest::FxBusSlot { sequence, .. }
        | AudioControlRequest::GlobalFxSlot { sequence, .. } => *sequence,
        AudioControlRequest::SamplePreview(request) => request.sequence,
    }
}

fn take_newer_full_after(
    barrier_sequence: u64,
    rx: &Receiver<AudioControlRequest>,
    pending: &mut VecDeque<AudioControlRequest>,
) -> Option<AudioControlRequest> {
    drain_into_pending(rx, pending);
    let mut requests = VecDeque::with_capacity(AUDIO_PREP_QUEUE_CAPACITY * 2);
    requests.extend(pending.drain(..));
    let mut latest_full = None;
    let mut retained = VecDeque::with_capacity(AUDIO_PREP_QUEUE_CAPACITY);
    for request in requests {
        if request_sequence(&request) <= barrier_sequence {
            continue;
        }
        if matches!(request, AudioControlRequest::FullConfig { .. }) {
            latest_full = Some(request);
            retained.clear();
        } else if retained.len() < AUDIO_PREP_QUEUE_CAPACITY {
            retained.push_back(request);
        }
    }
    *pending = retained;
    latest_full
}

fn process_owner_request(
    request: AudioControlRequest,
    engine_tx: &EngineEventSender,
    result_tx: &Sender<HostMessage>,
    state: &DesktopAudioPrepState,
    control: &DesktopAudioControl,
) {
    let generation_request = request.clone();
    let prepared = match request {
        AudioControlRequest::InstrumentSlot {
            instrument_slot,
            generation,
            config,
            ..
        } => prepare_instrument_slot_event(instrument_slot, generation, config, state),
        AudioControlRequest::FxBusSlot {
            bus_index,
            slot_index,
            generation,
            fx_type,
            params,
            ..
        } => Ok(prepare_fx_bus_slot_event(
            bus_index, slot_index, generation, fx_type, params,
        )),
        AudioControlRequest::GlobalFxSlot {
            slot_index,
            generation,
            fx_type,
            params,
            ..
        } => Ok(prepare_global_fx_slot_event(
            slot_index, generation, fx_type, params,
        )),
        AudioControlRequest::FullConfig { .. } | AudioControlRequest::SamplePreview(_) => return,
    };
    let event = match prepared {
        Ok(event) => event,
        Err(error) => {
            send_audio_prep_result(result_tx, owner_prep_failure(error));
            return;
        }
    };
    if let Err(error) = send_prepared_event(engine_tx, event.clone()) {
        send_audio_prep_result(result_tx, owner_prep_failure_from_queue(error));
        return;
    }
    control.commit_owner_generation(&generation_request, &event);
}

fn process_preview(
    request: PreviewRequest,
    rx: &Receiver<AudioControlRequest>,
    pending: &mut VecDeque<AudioControlRequest>,
    engine_tx: &EngineEventSender,
    result_tx: &Sender<HostMessage>,
    state: &DesktopAudioPrepState,
    control: &DesktopAudioControl,
) {
    let mut latest = request;
    while pending.len() < AUDIO_PREP_QUEUE_CAPACITY {
        match rx.try_recv() {
            Ok(AudioControlRequest::SamplePreview(next)) if next.token > latest.token => {
                latest = next
            }
            Ok(request) if pending.len() < AUDIO_PREP_QUEUE_CAPACITY => pending.push_back(request),
            Err(_) => break,
            Ok(_) => {}
        }
    }
    if let Ok(mut slot) = control.preview_latest.lock() {
        if slot.as_ref().is_some_and(|next| next.token > latest.token) {
            latest = slot.take().expect("preview latest slot");
        } else {
            slot.take();
        }
    }
    if latest.token != control.next_preview_token.load(Ordering::Acquire) {
        return;
    }
    let generation = state
        .generations
        .lock()
        .map(|generations| generations.sample[latest.instrument_slot])
        .unwrap_or(0);
    #[cfg(test)]
    if let Some(gate) = &control.preview_prepare_gate {
        gate.started.store(true, Ordering::SeqCst);
        while !gate.release.load(Ordering::Acquire) {
            std::thread::yield_now();
        }
    }
    let event = match prepare_sample_preview(
        latest.instrument_slot,
        generation,
        &latest.path,
        latest.velocity,
        state,
    ) {
        Ok(event) => event,
        Err(error) => {
            if latest.token == control.next_preview_token.load(Ordering::Acquire) {
                send_audio_prep_result(result_tx, sample_preview_failure(error));
            }
            return;
        }
    };
    if latest.token != control.next_preview_token.load(Ordering::Acquire) {
        return;
    }
    match send_prepared_event(engine_tx, event) {
        Ok(()) => send_audio_prep_result(
            result_tx,
            RuntimeStoreResult::OperationSucceeded {
                operation: RuntimeOperation::SamplePreview,
                request_id: None,
                revision: None,
            },
        ),
        Err(error) => send_audio_prep_result(result_tx, sample_preview_queue_failure(error)),
    }
}

fn send_prepared_event(
    engine_tx: &EngineEventSender,
    event: EngineEvent,
) -> Result<(), QueueSendError> {
    match engine_tx.send(event) {
        Ok(()) => Ok(()),
        Err(
            error @ QueueSendError::Full {
                queue: QueueKind::Musical | QueueKind::Structural,
            },
        ) => {
            let _ = engine_tx.send(EngineEvent::AllNotesOff);
            Err(error)
        }
        Err(error) => Err(error),
    }
}
