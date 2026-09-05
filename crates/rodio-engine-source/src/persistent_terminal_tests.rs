use super::*;
use realtime_engine::synth::SourceWorkerHoldControl;
use realtime_engine::synth::{SourceWorkerHealth, SourceWorkerHealthSnapshot, SourceWorkerRuntime};
use std::sync::{mpsc, Arc, Barrier};
use std::time::{Duration, Instant};

const RATE: u32 = 44_100;
const BLOCK_FRAMES: usize = 128;

fn expected_active_synth_voices_after_first_callback() -> usize {
    realtime_engine::synth::SYNTH_VOICE_LANE_CAPACITY
        .min(realtime_engine::synth::MAX_CONTROL_EVENTS_PER_CALLBACK - 3)
}

fn expected_voice_admission_drops_after_first_callback() -> u64 {
    u64::from(
        expected_active_synth_voices_after_first_callback()
            == realtime_engine::synth::SYNTH_VOICE_LANE_CAPACITY,
    )
}

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
            EngineSourceWorkerState::persistent(runtime),
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

fn assert_silence(samples: &[u32]) {
    assert!(samples.iter().all(|sample| *sample == 0));
}

fn gated_source(
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
        SourceWorkerLifecycle::start_prewarmed_held_for_test(&mut engine).unwrap()
    } else {
        SourceWorkerLifecycle::start_prewarmed(&mut engine).unwrap()
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

fn worker_health(source: &EngineSource) -> SourceWorkerHealthSnapshot {
    source
        .worker_state
        .worker
        .as_ref()
        .expect("persistent worker")
        .runtime
        .health_snapshot()
}

fn worker_jobs(source: &EngineSource) -> [u64; 2] {
    source
        .worker_state
        .worker
        .as_ref()
        .expect("persistent worker")
        .runtime
        .jobs_started_for_test()
}

fn render_attempts(source: &EngineSource) -> u64 {
    source
        .worker_state
        .worker
        .as_ref()
        .expect("persistent worker")
        .runtime
        .render_attempts_for_test()
}

fn queue_active_state(tx: &EngineEventSender, samples: Arc<[f32]>) {
    tx.send(EngineEvent::SetPreparedAudioConfig(
        super::persistent_profile_tests::mixed_config(samples),
    ))
    .unwrap();
    tx.send(EngineEvent::SetVoiceStealingMode(
        realtime_engine::synth::VoiceStealingMode::None,
    ))
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

fn queue_unconsumed_controls(tx: &EngineEventSender) -> mpsc::Receiver<u128> {
    let (report_tx, report_rx) = mpsc::sync_channel(1);
    tx.send(EngineEvent::SetSynthParam {
        instrument_slot: 0,
        path: "synth.amp.gainPct".into(),
        value: 12.0,
    })
    .unwrap();
    tx.send(EngineEvent::ProbeMark {
        sent_at: Instant::now(),
        report_tx,
    })
    .unwrap();
    report_rx
}

fn wait_for_worker_jobs(source: &EngineSource, expected: [u64; 2]) {
    for _ in 0..100_000 {
        if worker_jobs(source) == expected {
            return;
        }
        std::thread::yield_now();
    }
    assert_eq!(worker_jobs(source), expected);
}

fn wait_for_worker_completions(source: &EngineSource) {
    for _ in 0..100_000 {
        if source
            .worker_state
            .worker
            .as_ref()
            .expect("persistent worker")
            .runtime
            .completion_states_for_test()
            == [true, true]
        {
            return;
        }
        std::thread::yield_now();
    }
    assert_eq!(
        source
            .worker_state
            .worker
            .as_ref()
            .expect("persistent worker")
            .runtime
            .completion_states_for_test(),
        [true, true]
    );
}

#[test]
fn persistent_deadline_keeps_cache_and_controls_unconsumed() {
    let samples: Arc<[f32]> = Arc::from(vec![0.8; 4_096]);
    let (tx, mut source, shutdown, hold_control) = gated_source(true, None, false);
    queue_active_state(&tx, Arc::clone(&samples));
    assert_silence(&block_bits(&mut source));
    let cached = source.profile_snapshot();
    assert_eq!(
        cached.active_synth_voices,
        expected_active_synth_voices_after_first_callback()
    );
    assert_eq!(cached.active_sample_voices, 1);
    assert_eq!(
        cached.cumulative_voice_admission_drops,
        expected_voice_admission_drops_after_first_callback()
    );
    assert_eq!(
        source.source_worker_health(),
        SourceWorkerHealth::DeadlineMiss
    );
    assert_eq!(worker_health(&source).failed_mask, 0b11);
    assert_eq!(worker_health(&source).deadline_misses, 1);
    assert_eq!(worker_jobs(&source), [0, 0]);
    assert_eq!(render_attempts(&source), 1);

    let report_rx = queue_unconsumed_controls(&tx);
    for _ in 0..2 {
        assert_silence(&block_bits(&mut source));
    }
    assert!(report_rx.try_recv().is_err());
    assert_eq!(source.profile_snapshot(), cached);
    assert_eq!(
        source.source_worker_health(),
        SourceWorkerHealth::DeadlineMiss
    );
    assert_eq!(worker_health(&source).deadline_misses, 1);
    assert_eq!(worker_jobs(&source), [0, 0]);
    assert_eq!(render_attempts(&source), 1);

    let (allocations, deallocations) = allocations_and_deallocations(|| {
        source.retire_workers_for_test();
    });
    assert_eq!((allocations, deallocations), (0, 0));
    drop(source);
    hold_control.release();
    assert_eq!(shutdown.shutdown().joined_workers, 2);
    assert!(Arc::strong_count(&samples) < 2);
}

#[test]
fn persistent_worker_panic_is_terminal_through_iterator() {
    let samples: Arc<[f32]> = Arc::from(vec![0.8; 4_096]);
    let (tx, mut source, shutdown, _hold_control) = gated_source(false, Some(0), false);
    queue_active_state(&tx, Arc::clone(&samples));
    assert_silence(&block_bits(&mut source));
    let cached = source.profile_snapshot();
    assert_eq!(
        cached.active_synth_voices,
        expected_active_synth_voices_after_first_callback()
    );
    assert_eq!(cached.active_sample_voices, 1);
    assert_eq!(
        source.source_worker_health(),
        SourceWorkerHealth::WorkerExited
    );
    assert_eq!(worker_health(&source).worker_exits, 1);
    assert_ne!(worker_health(&source).failed_mask & 0b01, 0);
    let jobs = worker_jobs(&source);
    assert_eq!(render_attempts(&source), 1);

    let report_rx = queue_unconsumed_controls(&tx);
    for _ in 0..2 {
        assert_silence(&block_bits(&mut source));
    }
    assert!(report_rx.try_recv().is_err());
    assert_eq!(source.profile_snapshot(), cached);
    assert_eq!(
        source.source_worker_health(),
        SourceWorkerHealth::WorkerExited
    );
    assert_eq!(worker_health(&source).worker_exits, 1);
    assert_eq!(worker_jobs(&source), jobs);
    assert_eq!(render_attempts(&source), 3);

    let (allocations, deallocations) = allocations_and_deallocations(|| {
        source.retire_workers_for_test();
    });
    assert_eq!((allocations, deallocations), (0, 0));
    drop(source);
    assert_eq!(shutdown.shutdown().joined_workers, 2);
    assert!(Arc::strong_count(&samples) < 2);
}

#[test]
fn recovered_owner_refreshes_post_render_profile_and_allows_controls() {
    let samples: Arc<[f32]> = Arc::from(vec![0.8, -0.4, 0.2, -0.1]);
    let (tx, mut source, shutdown, hold_control) = gated_source(true, None, false);
    tx.send(EngineEvent::SetPreparedAudioConfig(
        super::persistent_profile_tests::mixed_config(Arc::clone(&samples)),
    ))
    .unwrap();
    tx.send(EngineEvent::NoteOn {
        instrument_slot: 1,
        note: 36,
        velocity: 100,
        duration_ms: 50_000,
    })
    .unwrap();
    assert_silence(&block_bits(&mut source));
    let before_late_render = source.profile_snapshot();
    assert_eq!(before_late_render.active_sample_voices, 1);
    assert_eq!(
        source.source_worker_health(),
        SourceWorkerHealth::DeadlineMiss
    );
    assert_eq!(worker_jobs(&source), [0, 0]);

    hold_control.release();
    wait_for_worker_jobs(&source, [1, 1]);
    wait_for_worker_completions(&source);
    source
        .worker_state
        .worker
        .as_mut()
        .expect("persistent worker")
        .runtime
        .set_deadline_for_test(Duration::from_secs(1));
    let report_rx = queue_unconsumed_controls(&tx);
    assert_silence(&block_bits(&mut source));
    let after_late_render = source.profile_snapshot();
    assert_eq!(after_late_render.active_sample_voices, 0);
    assert_ne!(after_late_render, before_late_render);
    assert!(report_rx.recv_timeout(Duration::from_secs(1)).is_ok());
    assert_eq!(source.source_worker_health(), SourceWorkerHealth::Healthy);
    assert_eq!(worker_health(&source).deadline_recoveries, 1);
    assert_eq!(worker_jobs(&source), [3, 3]);
    assert_eq!(render_attempts(&source), 2);

    let (allocations, deallocations) = allocations_and_deallocations(|| {
        source.retire_workers_for_test();
    });
    assert_eq!((allocations, deallocations), (0, 0));
    drop(source);
    assert_eq!(shutdown.shutdown().joined_workers, 2);
    assert!(Arc::strong_count(&samples) < 2);
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
    assert_eq!(worker.runtime.jobs_started_for_test(), [0, 0]);
    drop(source);
    assert_eq!(owner.shutdown().joined_workers, 2);
    assert_eq!(lifecycle_probe.starts_for_test(), 1);
    assert_eq!(lifecycle_probe.exits_for_test(), 1);
}
