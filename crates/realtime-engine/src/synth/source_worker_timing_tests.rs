use super::source_worker_timing::SourceWorkerTimingProbe;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

static CPU: AtomicU32 = AtomicU32::new(2);

fn test_cpu() -> u32 {
    CPU.load(Ordering::Relaxed)
}

#[test]
fn timing_probe_records_cpu_endpoint_change_and_freezes_coordinator() {
    let probe = SourceWorkerTimingProbe::new(Some(test_cpu));
    probe.begin_sequence(7, std::time::Duration::from_nanos(100));
    probe.record_dispatch(7, 0b11);
    CPU.store(2, Ordering::Relaxed);
    let start = probe.worker_start();
    CPU.store(3, Ordering::Relaxed);
    probe.record_worker(0, 7, start, 17, None);
    probe.record_completion(7, 0, std::time::Duration::from_nanos(50));
    probe.record_reduction(7, std::time::Duration::from_nanos(80));
    probe.freeze(0b01, 0b01, Some(std::time::Duration::from_nanos(120)), true);
    probe.record_completion(7, 1, std::time::Duration::from_nanos(60));
    probe.record_reduction(7, std::time::Duration::from_nanos(1));
    probe.record_engine_block_total(7, std::time::Duration::from_nanos(90));
    probe.record_callback_total(std::time::Duration::from_nanos(100));
    probe.record_engine_block_total(7, std::time::Duration::from_nanos(1));
    probe.record_callback_total(std::time::Duration::from_nanos(1));
    probe.begin_sequence(8, std::time::Duration::from_nanos(200));

    let snapshot = probe.snapshot();
    assert_eq!(snapshot.coordinator.sequence, Some(7));
    assert_eq!(snapshot.coordinator.first_parity, Some(0));
    assert_eq!(snapshot.coordinator.completed_mask, Some(1));
    assert_eq!(
        snapshot.coordinator.dispatch_to_deadline_elapsed_ns,
        Some(120)
    );
    assert_eq!(snapshot.coordinator.reduction_ns, Some(80));
    assert_eq!(snapshot.coordinator.engine_block_total_ns, Some(90));
    assert_eq!(snapshot.coordinator.callback_total_ns, Some(100));
    assert!(snapshot.coordinator.failed);
    assert!(snapshot.coordinator.frozen);
    assert!(snapshot.cpu_endpoint_changed);
    assert!(snapshot.workers[0].finished);
    assert!(!snapshot.workers[1].finished);
}

#[test]
fn timing_probe_leaves_unexecuted_fields_nullable() {
    let probe = SourceWorkerTimingProbe::new(None);
    let snapshot = probe.snapshot();

    assert_eq!(snapshot.coordinator.sequence, None);
    assert_eq!(snapshot.coordinator.reduction_ns, None);
    assert_eq!(snapshot.workers[0].render_ns, None);
    assert_eq!(snapshot.workers[1].cpu_end, None);
}

#[test]
fn timing_probe_uses_supplied_render_duration() {
    let probe = SourceWorkerTimingProbe::new(None);
    probe.begin_sequence(10, Duration::from_nanos(100));
    let start = probe.worker_start();
    probe.record_worker(0, 10, start, 777, None);

    assert_eq!(probe.snapshot().workers[0].render_ns, Some(777));
}

#[test]
fn timing_probe_tracks_source_and_bus_wave_timing() {
    let probe = SourceWorkerTimingProbe::new(None);
    probe.begin_sequence(12, Duration::from_nanos(100));
    probe.record_dispatch(12, 0b11);

    let first_start = probe.worker_start();
    probe.record_worker(0, 12, first_start, 777, None);
    probe.record_completion(12, 0, Duration::from_nanos(20));
    let second_start = probe.worker_start();
    probe.record_worker(1, 12, second_start, 555, None);
    probe.record_completion(12, 1, Duration::from_nanos(25));

    let source_snapshot = probe.snapshot();
    assert_eq!(source_snapshot.coordinator.first_parity, Some(0));
    assert_eq!(source_snapshot.coordinator.dispatch_to_first_ns, Some(20));
    assert_eq!(source_snapshot.coordinator.dispatch_to_both_ns, Some(25));

    probe.record_bus_dispatch(12);
    let dispatched_snapshot = probe.snapshot();
    assert_eq!(dispatched_snapshot.coordinator.completed_mask, Some(0));
    assert_eq!(dispatched_snapshot.coordinator.first_parity, None);
    assert_eq!(dispatched_snapshot.coordinator.dispatch_to_first_ns, None);
    assert_eq!(dispatched_snapshot.coordinator.dispatch_to_both_ns, None);

    probe.record_bus_worker(0, 12, 333, None);
    probe.record_bus_worker(1, 12, 222, None);
    probe.record_bus_completion(12, 1, Duration::from_nanos(50));
    probe.record_bus_completion(12, 0, Duration::from_nanos(60));

    let bus_snapshot = probe.snapshot();
    assert_eq!(bus_snapshot.workers[0].render_ns, Some(1_110));
    assert_eq!(bus_snapshot.workers[1].render_ns, Some(777));
    assert_eq!(bus_snapshot.coordinator.completed_mask, Some(0b11));
    assert_eq!(bus_snapshot.coordinator.first_parity, Some(1));
    assert_eq!(bus_snapshot.coordinator.dispatch_to_first_ns, Some(50));
    assert_eq!(bus_snapshot.coordinator.dispatch_to_both_ns, Some(60));
}

#[test]
fn timing_probe_records_reverse_completion_order_and_phase_totals() {
    let probe = SourceWorkerTimingProbe::new(None);
    probe.begin_sequence(11, Duration::from_nanos(100));
    probe.record_dispatch(11, 0b11);
    probe.record_completion(11, 1, Duration::from_nanos(40));
    probe.record_completion(11, 0, Duration::from_nanos(50));
    probe.record_reduction(11, Duration::from_nanos(10));
    probe.record_coordinator_remainder(11, Duration::from_nanos(20));
    probe.record_engine_block_total(11, Duration::from_nanos(30));
    probe.record_output_sequence(11);
    probe.record_callback_total(Duration::from_nanos(40));
    probe.freeze(0b11, 0b11, None, false);

    let snapshot = probe.snapshot();
    assert_eq!(snapshot.coordinator.first_parity, Some(1));
    assert_eq!(snapshot.coordinator.dispatch_to_first_ns, Some(40));
    assert_eq!(snapshot.coordinator.dispatch_to_both_ns, Some(50));
    assert_eq!(snapshot.coordinator.reduction_ns, Some(10));
    assert_eq!(snapshot.coordinator.coordinator_remainder_ns, Some(20));
    assert_eq!(snapshot.coordinator.engine_block_total_ns, Some(30));
    assert_eq!(snapshot.coordinator.callback_total_ns, Some(40));
    assert!(
        snapshot.coordinator.reduction_ns.unwrap()
            < snapshot.coordinator.coordinator_remainder_ns.unwrap()
    );
    assert!(
        snapshot.coordinator.coordinator_remainder_ns.unwrap()
            < snapshot.coordinator.engine_block_total_ns.unwrap()
    );
    assert!(
        snapshot.coordinator.engine_block_total_ns.unwrap()
            <= snapshot.coordinator.callback_total_ns.unwrap()
    );
}

#[test]
fn timing_probe_accepts_recovery_completion_without_restarting_sequence() {
    let probe = SourceWorkerTimingProbe::new(None);
    probe.begin_sequence(13, Duration::from_nanos(100));
    probe.record_dispatch(13, 0b11);
    probe.record_completion(13, 0, Duration::from_nanos(40));
    probe.freeze(0b01, 0b01, None, true);
    assert!(!probe.begin_sequence(14, Duration::from_nanos(200)));
    assert!(!probe.begin_sequence(15, Duration::from_nanos(200)));
    assert!(!probe.begin_sequence(16, Duration::from_nanos(200)));
    probe.record_recovery_completion(13, 1, Duration::from_nanos(70));
    probe.record_recovery_completion(13, 1, Duration::from_nanos(1));

    let snapshot = probe.snapshot();
    assert_eq!(snapshot.coordinator.sequence, Some(13));
    assert_eq!(snapshot.coordinator.completed_mask, Some(0b11));
    assert_eq!(snapshot.coordinator.dispatch_to_first_ns, Some(40));
    assert_eq!(snapshot.coordinator.dispatch_to_both_ns, Some(70));
    assert!(snapshot.coordinator.frozen);
}

#[test]
fn timing_probe_operations_do_not_allocate() {
    let probe = SourceWorkerTimingProbe::new(Some(test_cpu));
    let (_, allocations, deallocations) =
        super::test_allocator::count_allocations_and_deallocations(|| {
            probe.begin_sequence(9, std::time::Duration::from_nanos(100));
            probe.record_dispatch(9, 0b11);
            let start = probe.worker_start();
            probe.record_worker(0, 9, start, 17, None);
            probe.record_completion(9, 0, std::time::Duration::from_nanos(40));
            probe.record_reduction(9, std::time::Duration::from_nanos(10));
            probe.record_callback_total(std::time::Duration::from_nanos(50));
            probe.freeze(0b11, 0b01, None, false);
            let _ = probe.snapshot();
        });

    assert_eq!(allocations, 0);
    assert_eq!(deallocations, 0);
}

#[test]
fn timing_probe_keeps_completed_output_while_next_sequence_is_in_flight() {
    let probe = SourceWorkerTimingProbe::new(None);
    complete_sequence_without_callback(&probe, 40);
    probe.record_output_sequence(40);
    probe.begin_sequence(41, Duration::from_nanos(200));
    let start = probe.worker_start();
    probe.record_worker(0, 41, start, 777, None);
    probe.record_callback_total(Duration::from_nanos(500));

    let snapshot = probe.snapshot();
    assert_eq!(snapshot.coordinator.sequence, Some(40));
    assert_eq!(snapshot.coordinator.callback_total_ns, Some(500));
    assert!(snapshot
        .workers
        .iter()
        .all(|worker| { worker.sequence == Some(40) && worker.finished }));
}

#[test]
fn timing_probe_prefers_failed_current_sequence_over_completed_output() {
    let probe = SourceWorkerTimingProbe::new(None);
    complete_sequence(&probe, 50);
    probe.record_output_sequence(50);
    probe.begin_sequence(51, Duration::from_nanos(200));
    probe.record_dispatch(51, 0b11);
    probe.record_completion(51, 0, Duration::from_nanos(20));
    probe.freeze(0b10, 0b01, None, true);
    assert!(!probe.begin_sequence(52, Duration::from_nanos(300)));
    assert!(!probe.begin_sequence(53, Duration::from_nanos(300)));
    probe.record_worker(0, 53, probe.worker_start(), 1, None);
    probe.record_output_sequence(50);
    probe.record_callback_total_for_sequence(50, Duration::from_nanos(1));

    let snapshot = probe.snapshot();
    assert_eq!(snapshot.coordinator.sequence, Some(51));
    assert_eq!(snapshot.coordinator.completed_mask, Some(0b01));
    assert!(snapshot.coordinator.failed);
    assert!(snapshot.coordinator.frozen);
}

#[test]
fn timing_probe_publishes_healthy_completion_only_after_callback_total() {
    let incomplete_probe = SourceWorkerTimingProbe::new(None);
    complete_sequence_without_callback(&incomplete_probe, 80);
    incomplete_probe.freeze_latest_completed();
    assert_eq!(incomplete_probe.snapshot().coordinator.sequence, None);

    let probe = SourceWorkerTimingProbe::new(None);
    complete_sequence_without_callback(&probe, 80);
    probe.record_callback_total_for_sequence(80, Duration::from_nanos(60));
    probe.freeze_latest_completed();

    let snapshot = probe.snapshot();
    assert_eq!(snapshot.coordinator.sequence, Some(80));
    assert_eq!(snapshot.coordinator.callback_total_ns, Some(60));
}

#[test]
fn timing_probe_ignores_healthy_partial_sequence_on_shutdown() {
    let probe = SourceWorkerTimingProbe::new(None);
    probe.begin_sequence(60, Duration::from_nanos(200));
    probe.record_dispatch(60, 0b11);
    let start = probe.worker_start();
    probe.record_worker(0, 60, start, 10, None);
    probe.freeze_latest_completed();

    let snapshot = probe.snapshot();
    assert_eq!(snapshot.coordinator.sequence, None);
    assert!(snapshot.coordinator.frozen);
    assert!(!snapshot.coordinator.failed);
}

#[test]
fn timing_probe_rejects_late_worker_data_after_slot_reuse() {
    let probe = SourceWorkerTimingProbe::new(None);
    probe.begin_sequence(70, Duration::from_nanos(100));
    probe.begin_sequence(72, Duration::from_nanos(200));
    let start = probe.worker_start();
    probe.record_worker(0, 70, start, 10, None);
    probe.record_completion(70, 0, Duration::from_nanos(20));

    let snapshot = probe.snapshot();
    assert_eq!(snapshot.coordinator.sequence, Some(72));
    assert!(snapshot.workers.iter().all(|worker| !worker.finished));
}

fn complete_sequence(probe: &SourceWorkerTimingProbe, sequence: u64) {
    complete_sequence_without_callback(probe, sequence);
    probe.record_callback_total_for_sequence(sequence, Duration::from_nanos(60));
}

fn complete_sequence_without_callback(probe: &SourceWorkerTimingProbe, sequence: u64) {
    probe.begin_sequence(sequence, Duration::from_nanos(100));
    probe.record_dispatch(sequence, 0b11);
    for parity in 0..2 {
        let start = probe.worker_start();
        probe.record_worker(parity, sequence, start, 10, None);
        probe.record_completion(sequence, parity, Duration::from_nanos(20));
    }
    probe.record_reduction(sequence, Duration::from_nanos(30));
    probe.record_coordinator_remainder(sequence, Duration::from_nanos(40));
    probe.record_engine_block_total(sequence, Duration::from_nanos(50));
}
