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
