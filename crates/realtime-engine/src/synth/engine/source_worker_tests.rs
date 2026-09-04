use super::*;
use std::thread;
use std::time::Duration;

use super::source_worker_test_fixtures::dynamic_engine;

#[test]
fn callback_rendezvous_has_no_sleep_or_yield() {
    let source = include_str!("source_worker.rs");
    assert!(!source.contains("yield_now"));
    assert!(!source.contains("thread::sleep"));
    assert!(!source.contains(&["SOURCE_WORKER", "_POLL_CEILING"].concat()));
    assert!(!source.contains(&["poll", "_limit"].concat()));
    assert!(!source.contains(&["poll", "s"].concat()));
}

#[test]
fn rendezvous_deadline_formula_is_exact_for_supported_rates_and_frames() {
    for sample_rate in [44_100, 48_000] {
        let mut engine = SynthEngine::new(sample_rate);
        let (lifecycle, runtime) =
            SourceWorkerLifecycle::start_prewarmed(&mut engine).expect("worker runtime");
        for frames in [64, 128, 256] {
            let expected = Duration::from_secs_f64(frames as f64 / sample_rate as f64 * 0.35);
            assert_eq!(runtime.deadline_for_test(frames), expected);
            if sample_rate == 44_100 && frames == 128 {
                let completion_ns = 932_251_u64;
                let dispatch_to_deadline_start_ns = 54_417_u64;
                let old_boundary_ns = dispatch_to_deadline_start_ns
                    + Duration::from_secs_f64(frames as f64 / sample_rate as f64 * 0.30).as_nanos()
                        as u64;
                let new_deadline_ns = expected.as_nanos() as u64;
                let new_boundary_ns = dispatch_to_deadline_start_ns + new_deadline_ns;
                let completion_after_deadline_start_ns =
                    completion_ns - dispatch_to_deadline_start_ns;
                assert_eq!(new_deadline_ns, 1_015_873);
                assert!(completion_ns > old_boundary_ns);
                assert!(completion_ns < new_boundary_ns);
                assert_eq!(
                    new_deadline_ns - completion_after_deadline_start_ns,
                    138_039
                );
            }
        }
        let retirement = runtime.retire();
        assert_eq!(lifecycle.shutdown(retirement).joined_workers, 2);
    }
}

#[test]
fn persistent_worker_thread_names_are_stable_and_linux_safe() {
    assert_eq!(
        SOURCE_WORKER_THREAD_NAMES,
        ["oct-dsp-src-0", "oct-dsp-src-1"]
    );
    assert!(SOURCE_WORKER_THREAD_NAMES
        .iter()
        .all(|name| name.len() <= 15));
}

#[test]
fn lifecycle_requires_prewarm_and_joins_both_workers() {
    let mut engine = dynamic_engine();
    let (lifecycle, runtime) =
        SourceWorkerLifecycle::start_prewarmed(&mut engine).expect("prewarmed worker runtime");
    assert_eq!(runtime.mode(), SourceWorkerMode::Persistent);
    let retirement = runtime.retire();
    assert_eq!(lifecycle.shutdown(retirement).joined_workers, 2);
}

#[test]
fn inline_mode_is_explicit_and_has_no_worker_health() {
    let mut runtime = SourceWorkerRuntime::inline();
    assert_eq!(runtime.mode(), SourceWorkerMode::Inline);
    assert_eq!(
        runtime.health_snapshot().status,
        SourceWorkerHealth::Disabled
    );
    let mut engine = dynamic_engine();
    let mut left = Vec::with_capacity(128);
    let mut right = Vec::with_capacity(128);
    let mut out = Vec::with_capacity(256);
    engine.note_on(1, 60, 100, 5_000);
    engine.render_interleaved_block_with_source_runtime(
        &mut runtime,
        128,
        &mut left,
        &mut right,
        &mut out,
    );
    assert!(out.iter().any(|sample| sample.to_bits() != 0));
}

#[test]
fn controls_gate_until_both_complete_bundles_are_home() {
    let mut engine = dynamic_engine();
    engine.note_on(1, 60, 100, 5_000);
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_prewarmed(&mut engine).expect("worker runtime");
    lifecycle.set_pause_for_test(true);
    let revision = engine.synth_render_revisions[1];
    assert!(runtime.dispatch_only_for_test(&mut engine, 128));
    assert!(runtime
        .with_controls_ready(&mut engine, |engine| {
            engine.set_synth_param(1, "synth.filter.cutoffHz", 900.0);
        })
        .is_none());
    assert_eq!(engine.synth_render_revisions[1], revision);

    lifecycle.set_pause_for_test(false);
    let mut collected = false;
    for _ in 0..100_000 {
        if runtime.collect_for_test(&mut engine) {
            collected = true;
            break;
        }
        thread::yield_now();
    }
    assert!(collected);
    assert!(runtime
        .with_controls_ready(&mut engine, |engine| {
            engine.set_synth_param(1, "synth.filter.cutoffHz", 900.0);
            engine.set_instrument_slot(
                1,
                InstrumentSlotConfig {
                    kind: "synth".into(),
                    synth: default_synth_config(),
                    mixer: None,
                },
            );
        })
        .is_some());
    let retirement = runtime.retire();
    assert_eq!(lifecycle.shutdown(retirement).joined_workers, 2);
}

#[test]
fn worker_scratch_is_exactly_one_parity_shape() {
    let mut engine = dynamic_engine();
    let (lifecycle, runtime) =
        SourceWorkerLifecycle::start_prewarmed(&mut engine).expect("worker runtime");
    assert_eq!(
        runtime.scratch_shape_for_test(),
        [
            (SYNTH_VOICE_PARTITION_LANE_CAPACITY, 2048),
            (SYNTH_VOICE_PARTITION_LANE_CAPACITY, 2048)
        ]
    );
    assert!(engine.block_slot_scratch.inline_source_executor.is_none());
    let retirement = runtime.retire();
    let _ = lifecycle.shutdown(retirement);
}

#[test]
fn partition_bundle_take_restores_synth_when_sample_is_missing() {
    let mut engine = dynamic_engine();
    let sample = engine
        .sample_voice_pool
        .take_partition(0)
        .expect("home sample partition");
    assert!(source_worker_transfer::take_source_partition_bundle(&mut engine, 0).is_none());
    assert!(engine.synth_voice_pool.partition_is_present(0));
    assert!(engine
        .sample_voice_pool
        .install_partition(0, sample)
        .is_ok());
    assert!(engine.voice_pools_home());
}

#[test]
fn idle_workers_do_not_start_jobs_or_spin_work() {
    let mut engine = dynamic_engine();
    let (lifecycle, _) =
        SourceWorkerLifecycle::start_prewarmed(&mut engine).expect("worker runtime");
    let before = lifecycle.jobs_started_for_test();
    thread::sleep(Duration::from_millis(5));
    assert_eq!(lifecycle.jobs_started_for_test(), before);
}

#[test]
fn persistent_render_has_no_coordinator_allocation_or_deallocation() {
    let mut engine = dynamic_engine();
    engine.note_on(0, 36, 100, 5_000);
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_prewarmed(&mut engine).expect("worker runtime");
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
    let retirement = runtime.retire();
    assert_eq!(lifecycle.shutdown(retirement).joined_workers, 2);
}

#[test]
fn actual_command_channel_full_is_terminal_and_recovers_both_bundles() {
    let mut engine = dynamic_engine();
    engine.note_on(1, 60, 100, 5_000);
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_prewarmed_held_for_test(&mut engine).expect("worker runtime");
    assert!(lifecycle.fill_work_channel_for_test(0));
    assert!(lifecycle.work_channel_is_full_for_test(0));
    runtime.set_deadline_for_test(Duration::ZERO);
    assert!(!runtime.dispatch_only_for_test(&mut engine, 128));
    assert_eq!(runtime.in_flight_mask_for_test(), 0b10);
    assert!(lifecycle.work_channel_is_full_for_test(0));
    assert!(lifecycle.work_channel_is_full_for_test(1));
    let health = runtime.health_snapshot();
    assert_eq!(health.status, SourceWorkerHealth::DispatchFailed);
    assert_eq!(health.failed_mask, 0b01);
    assert_eq!(health.dispatch_failures, 1);
    assert_eq!(health.completion_failures, 0);
    assert_eq!(health.deadline_misses, 0);
    assert_eq!(health.worker_exits, 0);
    assert!(!engine.voice_pools_home());
    assert!(runtime.with_controls_ready(&mut engine, |_| ()).is_none());
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

    lifecycle.set_hold_before_receive_for_test(false);
    for _ in 0..10_000 {
        if lifecycle.jobs_started_for_test() == [1, 1] {
            break;
        }
        thread::yield_now();
    }
    assert_eq!(lifecycle.jobs_started_for_test(), [1, 1]);
    let mut collected = false;
    for _ in 0..10_000 {
        if runtime.partitions_home_for_test() {
            collected = true;
            break;
        }
        let _ = runtime.collect_for_test(&mut engine);
        thread::yield_now();
    }
    assert!(
        collected,
        "in flight mask: {}",
        runtime.in_flight_mask_for_test()
    );
    assert!(runtime.partitions_home_for_test());
    assert_eq!(runtime.in_flight_mask_for_test(), 0);
    assert_eq!(runtime.health_snapshot(), health);

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

#[test]
fn oversized_block_latches_invalid_without_dispatch() {
    let mut engine = dynamic_engine();
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_prewarmed(&mut engine).expect("worker runtime");
    let mut left = vec![1.0; 128];
    let mut right = vec![1.0; 128];
    let mut out = vec![1.0; 256];
    let (_, allocations, deallocations) =
        crate::synth::test_allocator::count_allocations_and_deallocations(|| {
            engine.render_interleaved_block_with_source_runtime(
                &mut runtime,
                BLOCK_SLOT_SCRATCH_FRAMES + 1,
                &mut left,
                &mut right,
                &mut out,
            );
        });
    assert_eq!((allocations, deallocations), (0, 0));
    assert_eq!(left.len(), 128);
    assert_eq!(right.len(), 128);
    assert_eq!(out.len(), 256);
    assert!(left.iter().all(|sample| sample.to_bits() == 0));
    assert!(right.iter().all(|sample| sample.to_bits() == 0));
    assert!(out.iter().all(|sample| sample.to_bits() == 0));
    assert_eq!(
        runtime.health_snapshot().status,
        SourceWorkerHealth::InvalidBlock
    );
    assert_eq!(runtime.health_snapshot().invalid_blocks, 1);
    assert_eq!(lifecycle.jobs_started_for_test(), [0, 0]);
    let retirement = runtime.retire();
    assert_eq!(lifecycle.shutdown(retirement).joined_workers, 2);
}

#[test]
fn deadline_failure_discards_audio_and_late_completion_recovers() {
    let mut engine = dynamic_engine();
    engine.note_on(1, 60, 100, 5_000);
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_prewarmed(&mut engine).expect("worker runtime");
    lifecycle.set_pause_for_test(true);
    runtime.set_deadline_for_test(Duration::ZERO);
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
    assert_eq!(runtime.health_snapshot().failed_mask, 0b11);
    assert_eq!(runtime.health_snapshot().deadline_misses, 1);
    assert_eq!(runtime.health_snapshot().deadline_recoveries, 0);
    assert_eq!(engine.sample_clock, 0);
    assert!(!engine.voice_pools_home());
    lifecycle.set_pause_for_test(false);
    for _ in 0..100_000 {
        if runtime.partitions_home_for_test() {
            break;
        }
        let _ = runtime.collect_for_test(&mut engine);
        thread::yield_now();
    }
    assert!(runtime.partitions_home_for_test());
    let health = runtime.health_snapshot();
    assert_eq!(health.status, SourceWorkerHealth::Healthy);
    assert_eq!(health.failed_mask, 0);
    assert_eq!(health.deadline_misses, 1);
    assert_eq!(health.deadline_recoveries, 1);
    assert_eq!(engine.sample_clock, 128);
    let retirement = runtime.retire();
    assert_eq!(lifecycle.shutdown(retirement).joined_workers, 2);
}

#[test]
fn disconnected_completion_latches_without_inline_fallback() {
    let mut engine = dynamic_engine();
    engine.note_on(1, 60, 100, 5_000);
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_prewarmed_disconnected_for_test(&mut engine, 0)
            .expect("worker runtime");
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
        SourceWorkerHealth::CompletionFailed
    );
    assert_ne!(runtime.health_snapshot().failed_mask & 1, 0);
    for _ in 0..16 {
        let _ = runtime.collect_for_test(&mut engine);
    }
    assert_eq!(runtime.health_snapshot().completion_failures, 1);
    let retirement = runtime.retire();
    let shutdown = lifecycle.shutdown(retirement);
    assert_eq!(shutdown.joined_workers, 2);
    assert_eq!(shutdown.destroyed_owner_count, 2);
}

#[test]
fn stale_completion_latches_and_is_not_reduced() {
    let mut engine = dynamic_engine();
    engine.note_on(1, 60, 100, 5_000);
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_prewarmed(&mut engine).expect("worker runtime");
    assert!(runtime.dispatch_only_for_test(&mut engine, 128));
    let mut rewritten = false;
    for _ in 0..100_000 {
        if runtime.rewrite_completion_sequence_for_test(0) {
            rewritten = true;
            break;
        }
        thread::yield_now();
    }
    assert!(rewritten);
    let _ = runtime.collect_for_test(&mut engine);
    assert_eq!(
        runtime.health_snapshot().status,
        SourceWorkerHealth::CompletionFailed
    );
    assert!(lifecycle.fault_owner_identities_for_test()[0].is_some());
    assert!(runtime.home_owner_identity_for_test(1).is_some());
    let retirement = runtime.retire();
    assert_eq!(lifecycle.shutdown(retirement).joined_workers, 2);
}

#[test]
fn worker_exit_latches_and_never_restarts_or_falls_back() {
    for _ in 0..100 {
        assert_worker_exit_ownership_is_recovered_home();
    }
}

fn assert_worker_exit_ownership_is_recovered_home() {
    let mut engine = dynamic_engine();
    engine.note_on(1, 60, 100, 5_000);
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_prewarmed_held_for_test(&mut engine).expect("worker runtime");
    let initial_identities = runtime.home_owner_identities_for_test();
    lifecycle.set_exit_on_job_for_test(0);
    lifecycle.set_exit_on_job_for_test(1);
    runtime.set_deadline_for_test(Duration::from_secs(1));
    assert!(runtime.dispatch_only_for_test(&mut engine, 128));
    lifecycle.set_hold_before_receive_for_test(false);
    let mut workers_exited = false;
    for _ in 0..100_000 {
        if runtime.workers_exited_for_test() == [true, true] {
            workers_exited = true;
            break;
        }
        thread::yield_now();
    }
    assert!(workers_exited);

    let mut owners_recovered_fault = false;
    for _ in 0..100_000 {
        let _ = runtime.collect_for_test(&mut engine);
        if runtime.in_flight_mask_for_test() == 0
            && lifecycle.fault_owner_identities_for_test()
                == [Some(initial_identities[0]), Some(initial_identities[1])]
        {
            owners_recovered_fault = true;
            break;
        }
        thread::yield_now();
    }
    assert!(owners_recovered_fault);
    assert_eq!(
        runtime.health_snapshot().status,
        SourceWorkerHealth::WorkerExited
    );
    assert_eq!(runtime.in_flight_mask_for_test(), 0);
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
    let retirement = runtime.retire();
    let shutdown = lifecycle.shutdown(retirement);
    assert_eq!(shutdown.joined_workers, 2);
    assert_eq!(shutdown.retirement_error, None);
    assert_eq!(shutdown.destroyed_owner_count, 2);
    assert_eq!(
        shutdown.destroyed_owner_identities,
        [Some(initial_identities[0]), Some(initial_identities[1])]
    );
}
