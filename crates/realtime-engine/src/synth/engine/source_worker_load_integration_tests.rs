use super::source_worker_test_fixtures::dynamic_engine;
use super::*;
use std::thread;

fn measured_load(reverse: bool) -> SourceWorkerLoadSnapshot {
    let mut engine = dynamic_engine();
    engine.note_on(0, 36, 100, 5_000);
    engine.note_on(1, 60, 100, 5_000);
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_prewarmed_with_frames(&mut engine, 12_000)
            .expect("worker runtime");
    assert_eq!(runtime.load_snapshot().unwrap().utilization_ppm, None);
    lifecycle.set_reverse_completion_for_test(reverse);
    assert!(runtime.dispatch_only_for_test(&mut engine, 128));
    let mut measurements = [(0, 0); 2];
    for parity in 0..2 {
        for _ in 0..10_000 {
            if runtime.completion_ready_for_test(parity) {
                break;
            }
            thread::yield_now();
        }
        measurements[parity] = runtime
            .completion_measurement_for_test(parity)
            .expect("worker completion measurement");
        assert!(runtime.rewrite_completion_measurement_for_test(
            parity,
            1_000_000 * (parity as u64 + 1),
            [2, 3][parity],
        ));
    }
    assert_eq!(measurements.map(|(_, units)| units), [5, 0]);
    assert!(runtime.collect_wait_for_test(&mut engine));
    let status = engine.audio_load_status();
    assert_eq!(status.worker_utilization, Some(0.008));
    assert!(!status.high_cpu_steady);
    let snapshot = runtime.load_snapshot().expect("persistent worker load");
    assert!(snapshot.utilization_ppm.is_some());
    assert_eq!(snapshot.busy_ns_ewma, [1_000_000, 2_000_000]);
    assert_eq!(snapshot.ns_per_unit_ewma, [500_000, 666_666]);
    assert_eq!(snapshot.observed_active_cost_units, [2, 3]);
    assert_eq!(snapshot.has_useful_measurement, [true, true]);
    let retirement = runtime.retire();
    assert_eq!(lifecycle.shutdown(retirement).joined_workers, 2);
    snapshot
}

#[test]
fn paired_load_updates_are_parity_ordered_and_reverse_completion_is_identical() {
    assert_eq!(measured_load(false), measured_load(true));
}

#[test]
fn paired_completion_updates_worker_warning_state_from_measured_load() {
    let mut engine = dynamic_engine();
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_prewarmed_with_frames(&mut engine, 12_000)
            .expect("worker runtime");
    assert!(runtime.dispatch_only_for_test(&mut engine, 128));
    for parity in 0..2 {
        for _ in 0..10_000 {
            if runtime.completion_ready_for_test(parity) {
                break;
            }
            thread::yield_now();
        }
        assert!(runtime.rewrite_completion_measurement_for_test(parity, 220_000_000, 1,));
    }

    assert!(runtime.collect_wait_for_test(&mut engine));
    let status = engine.audio_load_status();
    assert_eq!(status.worker_utilization, Some(0.88));
    assert!(status.high_cpu_steady);
    assert!(!status.missed_quantum_flash);
    let retirement = runtime.retire();
    assert_eq!(lifecycle.shutdown(retirement).joined_workers, 2);
}

#[test]
fn invalid_completion_does_not_update_load() {
    let mut engine = dynamic_engine();
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_prewarmed_with_frames(&mut engine, 12_000)
            .expect("worker runtime");
    let before = runtime.load_snapshot().expect("persistent worker load");
    let before_status = engine.audio_load_status();
    assert!(runtime.dispatch_only_for_test(&mut engine, 128));
    for parity in 0..2 {
        for _ in 0..10_000 {
            if runtime.completion_ready_for_test(parity) {
                break;
            }
            thread::yield_now();
        }
    }
    assert!(runtime.rewrite_completion_measurement_for_test(
        0,
        1_000_000,
        SOURCE_WORKER_MAX_COST_UNITS + 1,
    ));
    assert!(!runtime.collect_wait_for_test(&mut engine));
    assert_eq!(runtime.load_snapshot(), Some(before));
    let after_status = engine.audio_load_status();
    assert_eq!(
        after_status.worker_utilization,
        before_status.worker_utilization
    );
    assert_eq!(after_status.high_cpu_steady, before_status.high_cpu_steady);
    let retirement = runtime.retire();
    assert_eq!(lifecycle.shutdown(retirement).joined_workers, 2);
}

#[test]
fn stale_completion_does_not_update_load() {
    let mut engine = dynamic_engine();
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_prewarmed_with_frames(&mut engine, 12_000)
            .expect("worker runtime");
    let before = runtime.load_snapshot().expect("persistent worker load");
    let before_status = engine.audio_load_status();
    assert!(runtime.dispatch_only_for_test(&mut engine, 128));
    for parity in 0..2 {
        for _ in 0..10_000 {
            if runtime.completion_ready_for_test(parity) {
                break;
            }
            thread::yield_now();
        }
    }
    assert!(runtime.rewrite_completion_sequence_for_test(0));
    assert!(!runtime.collect_wait_for_test(&mut engine));
    assert_eq!(runtime.load_snapshot(), Some(before));
    let after_status = engine.audio_load_status();
    assert_eq!(
        after_status.worker_utilization,
        before_status.worker_utilization
    );
    assert_eq!(after_status.high_cpu_steady, before_status.high_cpu_steady);
    let retirement = runtime.retire();
    assert_eq!(lifecycle.shutdown(retirement).joined_workers, 2);
}

#[test]
fn terminal_completion_does_not_update_load() {
    let mut engine = dynamic_engine();
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_prewarmed_with_frames(&mut engine, 12_000)
            .expect("worker runtime");
    let before = runtime.load_snapshot().expect("persistent worker load");
    let before_status = engine.audio_load_status();
    lifecycle.set_exit_on_job_for_test(0);
    assert!(runtime.dispatch_only_for_test(&mut engine, 128));
    assert!(!runtime.collect_wait_for_test(&mut engine));
    assert_eq!(runtime.load_snapshot(), Some(before));
    let after_status = engine.audio_load_status();
    assert_eq!(
        after_status.worker_utilization,
        before_status.worker_utilization
    );
    assert_eq!(after_status.high_cpu_steady, before_status.high_cpu_steady);
    let retirement = runtime.retire();
    assert_eq!(lifecycle.shutdown(retirement).joined_workers, 2);
}

#[test]
fn inline_runtime_has_no_production_load_state() {
    assert_eq!(SourceWorkerRuntime::inline().load_snapshot(), None);
}
