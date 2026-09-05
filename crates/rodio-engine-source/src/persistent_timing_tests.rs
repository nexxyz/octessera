use super::*;
use realtime_engine::synth::{
    default_synth_config, prepare_instruments_config, InstrumentSlotConfig, InstrumentsConfig,
    SourceWorkerHealth, SourceWorkerTimingProbe, DEFAULT_PAN_POSITIONS,
};
use std::sync::Arc;
use std::time::Duration;

#[cfg(feature = "routing-tree-benchmark")]
fn successful_start_hook(_: usize) -> Result<(), ()> {
    Ok(())
}

#[test]
fn persistent_engine_source_deadline_path_retains_terminal_totals_after_join() {
    let (_tx, rx) = event_queue();
    let probe = Arc::new(SourceWorkerTimingProbe::new(None));
    let (mut source, shutdown_owner) =
        EngineSource::with_persistent_workers_for_benchmark_with_timing_probe(
            rx,
            48_000,
            64,
            None,
            Arc::clone(&probe),
        )
        .unwrap();
    source
        .worker_state
        .worker
        .as_mut()
        .expect("persistent worker")
        .runtime
        .set_pause_for_parity_for_test(0, true);

    let (allocations, deallocations) = allocations_and_deallocations(|| {
        for _ in 0..256 {
            assert_eq!(source.next(), Some(0.0));
        }
        probe.record_callback_total(Duration::from_nanos(50));
    });
    assert_eq!(allocations, 0);
    assert_eq!(deallocations, 0);
    assert_eq!(
        source.source_worker_health(),
        SourceWorkerHealth::DeadlineMiss
    );

    let terminal = probe.snapshot();
    assert!(terminal.coordinator.frozen);
    assert!(terminal.coordinator.failed);
    assert!(terminal.coordinator.sequence.is_some());
    assert!(terminal.coordinator.engine_block_total_ns.is_some());
    assert!(terminal.coordinator.callback_total_ns.is_some());
    assert_eq!(terminal.coordinator.reduction_ns, None);
    assert_eq!(terminal.coordinator.coordinator_remainder_ns, None);

    source
        .worker_state
        .worker
        .as_mut()
        .expect("persistent worker")
        .runtime
        .set_pause_for_parity_for_test(0, false);
    drop(source);
    assert_eq!(shutdown_owner.shutdown().joined_workers, 2);

    let joined = probe.snapshot();
    assert!(joined.workers.iter().all(|worker| worker.finished));
    assert!(joined
        .workers
        .iter()
        .all(|worker| worker.sequence == joined.coordinator.sequence));
    assert!(joined.late_after_deadline_ns.is_some());
}

#[cfg(feature = "routing-tree-benchmark")]
#[test]
fn routing_tree_timing_records_output_pipeline_stages() {
    let (_tx, rx) = event_queue();
    let probe = Arc::new(SourceWorkerTimingProbe::new(None));
    let (mut source, shutdown_owner) =
        EngineSource::with_routing_tree_persistent_workers_for_benchmark_with_timing_probe_and_hook(
            rx,
            48_000,
            64,
            None,
            Arc::clone(&probe),
            successful_start_hook,
        )
        .unwrap();
    source
        .worker_state
        .worker
        .as_mut()
        .expect("routing-tree worker")
        .runtime
        .set_deadline_for_test(Duration::from_secs(1));

    for _ in 0..1024 {
        assert_eq!(source.next(), Some(0.0));
    }
    assert_eq!(source.source_worker_health(), SourceWorkerHealth::Healthy);
    let partial_sequence = probe
        .snapshot()
        .coordinator
        .sequence
        .expect("in-flight sequence");
    probe.record_callback_total(Duration::from_nanos(50));

    let snapshot = probe.snapshot();
    let output_sequence = snapshot.coordinator.sequence.expect("output sequence");
    assert_ne!(partial_sequence, output_sequence);
    assert!(snapshot
        .workers
        .iter()
        .all(|worker| worker.sequence == Some(output_sequence)));
    assert!(snapshot.coordinator.reduction_ns.is_some());
    assert!(snapshot.coordinator.coordinator_remainder_ns.is_some());
    assert!(snapshot.coordinator.engine_block_total_ns.is_some());
    assert!(!snapshot.coordinator.failed);

    drop(source);
    assert_eq!(shutdown_owner.shutdown().joined_workers, 2);
}

#[cfg(feature = "routing-tree-benchmark")]
#[test]
fn routing_tree_timing_failure_recovers_and_resumes_fresh_output() {
    let (control_tx, rx) = event_queue();
    control_tx
        .send(EngineEvent::SetPreparedInstruments(
            prepare_instruments_config(
                InstrumentsConfig {
                    instruments: vec![InstrumentSlotConfig {
                        kind: "synth".into(),
                        synth: default_synth_config(),
                        mixer: None,
                    }],
                    mixer: None,
                    pan_positions: DEFAULT_PAN_POSITIONS,
                    master_volume: 100.0,
                },
                48_000,
            ),
        ))
        .unwrap();
    let probe = Arc::new(SourceWorkerTimingProbe::new(None));
    let (mut source, shutdown_owner) =
        EngineSource::with_routing_tree_persistent_workers_for_benchmark_with_timing_probe_and_hook(
            rx,
            48_000,
            64,
            None,
            Arc::clone(&probe),
            successful_start_hook,
        )
        .unwrap();
    {
        let runtime = &mut source
            .worker_state
            .worker
            .as_mut()
            .expect("routing-tree worker")
            .runtime;
        runtime.set_pause_for_parity_for_test(0, true);
        runtime.set_deadline_for_test(Duration::ZERO);
    }
    for _ in 0..(64 * 2) {
        let _ = source.next();
    }
    {
        let runtime = &mut source
            .worker_state
            .worker
            .as_mut()
            .expect("routing-tree worker")
            .runtime;
        assert!(runtime.wait_until_paused_for_test(0, Duration::from_secs(1)));
    }

    for _ in 0..(64 * 2) {
        let _ = source.next();
    }
    assert_eq!(
        source.source_worker_health(),
        SourceWorkerHealth::DeadlineMiss
    );
    let failed_sequence = probe
        .snapshot()
        .coordinator
        .sequence
        .expect("failed routing sequence");
    let rendered_before_recovery = source.persistent_output_counters().rendered_quantums;

    control_tx
        .send(EngineEvent::NoteOn {
            instrument_slot: 0,
            note: 60,
            velocity: 127,
            duration_ms: 5_000,
        })
        .unwrap();
    {
        let runtime = &mut source
            .worker_state
            .worker
            .as_mut()
            .expect("routing-tree worker")
            .runtime;
        runtime.set_pause_for_parity_for_test(0, false);
        runtime.set_deadline_for_test(Duration::from_secs(1));
    }
    let mut samples = Vec::new();
    let recovery_deadline = std::time::Instant::now() + Duration::from_secs(1);
    while source.source_worker_health() != SourceWorkerHealth::Healthy {
        for _ in 0..(64 * 2) {
            samples.push(source.next().unwrap());
        }
        assert!(std::time::Instant::now() < recovery_deadline);
    }
    for _ in 0..(64 * 2) {
        samples.push(source.next().unwrap());
    }
    assert!(samples.iter().any(|sample| sample.abs() > 0.0001));
    assert!(source.persistent_output_counters().rendered_quantums > rendered_before_recovery);
    assert_eq!(source.source_worker_health(), SourceWorkerHealth::Healthy);
    assert_ne!(
        source.source_worker_health(),
        SourceWorkerHealth::DispatchFailed
    );

    let timing = probe.snapshot();
    assert_eq!(timing.coordinator.sequence, Some(failed_sequence));
    assert!(timing.coordinator.failed);
    assert_eq!(timing.coordinator.completed_mask, Some(0b11));
    assert!(timing.workers.iter().all(|worker| worker.finished));
    assert!(timing.late_after_deadline_ns.is_some());

    drop(source);
    assert_eq!(shutdown_owner.shutdown().joined_workers, 2);
}

#[test]
fn benchmark_without_timing_probe_preserves_audio_bits_health_and_joins() {
    let (disabled_tx, disabled_rx) = event_queue();
    let (mut disabled, disabled_shutdown) =
        EngineSource::with_persistent_workers_for_benchmark(disabled_rx, 44_100, 128, None)
            .unwrap();
    let (reference_tx, reference_rx) = event_queue();
    let (mut reference, reference_shutdown) =
        EngineSource::with_persistent_workers(reference_rx, 44_100, 128, None).unwrap();
    let note_on = EngineEvent::NoteOn {
        instrument_slot: 0,
        note: 60,
        velocity: 100,
        duration_ms: 1_000,
    };
    disabled_tx.send(note_on.clone()).unwrap();
    reference_tx.send(note_on).unwrap();
    let disabled_bits: Vec<_> = (0..256)
        .map(|_| disabled.next().unwrap().to_bits())
        .collect();
    let reference_bits: Vec<_> = (0..256)
        .map(|_| reference.next().unwrap().to_bits())
        .collect();

    assert_eq!(disabled_bits, reference_bits);
    assert!(disabled_bits.iter().any(|sample| *sample != 0));
    assert_eq!(disabled.source_worker_health(), SourceWorkerHealth::Healthy);
    assert_eq!(
        reference.source_worker_health(),
        SourceWorkerHealth::Healthy
    );
    drop(disabled);
    drop(reference);
    assert_eq!(disabled_shutdown.shutdown().joined_workers, 2);
    assert_eq!(reference_shutdown.shutdown().joined_workers, 2);
}

#[test]
fn normal_persistent_constructors_do_not_attach_timing_probe() {
    fn successful_start_hook(_: usize) -> Result<(), ()> {
        Ok(())
    }

    fn assert_without_timing_probe(
        constructor: impl FnOnce() -> Result<
            (EngineSource, EngineSourceWorkerShutdownOwner),
            SourceWorkerSetupError,
        >,
    ) {
        let (source, shutdown_owner) = constructor().unwrap();
        assert!(source
            .worker_state
            .worker
            .as_ref()
            .expect("persistent worker")
            .runtime
            .timing_block_start()
            .is_none());
        drop(source);
        assert_eq!(shutdown_owner.shutdown().joined_workers, 2);
    }

    assert_without_timing_probe(|| {
        let (_tx, rx) = event_queue();
        EngineSource::with_persistent_workers(rx, 44_100, 128, None)
    });
    assert_without_timing_probe(|| {
        let (_tx, rx) = event_queue();
        EngineSource::with_persistent_workers_with_hook(
            rx,
            44_100,
            128,
            None,
            successful_start_hook,
        )
    });
    assert_without_timing_probe(|| {
        let (_tx, rx) = event_queue();
        EngineSource::with_persistent_workers_for_benchmark(rx, 44_100, 128, None)
    });
    assert_without_timing_probe(|| {
        let (_tx, rx) = event_queue();
        EngineSource::with_persistent_workers_for_benchmark_with_hook(
            rx,
            44_100,
            128,
            None,
            successful_start_hook,
        )
    });

    #[cfg(feature = "routing-tree-benchmark")]
    {
        assert_without_timing_probe(|| {
            let (_tx, rx) = event_queue();
            EngineSource::with_routing_tree_persistent_workers_for_benchmark(rx, 44_100, 128, None)
        });
        assert_without_timing_probe(|| {
            let (_tx, rx) = event_queue();
            EngineSource::with_routing_tree_persistent_workers_for_benchmark_with_hook(
                rx,
                44_100,
                128,
                None,
                successful_start_hook,
            )
        });
    }
}
