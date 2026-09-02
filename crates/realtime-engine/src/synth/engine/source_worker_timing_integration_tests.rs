use super::super::source_worker_timing::SourceWorkerTimingProbe;
use super::source_worker_test_fixtures::dynamic_engine;
use super::*;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

#[test]
fn timing_probe_keeps_late_worker_data_through_deadline_retire_and_join() {
    let mut engine = dynamic_engine();
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_prewarmed(&mut engine).expect("worker runtime");
    let probe = Arc::new(SourceWorkerTimingProbe::new(None));
    runtime.attach_timing_probe(Arc::clone(&probe));
    lifecycle.set_pause_for_parity_for_test(0, true);
    assert!(runtime.dispatch_only_for_test(&mut engine, 128));
    for _ in 0..100_000 {
        if runtime.completion_ready_for_test(1) {
            break;
        }
        thread::yield_now();
    }
    assert!(runtime.completion_ready_for_test(1));
    runtime.set_deadline_for_test(Duration::ZERO);
    assert!(!runtime.collect_wait_for_test(&mut engine));
    thread::sleep(Duration::from_millis(2));
    lifecycle.set_pause_for_parity_for_test(0, false);
    let retirement = runtime.retire();
    assert_eq!(lifecycle.shutdown(retirement).joined_workers, 2);

    let snapshot = probe.snapshot();
    assert!(snapshot.coordinator.frozen);
    assert!(snapshot.coordinator.failed);
    assert_eq!(snapshot.coordinator.completed_mask, Some(2));
    assert!(snapshot.coordinator.dispatch_to_deadline_start_ns.is_some());
    assert!(snapshot
        .coordinator
        .dispatch_to_deadline_elapsed_ns
        .is_some());
    assert_eq!(snapshot.coordinator.dispatch_to_both_ns, None);
    assert!(snapshot.workers.iter().all(|worker| worker.finished));
    assert!(snapshot
        .workers
        .iter()
        .all(|worker| worker.sequence == snapshot.coordinator.sequence));
    assert!(snapshot.late_after_deadline_ns.is_some());
}

#[test]
fn normal_runtime_does_not_start_timing_without_a_probe() {
    let mut engine = dynamic_engine();
    let (lifecycle, runtime) =
        SourceWorkerLifecycle::start_prewarmed(&mut engine).expect("worker runtime");

    assert!(runtime.timing_block_start().is_none());

    let retirement = runtime.retire();
    assert_eq!(lifecycle.shutdown(retirement).joined_workers, 2);
}
