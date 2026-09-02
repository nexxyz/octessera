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
fn timing_probe_records_reverse_completion_order_and_phase_totals() {
    let probe = SourceWorkerTimingProbe::new(None);
    probe.begin_sequence(11, Duration::from_nanos(100));
    probe.record_dispatch(11, 0b11);
    probe.record_completion(11, 1, Duration::from_nanos(40));
    probe.record_completion(11, 0, Duration::from_nanos(50));
    probe.record_reduction(11, Duration::from_nanos(10));
    probe.record_coordinator_remainder(11, Duration::from_nanos(20));
    probe.record_engine_block_total(11, Duration::from_nanos(30));
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
