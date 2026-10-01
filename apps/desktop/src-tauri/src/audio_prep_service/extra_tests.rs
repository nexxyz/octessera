use super::*;
use playback_runtime::{
    RuntimeErrorCode, RuntimeErrorDomain, RuntimeOperation, RuntimeStoreResult,
};
use rodio_engine_source::{event_queue, EngineEvent};
use std::collections::VecDeque;
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

fn test_state() -> DesktopAudioPrepState {
    DesktopAudioPrepState {
        config_revision: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        synth_slots: Arc::new(Mutex::new([true; INSTRUMENT_SLOT_COUNT])),
        sample_decode_cache: SampleDecodeCache::new(),
        sample_bank_signature: Arc::new(Mutex::new(String::new())),
        generations: Arc::new(Mutex::new(AudioGenerationState::default())),
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

fn sampler_config(path: &str) -> Value {
    serde_json::json!({
        "type": "sampler",
        "sample": { "slots": [{ "path": path }] }
    })
}

fn recv_result(rx: &Receiver<HostMessage>) -> RuntimeStoreResult {
    match rx.recv_timeout(Duration::from_secs(1)).unwrap() {
        HostMessage::RuntimeResult { result } => result,
        _ => panic!("unexpected prep message"),
    }
}

fn wait_for_generations(
    generations: &Arc<Mutex<AudioGenerationState>>,
    instrument: u64,
    sample: u64,
) {
    let deadline = std::time::Instant::now() + Duration::from_secs(1);
    loop {
        let matches = generations
            .lock()
            .map(|current| current.instrument[0] == instrument && current.sample[0] == sample)
            .unwrap_or(false);
        if matches {
            return;
        }
        assert!(std::time::Instant::now() < deadline);
        std::thread::yield_now();
    }
}

#[test]
fn prep_failure_returns_identified_typed_fault_without_mutating_state() {
    let (engine_tx, mut engine_rx) = event_queue();
    let state = test_state();
    *state.sample_bank_signature.lock().unwrap() = "retained".into();
    let signature = state.sample_bank_signature.clone();
    let generations = state.generations.clone();
    let (control, result_rx) = spawn_desktop_audio_control(engine_tx, state);
    assert_eq!(
        control.enqueue_full_config(
            7,
            7,
            Some("audio-7".into()),
            serde_json::json!({ "instruments": "invalid" }),
        ),
        AudioPrepEnqueueResult::Accepted
    );
    assert!(
        matches!(recv_result(&result_rx), RuntimeStoreResult::Identified { request_id, revision: Some(7), result } if request_id == "audio-7" && matches!(result.as_ref(), RuntimeStoreResult::RuntimeFailure { error } if error.domain == RuntimeErrorDomain::Audio && error.code == RuntimeErrorCode::InvalidPayload && error.operation == RuntimeOperation::AudioCommand))
    );
    assert!(engine_rx.try_recv().is_err());
    assert_eq!(*signature.lock().unwrap(), "retained");
    assert_eq!(generations.lock().unwrap().full, 0);
}

#[test]
fn prep_success_emits_full_event_and_identified_result() {
    let (engine_tx, mut engine_rx) = event_queue();
    let state = test_state();
    let generations = state.generations.clone();
    let (control, result_rx) = spawn_desktop_audio_control(engine_tx, state);
    assert_eq!(
        control.enqueue_full_config(8, 8, Some("audio-8".into()), audio_config(70)),
        AudioPrepEnqueueResult::Accepted
    );
    assert!(matches!(
        next_event(&mut engine_rx),
        EngineEvent::SetPreparedAudioConfig { generation: 8, .. }
    ));
    assert!(
        matches!(recv_result(&result_rx), RuntimeStoreResult::Identified { request_id, revision: Some(8), result } if request_id == "audio-8" && matches!(result.as_ref(), RuntimeStoreResult::OperationSucceeded { operation: RuntimeOperation::AudioCommand, .. }))
    );
    assert_eq!(generations.lock().unwrap().full, 8);
    assert_eq!(generations.lock().unwrap().sample[0], 8);
}

#[test]
fn owner_generation_contract_is_atomic_and_non_sampler_keeps_sample_baseline() {
    let (engine_tx, mut engine_rx) = event_queue();
    let state = test_state();
    state
        .config_revision
        .store(17, std::sync::atomic::Ordering::SeqCst);
    state.generations.lock().unwrap().instrument[0] = 5;
    state.generations.lock().unwrap().sample[0] = 5;
    let generations = state.generations.clone();
    let (control, _result_rx) = spawn_desktop_audio_control(engine_tx, state);

    assert_eq!(
        control.enqueue_instrument_slot(0, 7, sampler_config("samples/Drum/kick/Kick2.wav")),
        AudioPrepEnqueueResult::Accepted
    );
    assert!(matches!(
        next_event(&mut engine_rx),
        EngineEvent::SetPreparedInstrumentOwner {
            generation: 7,
            sample_bank: Some(_),
            ..
        }
    ));
    wait_for_generations(&generations, 7, 7);

    assert_eq!(
        control.enqueue_instrument_slot(0, 8, serde_json::json!({ "type": "synth" })),
        AudioPrepEnqueueResult::Accepted
    );
    assert!(matches!(
        next_event(&mut engine_rx),
        EngineEvent::SetPreparedInstrumentOwner {
            generation: 8,
            sample_bank: None,
            ..
        }
    ));
    wait_for_generations(&generations, 8, 7);
    assert_eq!(
        control
            .config_revision
            .load(std::sync::atomic::Ordering::SeqCst),
        17
    );
}

#[test]
fn owner_queue_failure_keeps_previous_state_and_revision_without_partial_activation() {
    let (engine_tx, mut engine_rx) = event_queue();
    for generation in 0..64 {
        engine_tx
            .send(EngineEvent::MomentaryFxStop { epoch: generation })
            .unwrap();
    }
    let state = test_state();
    state
        .config_revision
        .store(17, std::sync::atomic::Ordering::SeqCst);
    state.generations.lock().unwrap().instrument[0] = 5;
    state.generations.lock().unwrap().sample[0] = 5;
    let generations = state.generations.clone();
    let (control, result_rx) = spawn_desktop_audio_control(engine_tx, state);
    assert_eq!(
        control.enqueue_instrument_slot(0, 7, sampler_config("samples/Drum/kick/Kick2.wav")),
        AudioPrepEnqueueResult::Accepted
    );
    assert!(matches!(
        recv_result(&result_rx),
        RuntimeStoreResult::RuntimeFailure { error }
            if error.code == RuntimeErrorCode::OperationFailed
                && error.operation == RuntimeOperation::AudioCommand
    ));
    assert_eq!(generations.lock().unwrap().instrument[0], 5);
    assert_eq!(generations.lock().unwrap().sample[0], 5);
    assert_eq!(
        control
            .config_revision
            .load(std::sync::atomic::Ordering::SeqCst),
        17
    );
    let owner_seen = std::iter::from_fn(|| engine_rx.try_recv().ok())
        .any(|event| matches!(event, EngineEvent::SetPreparedInstrumentOwner { .. }));
    assert!(!owner_seen);
}

#[test]
fn prep_queue_reports_full_and_disconnected_without_blocking() {
    let (tx, rx) = mpsc::sync_channel(AUDIO_PREP_QUEUE_CAPACITY);
    let control = DesktopAudioControl {
        tx,
        config_revision: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        next_sequence: Arc::new(AtomicU64::new(0)),
        next_preview_token: Arc::new(AtomicU64::new(0)),
        preview_latest: Arc::new(Mutex::new(None)),
        generations: Arc::new(Mutex::new(AudioGenerationState::default())),
        preview_prepare_gate: None,
    };
    for revision in 0..AUDIO_PREP_QUEUE_CAPACITY as u64 {
        assert_eq!(
            control.enqueue_full_config(revision, revision, None, audio_config(70)),
            AudioPrepEnqueueResult::Accepted
        );
    }
    assert_eq!(
        control.enqueue_full_config(99, 99, None, audio_config(70)),
        AudioPrepEnqueueResult::Full
    );
    assert_eq!(
        control
            .config_revision
            .load(std::sync::atomic::Ordering::SeqCst),
        31
    );
    drop(rx);
    assert_eq!(
        control.enqueue_full_config(100, 100, None, audio_config(70)),
        AudioPrepEnqueueResult::Disconnected
    );
    assert_eq!(
        control
            .config_revision
            .load(std::sync::atomic::Ordering::SeqCst),
        31
    );
}

#[test]
fn full_barrier_discards_older_persistent_owner_requests() {
    let (request_tx, request_rx) = mpsc::sync_channel(AUDIO_PREP_QUEUE_CAPACITY);
    request_tx
        .send(AudioControlRequest::InstrumentSlot {
            sequence: 1,
            instrument_slot: 0,
            generation: 1,
            config: serde_json::json!({ "type": "synth" }),
        })
        .unwrap();
    request_tx
        .send(AudioControlRequest::FullConfig {
            sequence: 2,
            revision: 2,
            generation: 2,
            request_id: None,
            config: audio_config(70),
        })
        .unwrap();
    drop(request_tx);
    let (engine_tx, mut engine_rx) = event_queue();
    let (result_tx, result_rx) = mpsc::channel();
    let state = test_state();
    state
        .config_revision
        .store(2, std::sync::atomic::Ordering::SeqCst);
    handle_full_config_request(
        AudioControlRequest::FullConfig {
            sequence: 0,
            revision: 1,
            generation: 1,
            request_id: None,
            config: audio_config(60),
        },
        &request_rx,
        &mut VecDeque::with_capacity(AUDIO_PREP_QUEUE_CAPACITY),
        &engine_tx,
        &result_tx,
        &state,
    );
    assert!(matches!(
        next_event(&mut engine_rx),
        EngineEvent::SetPreparedAudioConfig { generation: 2, .. }
    ));
    assert!(matches!(
        recv_result(&result_rx),
        RuntimeStoreResult::OperationSucceeded {
            operation: RuntimeOperation::AudioCommand,
            ..
        }
    ));
    assert!(engine_rx.try_recv().is_err());
}

#[test]
fn preview_decode_failure_is_reported_without_audio_event() {
    let (engine_tx, mut engine_rx) = event_queue();
    let (control, result_rx) = spawn_desktop_audio_control(engine_tx, test_state());
    assert_eq!(
        control.enqueue_sample_preview(0, "../missing.wav".into(), 96),
        AudioPrepEnqueueResult::Accepted
    );
    assert!(
        matches!(recv_result(&result_rx), RuntimeStoreResult::RuntimeFailure { error } if error.domain == RuntimeErrorDomain::Sample && error.operation == RuntimeOperation::SamplePreview && error.code == RuntimeErrorCode::NotFound)
    );
    assert!(engine_rx.try_recv().is_err());
}

#[test]
fn preview_latest_is_processed_after_full_owner_queue_without_new_input() {
    let (request_tx, request_rx) = mpsc::sync_channel(AUDIO_PREP_QUEUE_CAPACITY);
    let (engine_tx, _engine_rx) = event_queue();
    let (result_tx, result_rx) = mpsc::channel();
    let state = test_state();
    let control = DesktopAudioControl {
        tx: request_tx,
        config_revision: state.config_revision.clone(),
        next_sequence: Arc::new(AtomicU64::new(0)),
        next_preview_token: Arc::new(AtomicU64::new(0)),
        preview_latest: Arc::new(Mutex::new(None)),
        generations: state.generations.clone(),
        preview_prepare_gate: None,
    };
    for sequence in 0..AUDIO_PREP_QUEUE_CAPACITY as u64 {
        control
            .tx
            .try_send(AudioControlRequest::InstrumentSlot {
                sequence,
                instrument_slot: 0,
                generation: sequence,
                config: serde_json::json!({ "type": "synth" }),
            })
            .unwrap();
    }
    assert_eq!(
        control.enqueue_sample_preview(0, "samples/Drum/kick/Kick2.wav".into(), 96),
        AudioPrepEnqueueResult::Accepted
    );
    let worker_control = control.clone();
    std::thread::spawn(move || {
        audio_control_loop(request_rx, engine_tx, result_tx, state, worker_control)
    });
    assert!(matches!(
        recv_result(&result_rx),
        RuntimeStoreResult::OperationSucceeded {
            operation: RuntimeOperation::SamplePreview,
            ..
        }
    ));
}

#[test]
fn full_barrier_leaves_channel_entries_when_pending_capacity_is_exhausted() {
    let (request_tx, request_rx) = mpsc::sync_channel(AUDIO_PREP_QUEUE_CAPACITY);
    let (engine_tx, mut engine_rx) = event_queue();
    let (result_tx, result_rx) = mpsc::channel();
    let state = test_state();
    let mut pending = VecDeque::with_capacity(AUDIO_PREP_QUEUE_CAPACITY);
    for sequence in 1..=31 {
        pending.push_back(AudioControlRequest::InstrumentSlot {
            sequence,
            instrument_slot: 0,
            generation: sequence,
            config: serde_json::json!({ "type": "synth" }),
        });
    }
    request_tx
        .send(AudioControlRequest::InstrumentSlot {
            sequence: 32,
            instrument_slot: 0,
            generation: 32,
            config: serde_json::json!({ "type": "synth" }),
        })
        .unwrap();
    request_tx
        .send(AudioControlRequest::FxBusSlot {
            sequence: 33,
            bus_index: 0,
            slot_index: 0,
            generation: 33,
            fx_type: "delay".into(),
            params: Default::default(),
        })
        .unwrap();
    request_tx
        .send(AudioControlRequest::FullConfig {
            sequence: 34,
            revision: 0,
            generation: 34,
            request_id: None,
            config: audio_config(70),
        })
        .unwrap();
    request_tx
        .send(AudioControlRequest::SamplePreview(PreviewRequest {
            sequence: 35,
            token: 1,
            instrument_slot: 0,
            path: "samples/Drum/kick/Kick2.wav".into(),
            velocity: 96,
        }))
        .unwrap();
    handle_full_config_request(
        AudioControlRequest::FullConfig {
            sequence: 0,
            revision: 0,
            generation: 0,
            request_id: None,
            config: audio_config(60),
        },
        &request_rx,
        &mut pending,
        &engine_tx,
        &result_tx,
        &state,
    );
    assert_eq!(pending.len(), AUDIO_PREP_QUEUE_CAPACITY);
    assert!(matches!(
        request_rx.try_recv(),
        Ok(AudioControlRequest::FxBusSlot { sequence: 33, .. })
    ));
    assert!(matches!(
        request_rx.try_recv(),
        Ok(AudioControlRequest::FullConfig { sequence: 34, .. })
    ));
    assert!(matches!(
        request_rx.try_recv(),
        Ok(AudioControlRequest::SamplePreview(PreviewRequest {
            sequence: 35,
            ..
        }))
    ));
    assert!(matches!(
        next_event(&mut engine_rx),
        EngineEvent::SetPreparedAudioConfig { generation: 0, .. }
    ));
    assert!(matches!(
        recv_result(&result_rx),
        RuntimeStoreResult::OperationSucceeded {
            operation: RuntimeOperation::AudioCommand,
            ..
        }
    ));
}

fn next_event(rx: &mut rodio_engine_source::EngineEventReceiver) -> EngineEvent {
    let deadline = std::time::Instant::now() + Duration::from_secs(1);
    loop {
        match rx.try_recv() {
            Ok(event) => return event,
            Err(_) => {
                assert!(std::time::Instant::now() < deadline);
                std::thread::yield_now();
            }
        }
    }
}
