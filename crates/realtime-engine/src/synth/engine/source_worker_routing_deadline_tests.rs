use super::*;
use std::thread;
use std::time::{Duration, Instant};

fn start_runtime(sample_rate: u32) -> (SynthEngine, SourceWorkerLifecycle, SourceWorkerRuntime) {
    let mut engine = SynthEngine::new(sample_rate);
    let (lifecycle, runtime) =
        SourceWorkerLifecycle::start_routing_tree_prewarmed(&mut engine, 256)
            .expect("routing-tree runtime");
    (engine, lifecycle, runtime)
}

fn shutdown(lifecycle: SourceWorkerLifecycle, runtime: SourceWorkerRuntime) {
    assert_eq!(lifecycle.shutdown(runtime.retire()).joined_workers, 2);
}

#[test]
fn routing_tree_deadline_formula_is_exact_for_supported_rates_and_frames() {
    for sample_rate in [44_100, 48_000] {
        let (_engine, lifecycle, runtime) = start_runtime(sample_rate);
        assert_eq!(runtime.routing_absolute_deadline_for_test(), None);
        for frames in [64, 128, 256] {
            let expected = Duration::from_secs_f64(frames as f64 / sample_rate as f64 * 0.85);
            assert_eq!(runtime.routing_tree_deadline_for_test(frames), expected);
        }
        shutdown(lifecycle, runtime);
    }
}

#[test]
fn routing_tree_deadline_override_is_authoritative() {
    let (_engine, lifecycle, mut runtime) = start_runtime(48_000);
    runtime.set_deadline_for_test(Duration::ZERO);
    assert_eq!(runtime.routing_tree_deadline_for_test(128), Duration::ZERO);
    runtime.set_deadline_for_test(Duration::from_secs(1));
    assert_eq!(
        runtime.routing_tree_deadline_for_test(128),
        Duration::from_secs(1)
    );
    shutdown(lifecycle, runtime);
}

#[test]
fn routing_tree_deadline_is_anchored_at_dispatch_and_not_extended_at_collect() {
    let (mut engine, lifecycle, mut runtime) = start_runtime(48_000);
    let budget = Duration::from_millis(40);
    runtime.set_deadline_for_test(budget);
    runtime.set_pause_for_parity_for_test(0, true);
    let before_dispatch = Instant::now();
    assert!(runtime.dispatch_routing_tree_for_test(&engine, 128, engine.sample_clock));
    assert!(runtime.wait_until_paused_for_test(0, Duration::from_secs(1)));
    let after_dispatch = Instant::now();
    let deadline = runtime
        .routing_absolute_deadline_for_test()
        .expect("routing-tree deadline");
    assert!(deadline >= before_dispatch + budget);
    assert!(deadline <= after_dispatch + budget);

    thread::sleep(Duration::from_millis(80));
    let collect_started = Instant::now();
    assert!(!runtime.collect_wait_for_test(&mut engine));
    assert!(collect_started.elapsed() < Duration::from_millis(20));
    assert_eq!(
        runtime.health_snapshot().status,
        SourceWorkerHealth::DeadlineMiss
    );
    assert_eq!(runtime.routing_absolute_deadline_for_test(), None);

    runtime.set_pause_for_parity_for_test(0, false);
    shutdown(lifecycle, runtime);
}

#[test]
fn routing_tree_deadline_clears_after_successful_collect() {
    let (mut engine, lifecycle, mut runtime) = start_runtime(48_000);
    runtime.set_deadline_for_test(Duration::from_secs(1));
    assert!(runtime.dispatch_routing_tree_for_test(&engine, 128, engine.sample_clock));
    assert!(runtime.routing_absolute_deadline_for_test().is_some());
    assert!(runtime.collect_wait_for_test(&mut engine));
    assert_eq!(runtime.routing_absolute_deadline_for_test(), None);
    shutdown(lifecycle, runtime);
}

#[test]
fn routing_tree_deadline_clears_after_recovery() {
    let (mut engine, lifecycle, mut runtime) = start_runtime(48_000);
    runtime.set_deadline_for_test(Duration::ZERO);
    runtime.set_pause_for_parity_for_test(0, true);
    assert!(runtime.dispatch_routing_tree_for_test(&engine, 128, engine.sample_clock));
    assert!(runtime.wait_until_paused_for_test(0, Duration::from_secs(1)));
    assert!(!runtime.collect_wait_for_test(&mut engine));
    assert_eq!(runtime.routing_absolute_deadline_for_test(), None);

    runtime.set_pause_for_parity_for_test(0, false);
    runtime.set_deadline_for_test(Duration::from_secs(1));
    let recovery_deadline = Instant::now() + Duration::from_secs(1);
    while !runtime.refresh_recovery(&mut engine) {
        assert!(Instant::now() < recovery_deadline);
        thread::yield_now();
    }
    assert_eq!(runtime.routing_absolute_deadline_for_test(), None);
    shutdown(lifecycle, runtime);
}

#[test]
fn routing_tree_deadline_clears_after_fatal_collect() {
    let (mut engine, lifecycle, mut runtime) = start_runtime(48_000);
    lifecycle.set_panic_on_job_for_test(0);
    runtime.set_deadline_for_test(Duration::from_secs(1));
    assert!(runtime.dispatch_routing_tree_for_test(&engine, 128, engine.sample_clock));
    assert!(runtime.routing_absolute_deadline_for_test().is_some());
    assert!(!runtime.collect_wait_for_test(&mut engine));
    assert_eq!(
        runtime.health_snapshot().status,
        SourceWorkerHealth::WorkerExited
    );
    assert_eq!(runtime.routing_absolute_deadline_for_test(), None);
    shutdown(lifecycle, runtime);
}
