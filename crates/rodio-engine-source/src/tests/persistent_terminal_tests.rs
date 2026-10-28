use super::*;
use realtime_engine::synth::SourceWorkerHoldControl;
use realtime_engine::synth::SourceWorkerRuntime;
use std::sync::{Arc, Barrier};
use std::time::Duration;

const RATE: u32 = 44_100;
const BLOCK_FRAMES: usize = 128;

impl EngineSource {
    fn with_persistent_workers_for_test(
        control_rx: EngineEventReceiver,
        sample_rate: u32,
        block_frames: usize,
        engine: Box<SynthEngine>,
        lifecycle: SourceWorkerLifecycle,
        runtime: SourceWorkerRuntime,
        panic_before_envelope: bool,
    ) -> (Self, EngineSourceWorkerShutdownOwner) {
        let (retired_tx, retired_rx) = bounded(RETIREMENT_QUEUE_CAPACITY);
        let (shutdown_tx, shutdown_owner) = crate::source_worker_reaper::spawn_persistent_reaper(
            lifecycle,
            retired_rx,
            panic_before_envelope,
        )
        .unwrap_or_else(|failure| panic!("persistent source reaper: {:?}", failure.error));
        let source = Self::with_engine(
            control_rx,
            sample_rate,
            block_frames.clamp(MIN_BLOCK_FRAMES, MAX_BLOCK_FRAMES),
            None,
            engine,
            EngineSourceWorkerState::routing_tree(runtime),
            SourceRetirementChannels {
                retired_tx,
                shutdown_tx,
            },
        );
        (source, shutdown_owner)
    }
}

fn block_bits(source: &mut EngineSource) -> Vec<u32> {
    (0..BLOCK_FRAMES * 2)
        .map(|_| source.next().unwrap().to_bits())
        .collect()
}

pub(super) fn gated_source(
    hold_workers: bool,
    panic_parity: Option<usize>,
    panic_before_envelope: bool,
) -> (
    EngineEventSender,
    EngineSource,
    EngineSourceWorkerShutdownOwner,
    SourceWorkerHoldControl,
) {
    let mut engine = Box::new(SynthEngine::new(RATE));
    let (lifecycle, mut runtime) = if hold_workers {
        SourceWorkerLifecycle::start_routing_tree_prewarmed_held_for_test(&mut engine, 128).unwrap()
    } else {
        SourceWorkerLifecycle::start_routing_tree_prewarmed(&mut engine, 128).unwrap()
    };
    if hold_workers {
        runtime.set_deadline_for_test(Duration::ZERO);
    } else {
        runtime.set_deadline_for_test(Duration::from_secs(1));
    }
    if let Some(parity) = panic_parity {
        lifecycle.set_panic_on_job_for_test(parity);
    }
    let hold_control = lifecycle.hold_control_for_test();
    let (tx, rx) = event_queue();
    let (source, shutdown) = EngineSource::with_persistent_workers_for_test(
        rx,
        RATE,
        BLOCK_FRAMES,
        engine,
        lifecycle,
        runtime,
        panic_before_envelope,
    );
    (tx, source, shutdown, hold_control)
}

fn queue_active_state(tx: &EngineEventSender, samples: Arc<[f32]>) {
    tx.send(EngineEvent::SetPreparedAudioConfig {
        generation: 0,
        config: super::persistent_profile_tests::mixed_config(samples),
    })
    .unwrap();
    tx.send(EngineEvent::SetVoiceStealingMode {
        generation: 0,
        mode: realtime_engine::synth::VoiceStealingMode::None,
    })
    .unwrap();
    tx.send(EngineEvent::NoteOn {
        instrument_slot: 1,
        note: 36,
        velocity: 100,
        duration_ms: 50_000,
    })
    .unwrap();
    for note in 0..=realtime_engine::synth::SYNTH_VOICE_LANE_CAPACITY {
        tx.send(EngineEvent::NoteOn {
            instrument_slot: 0,
            note: note as u8,
            velocity: 100,
            duration_ms: 50_000,
        })
        .unwrap();
    }
}

#[test]
fn reaper_panic_before_envelope_still_retires_workers_and_audio() {
    let samples: Arc<[f32]> = Arc::from(vec![0.8; 4_096]);
    let (tx, mut source, shutdown, _hold_control) = gated_source(false, None, true);
    queue_active_state(&tx, Arc::clone(&samples));
    let _ = block_bits(&mut source);
    let before_drop = Arc::strong_count(&samples);
    drop(source);
    let result = shutdown.shutdown();
    assert_eq!(result.joined_workers, 2);
    assert_eq!(result.retirement_error, None);
    assert!(Arc::strong_count(&samples) < before_drop);
}

#[test]
fn retirement_sender_disconnect_uses_runtime_drop_shutdown() {
    let samples: Arc<[f32]> = Arc::from(vec![0.8; 4_096]);
    let (tx, mut source, shutdown, _hold_control) = gated_source(false, None, false);
    queue_active_state(&tx, Arc::clone(&samples));
    let _ = block_bits(&mut source);
    let before_drop = Arc::strong_count(&samples);
    drop(source.worker_state.worker.take());
    drop(source);
    let result = shutdown.shutdown();
    assert_eq!(result.joined_workers, 2);
    assert_eq!(result.retirement_error, None);
    assert!(Arc::strong_count(&samples) < before_drop);
}

#[test]
fn same_thread_panic_unwind_is_nonblocking_in_both_declaration_orders() {
    for owner_declared_first in [false, true] {
        let result = std::thread::spawn(move || {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                if owner_declared_first {
                    let (_tx, _owner, _source) = super::persistent_tests::persistent_source(128);
                    panic!("test unwind");
                }
                let (_tx, _source, _owner) = super::persistent_tests::persistent_source(128);
                panic!("test unwind");
            }))
        })
        .join();
        assert!(result.is_ok());
    }
}

#[test]
fn concurrent_source_and_owner_drop_races_are_nonblocking() {
    for _ in 0..50 {
        let (_tx, source, owner) = super::persistent_tests::persistent_source(128);
        let barrier = Arc::new(Barrier::new(2));
        let source_barrier = Arc::clone(&barrier);
        let owner_barrier = Arc::clone(&barrier);
        let source_thread = std::thread::spawn(move || {
            source_barrier.wait();
            drop(source);
        });
        let owner_thread = std::thread::spawn(move || {
            owner_barrier.wait();
            drop(owner);
        });
        assert!(source_thread.join().is_ok());
        assert!(owner_thread.join().is_ok());
    }
}

#[test]
fn persistent_startup_has_two_workers_and_one_combined_reaper() {
    let (_tx, source, owner) = super::persistent_tests::persistent_source(128);
    let lifecycle_probe = owner.lifecycle_probe_for_test();
    for _ in 0..100_000 {
        if lifecycle_probe.starts_for_test() == 1 {
            break;
        }
        std::thread::yield_now();
    }
    assert_eq!(lifecycle_probe.starts_for_test(), 1);
    assert_eq!(lifecycle_probe.exits_for_test(), 0);
    let worker = source
        .worker_state
        .worker
        .as_ref()
        .expect("persistent worker");
    assert_eq!(worker.runtime.jobs_started_for_test(), [1, 1]);
    drop(source);
    assert_eq!(owner.shutdown().joined_workers, 2);
    assert_eq!(lifecycle_probe.starts_for_test(), 1);
    assert_eq!(lifecycle_probe.exits_for_test(), 1);
}
