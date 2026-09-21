use super::*;
use playback_runtime::{
    RuntimeErrorCode, RuntimeErrorDomain, RuntimeOperation, RuntimeStoreResult,
};
use rodio_engine_source::event_queue;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn test_state() -> DesktopAudioPrepState {
    DesktopAudioPrepState {
        config_revision: Arc::new(AtomicU64::new(0)),
        synth_slots: Arc::new(Mutex::new([true; INSTRUMENT_SLOT_COUNT])),
        sample_decode_cache: SampleDecodeCache::new(),
        sample_bank_signature: Arc::new(Mutex::new(String::new())),
        generations: Arc::new(Mutex::new(AudioGenerationState::default())),
    }
}

fn recv_result(rx: &Receiver<HostMessage>) -> RuntimeStoreResult {
    match rx.recv_timeout(Duration::from_secs(1)).unwrap() {
        HostMessage::RuntimeResult { result } => result,
        _ => panic!("unexpected prep message"),
    }
}

#[test]
fn superseded_preview_failure_is_silent_but_current_failure_reports() {
    let (request_tx, request_rx) = mpsc::sync_channel(AUDIO_PREP_QUEUE_CAPACITY);
    let (engine_tx, _engine_rx) = event_queue();
    let (result_tx, result_rx) = mpsc::channel();
    let state = test_state();
    let gate = Arc::new(PreviewPrepareTestGate {
        started: AtomicBool::new(false),
        release: AtomicBool::new(false),
    });
    let control = DesktopAudioControl {
        tx: request_tx,
        config_revision: state.config_revision.clone(),
        next_sequence: Arc::new(AtomicU64::new(0)),
        next_preview_token: Arc::new(AtomicU64::new(0)),
        preview_latest: Arc::new(Mutex::new(None)),
        generations: state.generations.clone(),
        preview_prepare_gate: Some(gate.clone()),
    };
    let worker_control = control.clone();
    std::thread::spawn(move || {
        audio_control_loop(request_rx, engine_tx, result_tx, state, worker_control)
    });
    assert_eq!(
        control.enqueue_sample_preview(0, "../missing.wav".into(), 96),
        AudioPrepEnqueueResult::Accepted
    );
    let deadline = std::time::Instant::now() + Duration::from_secs(1);
    while !gate.started.load(Ordering::Acquire) {
        assert!(std::time::Instant::now() < deadline);
        std::thread::yield_now();
    }
    assert_eq!(
        control.enqueue_sample_preview(0, "samples/Drum/kick/Kick2.wav".into(), 96),
        AudioPrepEnqueueResult::Accepted
    );
    gate.release.store(true, Ordering::Release);
    assert!(matches!(
        recv_result(&result_rx),
        RuntimeStoreResult::OperationSucceeded {
            operation: RuntimeOperation::SamplePreview,
            ..
        }
    ));
    assert_eq!(
        control.enqueue_sample_preview(0, "../missing.wav".into(), 96),
        AudioPrepEnqueueResult::Accepted
    );
    assert!(matches!(
        recv_result(&result_rx),
        RuntimeStoreResult::RuntimeFailure { error }
            if error.domain == RuntimeErrorDomain::Sample
                && error.operation == RuntimeOperation::SamplePreview
                && error.code == RuntimeErrorCode::NotFound
    ));
}
