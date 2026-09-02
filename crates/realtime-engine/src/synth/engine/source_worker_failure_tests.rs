use super::*;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use super::source_worker_test_fixtures::{dynamic_engine, sample_engine_with_shared_buffer};

#[test]
fn healthy_idle_retirement_keeps_bundles_until_lifecycle_shutdown() {
    let shared_samples: Arc<[f32]> = Arc::from(vec![0.25; 16_384]);
    let mut engine = sample_engine_with_shared_buffer(Arc::clone(&shared_samples));
    engine.note_on(0, 36, 100, 5_000);
    let before_retire = Arc::strong_count(&shared_samples);

    let (lifecycle, runtime) = SourceWorkerLifecycle::start_prewarmed_held_for_test(&mut engine)
        .expect("prewarmed worker runtime");
    let owner_identities = runtime.home_owner_identities_for_test();
    let sample_identities = runtime.home_sample_buffer_addresses_for_test();
    assert!(sample_identities
        .iter()
        .flatten()
        .any(|identity| *identity == Arc::as_ptr(&shared_samples) as *const f32 as usize));
    let (retirement, allocations, deallocations) =
        crate::synth::test_allocator::count_allocations_and_deallocations(|| runtime.retire());
    assert_eq!((allocations, deallocations), (0, 0));
    assert_eq!(Arc::strong_count(&shared_samples), before_retire);

    lifecycle.set_hold_before_receive_for_test(false);
    let shutdown = lifecycle.shutdown(retirement);
    assert_eq!(shutdown.joined_workers, 2);
    assert_eq!(shutdown.destroyed_owner_count, 2);
    assert_eq!(
        shutdown.destroyed_owner_identities,
        [Some(owner_identities[0]), Some(owner_identities[1])]
    );
    assert!(Arc::strong_count(&shared_samples) < before_retire);
}

#[test]
fn lifecycle_drop_after_runtime_retirement_destroys_home_owners() {
    let shared_samples: Arc<[f32]> = Arc::from(vec![0.25; 16_384]);
    let mut engine = sample_engine_with_shared_buffer(Arc::clone(&shared_samples));
    engine.note_on(0, 36, 100, 5_000);
    let before_drop = Arc::strong_count(&shared_samples);
    let (lifecycle, runtime) =
        SourceWorkerLifecycle::start_prewarmed(&mut engine).expect("prewarmed worker runtime");
    let retirement = runtime.retire();
    drop(retirement);
    drop(lifecycle);
    assert!(Arc::strong_count(&shared_samples) < before_drop);
}

#[test]
fn partial_worker_spawn_failure_joins_started_worker_before_return() {
    let spawn_failure = super::source_worker_lifecycle::worker::fail_worker_spawn_at_for_test(1);
    let mut engine = dynamic_engine();
    let error = match SourceWorkerLifecycle::start_prewarmed(&mut engine) {
        Ok(_) => panic!("injected worker spawn failure should return an error"),
        Err(error) => error,
    };
    assert_eq!(error, SourceWorkerSetupError::WorkerThreadUnavailable);
    assert_eq!(spawn_failure.active_workers_for_test(), 0);
}

#[test]
fn lifecycle_drop_waits_for_runtime_close_before_joining() {
    let mut engine = dynamic_engine();
    let (lifecycle, runtime) =
        SourceWorkerLifecycle::start_prewarmed(&mut engine).expect("prewarmed worker runtime");
    let lifecycle_join = thread::spawn(move || drop(lifecycle));
    thread::sleep(Duration::from_millis(5));
    assert!(!lifecycle_join.is_finished());
    drop(runtime);
    assert!(lifecycle_join.join().is_ok());
}

#[test]
fn deadline_runtime_drop_defers_owner_destruction_to_lifecycle_shutdown() {
    let shared_samples: Arc<[f32]> = Arc::from(vec![0.25; 16_384]);
    let mut engine = sample_engine_with_shared_buffer(Arc::clone(&shared_samples));
    engine.note_on(0, 36, 100, 5_000);
    let before_drop = Arc::strong_count(&shared_samples);

    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_prewarmed_held_for_test(&mut engine)
            .expect("prewarmed worker runtime");
    runtime.set_deadline_for_test(std::time::Duration::ZERO);
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
    assert_eq!(
        runtime.health_snapshot().status,
        SourceWorkerHealth::DeadlineMiss
    );
    assert!(!engine.voice_pools_home());
    assert_eq!(Arc::strong_count(&shared_samples), before_drop);

    let (_, allocations, deallocations) =
        crate::synth::test_allocator::count_allocations_and_deallocations(|| drop(runtime));
    assert_eq!((allocations, deallocations), (0, 0));
    assert_eq!(Arc::strong_count(&shared_samples), before_drop);

    lifecycle.set_hold_before_receive_for_test(false);
    let retirement = lifecycle.retirement_after_runtime_drop_for_test();
    assert_eq!(lifecycle.shutdown(retirement).joined_workers, 2);
    assert!(
        Arc::strong_count(&shared_samples) < before_drop,
        "before={before_drop}, after={}",
        Arc::strong_count(&shared_samples)
    );
}

#[test]
fn runtime_drop_does_not_join_workers() {
    let mut engine = dynamic_engine();
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_prewarmed_held_for_test(&mut engine)
            .expect("prewarmed worker runtime");
    assert!(runtime.dispatch_only_for_test(&mut engine, 128));

    let (_, allocations, deallocations) =
        crate::synth::test_allocator::count_allocations_and_deallocations(|| drop(runtime));
    assert_eq!((allocations, deallocations), (0, 0));

    lifecycle.set_hold_before_receive_for_test(false);
    for _ in 0..100_000 {
        if lifecycle.jobs_started_for_test() == [1, 1] {
            break;
        }
        thread::yield_now();
    }
    assert_eq!(lifecycle.jobs_started_for_test(), [1, 1]);
    let retirement = lifecycle.retirement_after_runtime_drop_for_test();
    assert_eq!(lifecycle.shutdown(retirement).joined_workers, 2);
}

#[test]
fn worker_panic_returns_bundle_and_latches_terminal_health() {
    let shared_samples: Arc<[f32]> = Arc::from(vec![0.25; 16_384]);
    let mut engine = sample_engine_with_shared_buffer(Arc::clone(&shared_samples));
    engine.note_on(0, 36, 100, 5_000);
    let before_render = Arc::strong_count(&shared_samples);

    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_prewarmed(&mut engine).expect("prewarmed worker runtime");
    lifecycle.set_panic_on_job_for_test(0);
    runtime.set_deadline_for_test(Duration::from_secs(1));
    let mut left = Vec::with_capacity(128);
    let mut right = Vec::with_capacity(128);
    let mut out = Vec::with_capacity(256);
    let (_, allocations, deallocations) =
        crate::synth::test_allocator::count_allocations_and_deallocations(|| {
            engine.render_interleaved_block_with_source_runtime(
                &mut runtime,
                128,
                &mut left,
                &mut right,
                &mut out,
            );
        });
    assert_eq!((allocations, deallocations), (0, 0));
    assert!(out.iter().all(|sample| sample.to_bits() == 0));
    let health = runtime.health_snapshot();
    assert_eq!(health.status, SourceWorkerHealth::WorkerExited);
    assert_ne!(health.failed_mask & 1, 0);
    assert_eq!(health.worker_exits, 1);
    assert!(runtime.partitions_home_for_test());
    assert_eq!(Arc::strong_count(&shared_samples), before_render);

    let jobs = lifecycle.jobs_started_for_test();
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

    let retirement = runtime.retire();
    assert_eq!(lifecycle.shutdown(retirement).joined_workers, 2);
}
