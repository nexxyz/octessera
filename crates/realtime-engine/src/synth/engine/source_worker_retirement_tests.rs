use super::*;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use super::source_worker_lifecycle::SourceWorkerOwnerIdentity;
use super::source_worker_test_fixtures::{dynamic_engine, sample_engine_with_shared_buffer};

type OwnerIdentity = SourceWorkerOwnerIdentity;

static PANIC_HOOK_LOCK: Mutex<()> = Mutex::new(());

struct RetirementExpectation {
    health: SourceWorkerHealth,
    home: [Option<OwnerIdentity>; 2],
    fault: [Option<OwnerIdentity>; 2],
    destroyed: [Option<OwnerIdentity>; 2],
}

fn count_without_panic_hook<F, R>(operation: F) -> (R, usize, usize)
where
    F: FnOnce() -> R,
{
    let _lock = PANIC_HOOK_LOCK.lock().expect("panic hook lock");
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let result = crate::synth::test_allocator::count_allocations_and_deallocations(operation);
    std::panic::set_hook(previous);
    result
}

fn shared_sample_engine() -> (SynthEngine, Arc<[f32]>) {
    let samples: Arc<[f32]> = Arc::from(vec![0.25; 16_384]);
    let mut engine = sample_engine_with_shared_buffer(Arc::clone(&samples));
    engine.note_on(0, 36, 100, 5_000);
    engine.note_on(0, 37, 100, 5_000);
    (engine, samples)
}

fn wait_for_jobs(lifecycle: &SourceWorkerLifecycle) {
    for _ in 0..100_000 {
        if lifecycle.jobs_started_for_test() == [1, 1] {
            return;
        }
        thread::yield_now();
    }
    assert_eq!(lifecycle.jobs_started_for_test(), [1, 1]);
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

fn assert_immediate_retirement(
    engine: &mut SynthEngine,
    lifecycle: SourceWorkerLifecycle,
    mut runtime: SourceWorkerRuntime,
    expected: RetirementExpectation,
    release_workers: impl FnOnce(&SourceWorkerLifecycle),
) {
    let (_, allocations, deallocations) =
        crate::synth::test_allocator::count_allocations_and_deallocations(|| {
            assert!(!runtime.collect_wait_for_test(engine));
        });
    assert_eq!((allocations, deallocations), (0, 0));
    assert_eq!(runtime.health_snapshot().status, expected.health);
    assert_eq!(
        [
            runtime.home_owner_identity_for_test(0),
            runtime.home_owner_identity_for_test(1),
        ],
        expected.home
    );
    assert_eq!(lifecycle.fault_owner_identities_for_test(), expected.fault);

    let join_handles_before = lifecycle.join_handles_present_for_test();
    let (retirement, allocations, deallocations) =
        crate::synth::test_allocator::count_allocations_and_deallocations(|| runtime.retire());
    assert_eq!((allocations, deallocations), (0, 0));
    assert_eq!(
        lifecycle.join_handles_present_for_test(),
        join_handles_before
    );

    release_workers(&lifecycle);
    let shutdown = lifecycle.shutdown(retirement);
    assert_eq!(shutdown.joined_workers, 2);
    assert_eq!(shutdown.destroyed_owner_count, 2);
    assert_eq!(shutdown.destroyed_owner_identities, expected.destroyed);
}

#[test]
fn deadline_after_one_completion_destroys_both_owners_off_callback() {
    let (mut engine, shared_samples) = shared_sample_engine();
    let before_shutdown = Arc::strong_count(&shared_samples);
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_prewarmed_held_for_test(&mut engine)
            .expect("prewarmed worker runtime");
    let initial = runtime.home_owner_identities_for_test();
    lifecycle.set_pause_for_parity_for_test(0, true);
    lifecycle.set_hold_before_receive_for_test(false);
    assert!(runtime.dispatch_only_for_test(&mut engine, 128));
    wait_for_jobs(&lifecycle);
    wait_for_completion(&runtime, 1);
    runtime.set_deadline_for_test(Duration::ZERO);
    assert_immediate_retirement(
        &mut engine,
        lifecycle,
        runtime,
        RetirementExpectation {
            health: SourceWorkerHealth::DeadlineMiss,
            home: [None, Some(initial[1])],
            fault: [None, None],
            destroyed: [Some(initial[0]), Some(initial[1])],
        },
        |lifecycle| lifecycle.set_pause_for_parity_for_test(0, false),
    );
    assert!(Arc::strong_count(&shared_samples) < before_shutdown);
}

#[test]
fn disconnected_completion_preserves_owner_until_lifecycle_join() {
    let (mut engine, shared_samples) = shared_sample_engine();
    let before_shutdown = Arc::strong_count(&shared_samples);
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_prewarmed_disconnected_held_for_test(&mut engine, 0)
            .expect("prewarmed worker runtime");
    let initial = runtime.home_owner_identities_for_test();
    lifecycle.set_pause_for_parity_for_test(0, true);
    lifecycle.set_hold_before_receive_for_test(false);
    assert!(runtime.dispatch_only_for_test(&mut engine, 128));
    wait_for_jobs(&lifecycle);
    wait_for_completion(&runtime, 1);
    runtime.set_deadline_for_test(Duration::ZERO);
    assert_immediate_retirement(
        &mut engine,
        lifecycle,
        runtime,
        RetirementExpectation {
            health: SourceWorkerHealth::CompletionFailed,
            home: [None, Some(initial[1])],
            fault: [None, None],
            destroyed: [Some(initial[0]), Some(initial[1])],
        },
        |lifecycle| lifecycle.set_pause_for_parity_for_test(0, false),
    );
    assert!(Arc::strong_count(&shared_samples) < before_shutdown);
}

#[test]
fn stale_completion_destroys_both_original_owners_without_reduction() {
    let (mut engine, shared_samples) = shared_sample_engine();
    let before_shutdown = Arc::strong_count(&shared_samples);
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_prewarmed(&mut engine).expect("prewarmed worker runtime");
    let initial = runtime.home_owner_identities_for_test();
    runtime.set_deadline_for_test(Duration::from_secs(1));
    assert!(runtime.dispatch_only_for_test(&mut engine, 128));
    for _ in 0..100_000 {
        if runtime.rewrite_completion_sequence_for_test(0) {
            break;
        }
        thread::yield_now();
    }
    assert!(!runtime.collect_for_test(&mut engine));
    assert_eq!(
        runtime.health_snapshot().status,
        SourceWorkerHealth::CompletionFailed
    );
    assert_eq!(runtime.home_owner_identity_for_test(1), Some(initial[1]));
    assert_eq!(
        lifecycle.fault_owner_identities_for_test()[0],
        Some(initial[0])
    );
    assert_immediate_retirement(
        &mut engine,
        lifecycle,
        runtime,
        RetirementExpectation {
            health: SourceWorkerHealth::CompletionFailed,
            home: [None, Some(initial[1])],
            fault: [Some(initial[0]), None],
            destroyed: [Some(initial[0]), Some(initial[1])],
        },
        |_| {},
    );
    assert!(Arc::strong_count(&shared_samples) < before_shutdown);
}

#[test]
fn panic_after_one_recovery_destroys_both_owners_off_callback() {
    let (mut engine, shared_samples) = shared_sample_engine();
    let before_shutdown = Arc::strong_count(&shared_samples);
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_prewarmed_held_for_test(&mut engine)
            .expect("prewarmed worker runtime");
    let initial = runtime.home_owner_identities_for_test();
    lifecycle.set_panic_on_job_for_test(0);
    lifecycle.set_pause_for_parity_for_test(1, true);
    lifecycle.set_hold_before_receive_for_test(false);
    assert!(runtime.dispatch_only_for_test(&mut engine, 128));
    wait_for_jobs(&lifecycle);
    wait_for_completion(&runtime, 0);
    runtime.set_deadline_for_test(Duration::ZERO);
    assert_immediate_retirement(
        &mut engine,
        lifecycle,
        runtime,
        RetirementExpectation {
            health: SourceWorkerHealth::WorkerExited,
            home: [None, None],
            fault: [Some(initial[0]), None],
            destroyed: [Some(initial[0]), Some(initial[1])],
        },
        |lifecycle| lifecycle.set_pause_for_parity_for_test(1, false),
    );
    assert!(Arc::strong_count(&shared_samples) < before_shutdown);
}

#[test]
fn worker_exit_after_both_completions_destroys_both_owners_off_callback() {
    let (mut engine, shared_samples) = shared_sample_engine();
    let before_shutdown = Arc::strong_count(&shared_samples);
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_prewarmed(&mut engine).expect("prewarmed worker runtime");
    let initial = runtime.home_owner_identities_for_test();
    lifecycle.set_exit_on_job_for_test(0);
    lifecycle.set_exit_on_job_for_test(1);
    runtime.set_deadline_for_test(Duration::from_secs(1));
    assert!(runtime.dispatch_only_for_test(&mut engine, 128));
    assert_immediate_retirement(
        &mut engine,
        lifecycle,
        runtime,
        RetirementExpectation {
            health: SourceWorkerHealth::WorkerExited,
            home: [None, None],
            fault: [Some(initial[0]), Some(initial[1])],
            destroyed: [Some(initial[0]), Some(initial[1])],
        },
        |_| {},
    );
    assert!(Arc::strong_count(&shared_samples) < before_shutdown);
}

#[test]
fn panicking_control_closure_preserves_both_owners_and_fails_closed() {
    let (mut engine, shared_samples) = shared_sample_engine();
    let before_shutdown = Arc::strong_count(&shared_samples);
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_prewarmed(&mut engine).expect("prewarmed worker runtime");
    let initial = runtime.home_owner_identities_for_test();
    let panic_payload = Box::new("control closure panic");
    let (panic_result, _, _) = count_without_panic_hook(|| {
        catch_unwind(AssertUnwindSafe(|| {
            runtime.with_controls_ready(&mut engine, |engine| {
                assert!(!source_worker_transfer::source_partitions_vacant(engine));
                for parity in 0..2 {
                    assert!(engine.synth_voice_pool.partition_is_present(parity));
                    assert!(engine.sample_voice_pool.partition_is_present(parity));
                }
                std::panic::resume_unwind(panic_payload);
            });
        }))
    });
    assert!(panic_result.is_err());
    assert!(source_worker_transfer::source_partitions_vacant(&engine));
    assert_eq!(
        [
            runtime.home_owner_identity_for_test(0),
            runtime.home_owner_identity_for_test(1),
        ],
        [None, None]
    );
    assert_eq!(
        lifecycle.fault_owner_identities_for_test(),
        [Some(initial[0]), Some(initial[1])]
    );
    let health = runtime.health_snapshot();
    assert_eq!(health.status, SourceWorkerHealth::CompletionFailed);
    assert_eq!(health.failed_mask, 0b11);
    assert_eq!(health.completion_failures, 1);

    let jobs = lifecycle.jobs_started_for_test();
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
    assert!(out.iter().all(|sample| sample.to_bits() == 0));
    assert_eq!(lifecycle.jobs_started_for_test(), jobs);
    assert!(runtime.with_controls_ready(&mut engine, |_| ()).is_none());
    assert_eq!(lifecycle.jobs_started_for_test(), jobs);

    let (retirement, allocations, deallocations) =
        crate::synth::test_allocator::count_allocations_and_deallocations(|| runtime.retire());
    assert_eq!((allocations, deallocations), (0, 0));
    let shutdown = lifecycle.shutdown(retirement);
    assert_eq!(shutdown.joined_workers, 2);
    assert_eq!(shutdown.destroyed_owner_count, 2);
    assert_eq!(
        shutdown.destroyed_owner_identities,
        [Some(initial[0]), Some(initial[1])]
    );
    assert!(Arc::strong_count(&shared_samples) < before_shutdown);
}

#[test]
fn disconnected_work_channel_returns_exact_owner_before_terminal_retirement() {
    let (mut engine, shared_samples) = shared_sample_engine();
    let before_shutdown = Arc::strong_count(&shared_samples);
    let (mut lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_prewarmed_held_for_test(&mut engine)
            .expect("prewarmed worker runtime");
    let initial = runtime.home_owner_identities_for_test();
    lifecycle.disconnect_work_for_test(0);
    runtime.disconnect_work_for_test(0);
    runtime.set_deadline_for_test(Duration::ZERO);
    assert!(!runtime.dispatch_only_for_test(&mut engine, 128));
    assert_eq!(runtime.home_owner_identity_for_test(0), Some(initial[0]));
    assert_eq!(
        runtime.health_snapshot().status,
        SourceWorkerHealth::DispatchFailed
    );
    let retirement = runtime.retire();
    lifecycle.set_hold_before_receive_for_test(false);
    let shutdown = lifecycle.shutdown(retirement);
    assert_eq!(shutdown.joined_workers, 2);
    assert_eq!(shutdown.destroyed_owner_count, 2);
    assert_eq!(
        shutdown.destroyed_owner_identities,
        [Some(initial[0]), Some(initial[1])]
    );
    assert!(Arc::strong_count(&shared_samples) < before_shutdown);
}

#[test]
fn asymmetric_full_work_dispatch_retires_immediately_without_joining_callback() {
    let mut engine = dynamic_engine();
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_prewarmed_held_for_test(&mut engine)
            .expect("prewarmed worker runtime");
    assert!(lifecycle.fill_work_channel_for_test(0));
    runtime.set_deadline_for_test(Duration::ZERO);
    assert!(!runtime.dispatch_only_for_test(&mut engine, 128));
    assert_eq!(
        runtime.health_snapshot().status,
        SourceWorkerHealth::DispatchFailed
    );

    let (retirement, allocations, deallocations) =
        crate::synth::test_allocator::count_allocations_and_deallocations(|| runtime.retire());
    assert_eq!((allocations, deallocations), (0, 0));
    lifecycle.set_hold_before_receive_for_test(false);
    assert_eq!(lifecycle.shutdown(retirement).joined_workers, 2);
}
