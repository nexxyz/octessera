use super::*;
use crossbeam_channel::bounded;
use realtime_engine::synth::{
    default_synth_config, install_source_worker_shutdown_probe_for_test, prepare_audio_config,
    InstrumentSlotConfig, InstrumentsConfig, SampleBankConfig, SampleBuffer, SampleSlotConfig,
    SourceWorkerStartHook, DEFAULT_PAN_POSITIONS, INSTRUMENT_SLOT_COUNT,
    MAX_CONTROL_EVENTS_PER_CALLBACK, MAX_SAMPLE_VOICES_PER_SLOT,
};
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

const RETIREMENT_FILL_COUNT: usize =
    RETIREMENT_QUEUE_CAPACITY + RETIREMENT_CONTROL_BACKLOG_CAPACITY;
const RETIREMENT_STORED_ITEM_COUNT: usize = RETIREMENT_FILL_COUNT + 1;
const TEST_SAMPLE_RATE: u32 = 44_100;

fn fail_schedule_parity_zero(parity: usize) -> Result<(), ()> {
    (parity != 0).then_some(()).ok_or(())
}

fn fail_schedule_parity_one(parity: usize) -> Result<(), ()> {
    (parity != 1).then_some(()).ok_or(())
}

fn shared_sample_engine() -> (SynthEngine, Arc<[f32]>) {
    let shared_samples: Arc<[f32]> = Arc::from(vec![0.25; 4_096]);
    let mut bank = SampleBankConfig::default();
    bank.slots[0] = SampleSlotConfig {
        buffer: Some(SampleBuffer {
            samples: Arc::clone(&shared_samples),
            channels: 1,
            sample_rate: TEST_SAMPLE_RATE,
        }),
    };
    let config = prepare_audio_config(
        InstrumentsConfig {
            instruments: (0..INSTRUMENT_SLOT_COUNT)
                .map(|_| InstrumentSlotConfig {
                    kind: "sampler".into(),
                    synth: default_synth_config(),
                    mixer: None,
                })
                .collect(),
            mixer: None,
            pan_positions: DEFAULT_PAN_POSITIONS,
            master_volume: 100.0,
        },
        Some((0..INSTRUMENT_SLOT_COUNT).map(|_| bank.clone()).collect()),
        None,
        TEST_SAMPLE_RATE,
    );
    let mut engine = SynthEngine::new(TEST_SAMPLE_RATE);
    let _ = engine.apply_prepared_audio_config(config);
    engine.note_on(0, 36, 100, 10_000);
    (engine, shared_samples)
}

fn fill_retirement_storage_for_reaper(tx: &EngineEventSender, source: &mut EngineSource) {
    prime_pending_render_retirement(source);
    for index in 0..RETIREMENT_FILL_COUNT {
        tx.send(EngineEvent::MomentaryFxStop {
            id: format!("reaper-fill-{index}"),
        })
        .unwrap();
    }
    let (allocation_count, deallocation_count) = allocations_and_deallocations(|| {
        assert_eq!(
            source.drain_control_events().control_events,
            MAX_CONTROL_EVENTS_PER_CALLBACK as u64
        );
        assert_eq!(
            source.drain_control_events().control_events,
            (RETIREMENT_FILL_COUNT - MAX_CONTROL_EVENTS_PER_CALLBACK) as u64
        );
        source.refill();
    });
    assert_eq!((allocation_count, deallocation_count), (0, 0));
    assert!(!source.retirement_disconnected);
    assert_eq!(source.retired_backlog_len(), RETIREMENT_BACKLOG_CAPACITY);
}

fn prime_pending_render_retirement(source: &mut EngineSource) {
    prime_engine_pending_render_retirement(&mut source.engine);
}

fn prime_engine_pending_render_retirement(engine: &mut SynthEngine) {
    let _ = engine.apply_prepared_audio_config(super::retirement_tests::full_sample_config(1.0));
    for slot in 0..INSTRUMENT_SLOT_COUNT {
        for _ in 0..MAX_SAMPLE_VOICES_PER_SLOT {
            engine.note_on(slot as u8, 36, 100, 10_000);
        }
    }
    engine.note_on(0, 36, 100, 10_000);
    assert!(!engine.pending_render_retired_is_empty());
}

fn held_inline_source() -> (
    EngineEventSender,
    EngineSource,
    std::sync::Arc<std::sync::atomic::AtomicBool>,
    JoinHandle<()>,
    Receiver<std::thread::ThreadId>,
) {
    let (tx, rx) = event_queue();
    let (retired_tx, shutdown_tx, hold_retired, reaper) =
        crate::source_worker_reaper::spawn_inline_reaper_for_test();
    let (drop_tx, drop_rx) = mpsc::channel();
    let mut source = EngineSource::with_engine(
        rx,
        TEST_SAMPLE_RATE,
        128,
        None,
        SynthEngine::new(TEST_SAMPLE_RATE),
        EngineSourceWorkerState::inline(),
        SourceRetirementChannels {
            retired_tx,
            shutdown_tx,
        },
    );
    source.set_retired_drop_probe(drop_tx);
    (tx, source, hold_retired, reaper, drop_rx)
}

fn held_persistent_source(
    terminal: bool,
    panic_before_envelope: bool,
) -> (
    EngineEventSender,
    EngineSource,
    EngineSourceWorkerShutdownOwner,
    std::sync::Arc<std::sync::atomic::AtomicBool>,
    Receiver<std::thread::ThreadId>,
    Option<realtime_engine::synth::SourceWorkerHoldControl>,
) {
    let mut engine = SynthEngine::new(TEST_SAMPLE_RATE);
    prime_engine_pending_render_retirement(&mut engine);
    let (lifecycle, mut runtime) = if terminal {
        SourceWorkerLifecycle::start_prewarmed_held_for_test(&mut engine).unwrap()
    } else {
        SourceWorkerLifecycle::start_prewarmed(&mut engine).unwrap()
    };
    runtime.set_timing_for_test(
        if terminal { 0 } else { usize::MAX },
        if terminal {
            Duration::ZERO
        } else {
            Duration::from_secs(1)
        },
    );
    let worker_hold = terminal.then(|| lifecycle.hold_control_for_test());
    let (retired_tx, retired_rx) = bounded(RETIREMENT_QUEUE_CAPACITY);
    let (shutdown_tx, shutdown_owner, hold_retired) =
        crate::source_worker_reaper::spawn_persistent_reaper_for_test(
            lifecycle,
            retired_rx,
            panic_before_envelope,
        )
        .unwrap_or_else(|failure| panic!("persistent reaper: {:?}", failure.error));
    let (tx, rx) = event_queue();
    let (drop_tx, drop_rx) = mpsc::channel();
    let mut source = EngineSource::with_engine(
        rx,
        TEST_SAMPLE_RATE,
        128,
        None,
        engine,
        EngineSourceWorkerState::persistent(runtime),
        SourceRetirementChannels {
            retired_tx,
            shutdown_tx,
        },
    );
    source.set_retired_drop_probe(drop_tx);
    (
        tx,
        source,
        shutdown_owner,
        hold_retired,
        drop_rx,
        worker_hold,
    )
}

fn assert_reaper_drops(
    drop_rx: Receiver<std::thread::ThreadId>,
    source_thread: std::thread::ThreadId,
    expected: usize,
) {
    let mut thread_ids = Vec::with_capacity(expected);
    for _ in 0..expected {
        thread_ids.push(
            drop_rx
                .recv_timeout(Duration::from_secs(1))
                .expect("retired payload drop notification"),
        );
    }
    assert!(thread_ids
        .iter()
        .all(|thread_id| *thread_id != source_thread));
    assert!(thread_ids.windows(2).all(|ids| ids[0] == ids[1]));
}

#[test]
fn persistent_factory_reaper_spawn_failure_cleans_workers_on_construction_thread() {
    let (result_tx, result_rx) = mpsc::sync_channel(1);
    let construction = std::thread::spawn(move || {
        let (probe_tx, probe_rx) = bounded(1);
        let _probe_guard = install_source_worker_shutdown_probe_for_test(probe_tx);
        let _spawn_failure = crate::source_worker_reaper::fail_next_reaper_spawn_for_test();
        let (engine, shared_samples) = shared_sample_engine();
        let sample_identity = Arc::as_ptr(&shared_samples) as *const f32 as usize;
        let before = Arc::strong_count(&shared_samples);
        let (_control_tx, control_rx) = event_queue();
        let factory = EngineSource::with_persistent_workers_with_engine(
            control_rx,
            TEST_SAMPLE_RATE,
            128,
            None,
            engine,
        );
        let after = Arc::strong_count(&shared_samples);
        let error = match factory {
            Ok((_source, _shutdown)) => panic!("reaper spawn failure was not injected"),
            Err(error) => error,
        };
        let shutdown = probe_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("factory cleanup report");
        result_tx
            .send((
                std::thread::current().id(),
                error,
                shutdown,
                sample_identity,
                before,
                after,
                _spawn_failure.attempts_for_test(),
            ))
            .unwrap();
    });
    let (
        construction_thread,
        error,
        (shutdown, shutdown_thread),
        sample_identity,
        before,
        after,
        reaper_spawn_attempts,
    ) = result_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("persistent factory reaper failure must return");
    construction.join().unwrap();

    assert_eq!(
        error,
        realtime_engine::synth::SourceWorkerSetupError::RetirementReaperUnavailable
    );
    assert_eq!(shutdown_thread, construction_thread);
    assert_eq!(shutdown.joined_workers, 2);
    assert_eq!(shutdown.retirement_error, None);
    assert_eq!(shutdown.destroyed_owner_count, 2);
    assert!(shutdown
        .destroyed_owner_identities
        .iter()
        .flatten()
        .any(|identity| identity.5 == Some(sample_identity)));
    assert!(after < before);
    assert_eq!(reaper_spawn_attempts, 1);
}

#[test]
fn persistent_factory_worker_schedule_failure_joins_workers_without_reaper_or_owner_leak() {
    for (expected_parity, start_hook) in [
        (0, fail_schedule_parity_zero as SourceWorkerStartHook),
        (1, fail_schedule_parity_one as SourceWorkerStartHook),
    ] {
        for _ in 0..100 {
            let (result_tx, result_rx) = mpsc::sync_channel(1);
            let construction = thread::spawn(move || {
                let construction_thread = thread::current().id();
                let (probe_tx, probe_rx) = bounded(1);
                let _probe_guard = install_source_worker_shutdown_probe_for_test(probe_tx);
                let reaper_spawn_failure =
                    crate::source_worker_reaper::fail_next_reaper_spawn_for_test();
                let (engine, shared_samples) = shared_sample_engine();
                assert_eq!(engine.profile_snapshot().active_sample_voices, 1);
                let sample_weak = Arc::downgrade(&shared_samples);
                let (_control_tx, control_rx) = event_queue();
                let factory = EngineSource::with_persistent_workers_with_engine_and_hook(
                    control_rx,
                    TEST_SAMPLE_RATE,
                    128,
                    None,
                    engine,
                    start_hook,
                );
                let error = match factory {
                    Ok((_source, _shutdown)) => {
                        panic!("worker schedule failure was not injected")
                    }
                    Err(error) => error,
                };
                let (shutdown, shutdown_thread) = probe_rx
                    .recv_timeout(Duration::from_secs(1))
                    .expect("worker schedule cleanup report");
                drop(shared_samples);
                let sample_destroyed_on_construction_thread = sample_weak.upgrade().is_none();
                result_tx
                    .send((
                        construction_thread,
                        error,
                        shutdown,
                        shutdown_thread,
                        sample_destroyed_on_construction_thread,
                        reaper_spawn_failure.attempts_for_test(),
                    ))
                    .unwrap();
            });
            let (
                construction_thread,
                error,
                shutdown,
                shutdown_thread,
                sample_destroyed_on_construction_thread,
                reaper_spawn_attempts,
            ) = result_rx
                .recv_timeout(Duration::from_secs(1))
                .expect("worker schedule failure must return");
            construction.join().unwrap();

            assert_eq!(
                error,
                realtime_engine::synth::SourceWorkerSetupError::WorkerSchedulingUnavailable {
                    parity: expected_parity
                }
            );
            assert_eq!(shutdown_thread, construction_thread);
            assert_eq!(shutdown.joined_workers, 2);
            assert_eq!(shutdown.destroyed_owner_count, 0);
            assert_eq!(shutdown.destroyed_owner_identities, [None, None]);
            assert!(sample_destroyed_on_construction_thread);
            assert_eq!(reaper_spawn_attempts, 0);
        }
    }
}

#[test]
fn inline_backlog_handoff_destroys_all_callback_payloads_on_reaper() {
    let (tx, mut source, hold_retired, reaper, drop_rx) = held_inline_source();
    fill_retirement_storage_for_reaper(&tx, &mut source);
    assert_eq!(
        source
            .retired_backlog_items()
            .filter(|item| item.drop_probe.is_some())
            .count(),
        RETIREMENT_BACKLOG_CAPACITY
    );
    let source_thread = std::thread::current().id();
    let (allocation_count, deallocation_count) = allocations_and_deallocations(|| {
        source.handoff_shutdown_for_test();
    });
    assert_eq!((allocation_count, deallocation_count), (0, 0));
    drop(source);
    hold_retired.store(false, std::sync::atomic::Ordering::Release);
    reaper.join().unwrap();
    assert_reaper_drops(drop_rx, source_thread, RETIREMENT_STORED_ITEM_COUNT);
}

#[test]
fn persistent_healthy_backlog_handoff_destroys_all_callback_payloads_on_reaper() {
    let (tx, mut source, shutdown, hold_retired, drop_rx, _worker_hold) =
        held_persistent_source(false, false);
    fill_retirement_storage_for_reaper(&tx, &mut source);
    let source_thread = std::thread::current().id();
    let (allocation_count, deallocation_count) = allocations_and_deallocations(|| {
        source.handoff_shutdown_for_test();
    });
    assert_eq!((allocation_count, deallocation_count), (0, 0));
    drop(source);
    hold_retired.store(false, std::sync::atomic::Ordering::Release);
    let result = shutdown.shutdown();
    assert_eq!(result.joined_workers, 2);
    assert_reaper_drops(drop_rx, source_thread, RETIREMENT_STORED_ITEM_COUNT);
}

#[test]
fn persistent_terminal_backlog_handoff_destroys_all_callback_payloads_on_reaper() {
    let (tx, mut source, shutdown, hold_retired, drop_rx, worker_hold) =
        held_persistent_source(true, false);
    fill_retirement_storage_for_reaper(&tx, &mut source);
    let _ = source.next();
    assert_eq!(
        source.source_worker_health(),
        SourceWorkerHealth::DeadlineMiss
    );
    let source_thread = std::thread::current().id();
    let (allocation_count, deallocation_count) = allocations_and_deallocations(|| {
        source.handoff_shutdown_for_test();
    });
    assert_eq!((allocation_count, deallocation_count), (0, 0));
    drop(source);
    worker_hold.expect("terminal worker hold").release();
    hold_retired.store(false, std::sync::atomic::Ordering::Release);
    let result = shutdown.shutdown();
    assert_eq!(result.joined_workers, 2);
    assert_reaper_drops(drop_rx, source_thread, RETIREMENT_STORED_ITEM_COUNT);
}

#[test]
fn persistent_reaper_panic_fallback_drains_handed_off_backlog() {
    let (tx, mut source, shutdown, hold_retired, drop_rx, worker_hold) =
        held_persistent_source(true, true);
    fill_retirement_storage_for_reaper(&tx, &mut source);
    let source_thread = std::thread::current().id();
    let (allocation_count, deallocation_count) = allocations_and_deallocations(|| {
        source.handoff_shutdown_for_test();
    });
    assert_eq!((allocation_count, deallocation_count), (0, 0));
    drop(source);
    worker_hold.expect("terminal worker hold").release();
    hold_retired.store(false, std::sync::atomic::Ordering::Release);
    let result = shutdown.shutdown();
    assert_eq!(result.joined_workers, 2);
    assert_reaper_drops(drop_rx, source_thread, RETIREMENT_STORED_ITEM_COUNT);
}
