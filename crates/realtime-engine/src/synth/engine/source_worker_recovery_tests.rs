use super::source_worker_test_fixtures::dynamic_engine;
use super::source_worker_two_wave_tests::full_bus_config;
use super::*;
use std::thread;
use std::time::{Duration, Instant};

fn render_block(
    engine: &mut SynthEngine,
    runtime: &mut SourceWorkerRuntime,
    frames: usize,
) -> Vec<f32> {
    let mut left = Vec::with_capacity(frames);
    let mut right = Vec::with_capacity(frames);
    let mut out = Vec::with_capacity(frames * 2);
    engine.render_interleaved_block_with_source_runtime(
        runtime, frames, &mut left, &mut right, &mut out,
    );
    out
}

fn wait_for_completion(runtime: &SourceWorkerRuntime, parity: usize) {
    for _ in 0..100_000 {
        if runtime.completion_ready_for_test(parity) {
            return;
        }
        thread::yield_now();
    }
    assert!(runtime.completion_ready_for_test(parity));
}

fn expire_second_wave_deadline(runtime: &mut SourceWorkerRuntime, deadline: &mut Instant) {
    runtime.set_pause_for_parity_for_test(0, true);
    *deadline = Instant::now();
}

#[test]
fn source_deadline_recovery_is_bounded_and_advances_once() {
    let mut engine = dynamic_engine();
    engine.note_on(1, 60, 100, 5_000);
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_prewarmed(&mut engine).expect("persistent runtime");
    let initial_owners = runtime.home_owner_identities_for_test();
    lifecycle.set_pause_for_test(true);
    runtime.set_deadline_for_test(Duration::ZERO);

    let missed = render_block(&mut engine, &mut runtime, 128);
    assert!(missed.iter().all(|sample| sample.to_bits() == 0));
    assert_eq!(
        runtime.health_snapshot().status,
        SourceWorkerHealth::DeadlineMiss
    );
    assert_eq!(engine.sample_clock, 0);
    assert_eq!(
        runtime.pending_recovery_state_for_test().0,
        Some(WorkerPhase::Sources)
    );

    lifecycle.set_pause_for_parity_for_test(1, false);
    wait_for_completion(&runtime, 1);
    assert!(!runtime.refresh_recovery(&mut engine));
    let (_, stamp, in_flight, completed) = runtime.pending_recovery_state_for_test();
    assert_eq!(stamp.expect("recovery stamp").frames, 128);
    assert_eq!(in_flight & 0b01, 0b01);
    assert_eq!(completed & 0b10, 0b10);
    assert_eq!(engine.sample_clock, 0);

    lifecycle.set_pause_for_parity_for_test(0, false);
    let mut recovered = false;
    for _ in 0..100_000 {
        if runtime.collect_for_test(&mut engine) {
            recovered = true;
            break;
        }
        thread::yield_now();
    }
    assert!(recovered);
    assert_eq!(
        runtime.health_snapshot().status,
        SourceWorkerHealth::Healthy
    );
    assert_eq!(runtime.health_snapshot().failed_mask, 0);
    assert_eq!(runtime.health_snapshot().deadline_misses, 1);
    assert_eq!(runtime.health_snapshot().deadline_recoveries, 1);
    assert_eq!(engine.sample_clock, 128);
    assert_eq!(runtime.home_owner_identities_for_test(), initial_owners);

    let jobs = runtime.jobs_started_for_test();
    runtime.set_deadline_for_test(Duration::from_secs(1));
    let _ = render_block(&mut engine, &mut runtime, 128);
    assert_eq!(runtime.jobs_started_for_test(), jobs.map(|jobs| jobs + 2));
    assert_eq!(engine.sample_clock, 256);
    assert_eq!(lifecycle.shutdown(runtime.retire()).joined_workers, 2);
}

#[test]
fn bus_deadline_recovery_preserves_carriers_and_silences_missed_block() {
    let mut engine = dynamic_engine();
    engine.set_instruments(full_bus_config());
    engine.note_on(0, 36, 100, 5_000);
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_prewarmed(&mut engine).expect("persistent runtime");
    let initial_scratch = runtime.home_bus_carrier_scratch_addresses_for_test();
    runtime.set_before_bus_dispatch_hook_for_test(expire_second_wave_deadline);
    runtime.set_deadline_for_test(Duration::from_secs(1));

    let missed = render_block(&mut engine, &mut runtime, 128);
    assert!(missed.iter().all(|sample| sample.to_bits() == 0));
    assert_eq!(
        runtime.health_snapshot().status,
        SourceWorkerHealth::DeadlineMiss
    );
    assert_eq!(engine.sample_clock, 0);
    assert_eq!(
        runtime.pending_recovery_state_for_test().0,
        Some(WorkerPhase::Buses)
    );
    let expected_residency = runtime
        .bus_dispatch_residency_for_test()
        .expect("bus residency");

    lifecycle.set_pause_for_parity_for_test(0, false);
    let mut recovered = false;
    for _ in 0..100_000 {
        if runtime.collect_for_test(&mut engine) {
            recovered = true;
            break;
        }
        thread::yield_now();
    }
    assert!(recovered);
    assert_eq!(
        runtime.health_snapshot().status,
        SourceWorkerHealth::Healthy
    );
    assert_eq!(runtime.health_snapshot().deadline_recoveries, 1);
    assert_eq!(engine.sample_clock, 128);
    let assignments = runtime.home_bus_carrier_assignments_for_test();
    for (logical_bus_id, parity) in expected_residency.into_iter().enumerate() {
        let parity = usize::from(parity);
        assert!(assignments[parity][logical_bus_id].is_some());
        assert!(assignments[1 - parity][logical_bus_id].is_none());
    }
    let mut initial_scratch = initial_scratch
        .into_iter()
        .flatten()
        .flatten()
        .collect::<Vec<_>>();
    let mut recovered_scratch = runtime
        .home_bus_carrier_scratch_addresses_for_test()
        .into_iter()
        .flatten()
        .flatten()
        .collect::<Vec<_>>();
    initial_scratch.sort_unstable();
    recovered_scratch.sort_unstable();
    assert_eq!(recovered_scratch, initial_scratch);

    let _ = render_block(&mut engine, &mut runtime, 128);
    assert_eq!(engine.sample_clock, 256);
    assert_eq!(lifecycle.shutdown(runtime.retire()).joined_workers, 2);
}

#[test]
fn invalid_late_completion_escalates_recovery_to_terminal_failure() {
    let mut engine = dynamic_engine();
    engine.note_on(1, 60, 100, 5_000);
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_prewarmed(&mut engine).expect("persistent runtime");
    lifecycle.set_pause_for_test(true);
    runtime.set_deadline_for_test(Duration::ZERO);
    let missed = render_block(&mut engine, &mut runtime, 128);
    assert!(missed.iter().all(|sample| sample.to_bits() == 0));

    lifecycle.set_pause_for_test(false);
    wait_for_completion(&runtime, 0);
    assert!(runtime.rewrite_completion_sequence_for_test(0));
    assert!(!runtime.refresh_recovery(&mut engine));
    let health = runtime.health_snapshot();
    assert_eq!(health.status, SourceWorkerHealth::CompletionFailed);
    assert_ne!(health.failed_mask & 0b01, 0);
    assert_eq!(health.deadline_recoveries, 0);
    assert!(lifecycle.fault_owner_identities_for_test()[0].is_some());

    assert_eq!(lifecycle.shutdown(runtime.retire()).joined_workers, 2);
}

#[test]
fn panicking_late_completion_escalates_recovery_to_worker_exit() {
    assert_late_worker_failure_is_terminal(true);
}

#[test]
fn exiting_late_completion_escalates_recovery_to_worker_exit() {
    assert_late_worker_failure_is_terminal(false);
}

fn assert_late_worker_failure_is_terminal(panic_on_job: bool) {
    let mut engine = dynamic_engine();
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_prewarmed_held_for_test(&mut engine)
            .expect("persistent runtime");
    runtime.set_deadline_for_test(Duration::ZERO);
    let missed = render_block(&mut engine, &mut runtime, 128);
    assert!(missed.iter().all(|sample| sample.to_bits() == 0));

    if panic_on_job {
        lifecycle.set_panic_on_job_for_test(0);
    } else {
        lifecycle.set_exit_on_job_for_test(0);
    }
    lifecycle.set_hold_before_receive_for_test(false);
    wait_for_completion(&runtime, 0);
    assert!(!runtime.refresh_recovery(&mut engine));
    let health = runtime.health_snapshot();
    assert_eq!(health.status, SourceWorkerHealth::WorkerExited);
    assert_ne!(health.failed_mask & 0b01, 0);
    assert_eq!(health.deadline_recoveries, 0);
    assert!(lifecycle.fault_owner_identities_for_test()[0].is_some());

    assert_eq!(lifecycle.shutdown(runtime.retire()).joined_workers, 2);
}

#[test]
fn disconnected_recovery_completion_is_terminal_without_fallback() {
    let mut engine = dynamic_engine();
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_prewarmed(&mut engine).expect("persistent runtime");
    lifecycle.set_pause_for_test(true);
    runtime.set_deadline_for_test(Duration::ZERO);
    let missed = render_block(&mut engine, &mut runtime, 128);
    assert!(missed.iter().all(|sample| sample.to_bits() == 0));

    runtime.disconnect_recovery_completion_for_test(0);
    assert!(!runtime.refresh_recovery(&mut engine));
    let health = runtime.health_snapshot();
    assert_eq!(health.status, SourceWorkerHealth::CompletionFailed);
    assert_ne!(health.failed_mask & 0b01, 0);
    assert_eq!(health.deadline_recoveries, 0);

    lifecycle.set_pause_for_test(false);
    assert_eq!(lifecycle.shutdown(runtime.retire()).joined_workers, 2);
}

#[test]
fn recovery_keeps_controls_gated_until_both_owners_are_home() {
    let mut engine = dynamic_engine();
    engine.note_on(1, 60, 100, 5_000);
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_prewarmed(&mut engine).expect("persistent runtime");
    lifecycle.set_pause_for_test(true);
    runtime.set_deadline_for_test(Duration::ZERO);
    let _ = render_block(&mut engine, &mut runtime, 128);
    let revision = engine.synth_render_revisions[1];
    assert!(runtime
        .with_controls_ready(&mut engine, |engine| {
            engine.set_synth_param(1, "synth.filter.cutoffHz", 900.0);
        })
        .is_none());
    assert_eq!(engine.synth_render_revisions[1], revision);

    lifecycle.set_pause_for_test(false);
    let mut recovered = false;
    for _ in 0..100_000 {
        if runtime.collect_for_test(&mut engine) {
            recovered = true;
            break;
        }
        thread::yield_now();
    }
    assert!(recovered);
    assert!(runtime
        .with_controls_ready(&mut engine, |engine| {
            engine.set_synth_param(1, "synth.filter.cutoffHz", 900.0);
        })
        .is_some());
    assert_eq!(lifecycle.shutdown(runtime.retire()).joined_workers, 2);
}

#[test]
fn recovery_refresh_has_no_heap_activity() {
    let mut engine = dynamic_engine();
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_prewarmed(&mut engine).expect("persistent runtime");
    lifecycle.set_pause_for_test(true);
    runtime.set_deadline_for_test(Duration::ZERO);
    let _ = render_block(&mut engine, &mut runtime, 128);
    lifecycle.set_pause_for_test(false);
    wait_for_completion(&runtime, 0);
    wait_for_completion(&runtime, 1);

    let (_, allocations, deallocations) =
        crate::synth::test_allocator::count_allocations_and_deallocations(|| {
            let _ = runtime.refresh_recovery(&mut engine);
        });
    assert_eq!((allocations, deallocations), (0, 0));
    assert_eq!(
        runtime.health_snapshot().status,
        SourceWorkerHealth::Healthy
    );
    assert_eq!(lifecycle.shutdown(runtime.retire()).joined_workers, 2);
}
