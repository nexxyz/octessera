use super::*;
use realtime_engine::synth::{SourceWorkerHealth, SourceWorkerTimingProbe};
use std::sync::Arc;
use std::time::Duration;

#[test]
fn persistent_engine_source_deadline_path_retains_terminal_totals_after_join() {
    let (_tx, rx) = event_queue();
    let probe = Arc::new(SourceWorkerTimingProbe::new(None));
    let (mut source, shutdown_owner) =
        EngineSource::with_persistent_workers_for_benchmark_with_timing_probe(
            rx,
            48_000,
            128,
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
        assert_eq!(source.next(), Some(0.0));
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
    let source = include_str!("source_factory.rs");
    let normal_end = source
        .find("#[cfg(feature = \"source-worker-benchmark-timing\")]")
        .expect("benchmark timing constructor boundary");
    let normal = &source[..normal_end];
    for forbidden in [
        "SourceWorkerTimingProbe",
        "timing_probe",
        "orange_cpu_sampler",
        "Instant::now",
    ] {
        assert!(
            !normal.contains(forbidden),
            "normal path contains {forbidden}"
        );
    }
}
