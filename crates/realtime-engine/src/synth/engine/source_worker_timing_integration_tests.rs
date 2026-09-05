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
fn timing_probe_freeze_does_not_block_ordinary_recovery_or_fresh_render() {
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
    let failed_sequence = probe
        .snapshot()
        .coordinator
        .sequence
        .expect("failed timing sequence");

    lifecycle.set_pause_for_parity_for_test(0, false);
    runtime.set_deadline_for_test(Duration::from_secs(1));
    let recovery_deadline = std::time::Instant::now() + Duration::from_secs(1);
    while !runtime.collect_for_test(&mut engine) {
        assert!(std::time::Instant::now() < recovery_deadline);
        thread::yield_now();
    }
    assert_eq!(
        runtime.health_snapshot().status,
        SourceWorkerHealth::Healthy
    );
    assert_ne!(
        runtime.health_snapshot().status,
        SourceWorkerHealth::DispatchFailed
    );

    let recovered_timing = probe.snapshot();
    assert_eq!(recovered_timing.coordinator.sequence, Some(failed_sequence));
    assert!(recovered_timing.coordinator.failed);
    assert_eq!(recovered_timing.coordinator.completed_mask, Some(0b11));
    assert!(recovered_timing
        .workers
        .iter()
        .all(|worker| worker.finished));

    let mut left = Vec::with_capacity(128);
    let mut right = Vec::with_capacity(128);
    let mut out = Vec::with_capacity(256);
    assert_eq!(
        engine.render_interleaved_block_with_source_runtime(
            &mut runtime,
            128,
            &mut left,
            &mut right,
            &mut out,
        ),
        SourceWorkerRenderDisposition::Fresh
    );
    assert_eq!(
        runtime.health_snapshot().status,
        SourceWorkerHealth::Healthy
    );
    assert_eq!(probe.snapshot().coordinator.sequence, Some(failed_sequence));

    assert_eq!(lifecycle.shutdown(runtime.retire()).joined_workers, 2);
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

#[test]
fn timing_probe_records_both_persistent_worker_waves() {
    let mut engine = dynamic_engine();
    engine.note_on(0, 36, 100, 5_000);
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_prewarmed(&mut engine).expect("persistent runtime");
    let probe = Arc::new(SourceWorkerTimingProbe::new(None));
    runtime.attach_timing_probe(Arc::clone(&probe));
    runtime.set_deadline_for_test(Duration::from_secs(1));

    let mut left = Vec::with_capacity(128);
    let mut right = Vec::with_capacity(128);
    let mut out = Vec::with_capacity(256);
    engine.render_interleaved_block_with_source_runtime(
        &mut runtime,
        128,
        &mut left,
        &mut right,
        &mut out,
    );

    let snapshot = probe.snapshot();
    assert!(snapshot.workers.iter().all(|worker| worker.finished));
    assert!(snapshot
        .workers
        .iter()
        .all(|worker| worker.render_ns.is_some_and(|duration| duration > 0)));
    assert_eq!(snapshot.coordinator.completed_mask, Some(0b11));
    assert!(snapshot.coordinator.dispatch_to_both_ns.is_some());
    assert!(!snapshot.coordinator.failed);

    assert_eq!(lifecycle.shutdown(runtime.retire()).joined_workers, 2);
}

#[cfg(feature = "routing-tree-benchmark")]
#[test]
fn routing_timing_evidence_uses_the_absolute_85_percent_deadline() {
    let mut engine = SynthEngine::new(48_000);
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_routing_tree_prewarmed(&mut engine, 128)
            .expect("routing-tree runtime");
    let probe = Arc::new(SourceWorkerTimingProbe::new(None));
    runtime.attach_timing_probe(Arc::clone(&probe));
    let mut left = Vec::with_capacity(128);
    let mut right = Vec::with_capacity(128);
    let mut out = Vec::with_capacity(256);

    assert_eq!(
        engine.render_interleaved_block_with_source_runtime(
            &mut runtime,
            128,
            &mut left,
            &mut right,
            &mut out,
        ),
        SourceWorkerRenderDisposition::Fresh
    );
    assert_eq!(
        engine.render_interleaved_block_with_source_runtime(
            &mut runtime,
            128,
            &mut left,
            &mut right,
            &mut out,
        ),
        SourceWorkerRenderDisposition::Fresh
    );
    runtime.record_engine_block_total(runtime.timing_block_start());
    probe.record_callback_total(Duration::ZERO);

    let snapshot = probe.snapshot();
    let budget_ns = Duration::from_secs_f64(128.0 / 48_000.0 * 0.85).as_nanos() as u64;
    let dispatch_to_deadline_start_ns = snapshot
        .coordinator
        .dispatch_to_deadline_start_ns
        .expect("routing dispatch-to-deadline start");
    let deadline_ns = snapshot.coordinator.deadline_ns.expect("routing deadline");
    let observed_budget_ns = dispatch_to_deadline_start_ns + deadline_ns;
    assert!(observed_budget_ns.abs_diff(budget_ns) <= 2);

    assert_eq!(lifecycle.shutdown(runtime.retire()).joined_workers, 2);
}
