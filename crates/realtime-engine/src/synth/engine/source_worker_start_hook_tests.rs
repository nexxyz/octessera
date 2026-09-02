use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::Duration;

use super::source_worker_test_fixtures::dynamic_engine;

static START_HOOK_CALLS: AtomicUsize = AtomicUsize::new(0);
static START_HOOK_PARITIES: AtomicUsize = AtomicUsize::new(0);

fn record_start_hook(parity: usize) -> Result<(), ()> {
    if thread::current().name() != Some(SOURCE_WORKER_THREAD_NAMES[parity]) {
        return Err(());
    }
    START_HOOK_CALLS.fetch_add(1, Ordering::AcqRel);
    START_HOOK_PARITIES.fetch_or(1 << parity, Ordering::AcqRel);
    Ok(())
}

fn fail_parity_one_start_hook(parity: usize) -> Result<(), ()> {
    if parity == 1 {
        Err(())
    } else {
        Ok(())
    }
}

#[test]
fn start_hook_runs_in_both_named_workers_before_prewarm_returns() {
    START_HOOK_CALLS.store(0, Ordering::Release);
    START_HOOK_PARITIES.store(0, Ordering::Release);
    let mut engine = dynamic_engine();
    let (lifecycle, runtime) =
        SourceWorkerLifecycle::start_prewarmed_with_hook(&mut engine, record_start_hook)
            .expect("scheduling hook should qualify both workers");

    assert_eq!(START_HOOK_CALLS.load(Ordering::Acquire), 2);
    assert_eq!(START_HOOK_PARITIES.load(Ordering::Acquire), 3);
    let retirement = runtime.retire();
    assert_eq!(lifecycle.shutdown(retirement).joined_workers, 2);
}

#[test]
fn start_hook_failure_is_typed_and_joins_both_workers_without_owners() {
    let (probe_tx, probe_rx) = crossbeam_channel::bounded(1);
    let _probe_guard = install_source_worker_shutdown_probe_for_test(probe_tx);
    let mut engine = dynamic_engine();
    let error = match SourceWorkerLifecycle::start_prewarmed_with_hook(
        &mut engine,
        fail_parity_one_start_hook,
    ) {
        Ok(_) => panic!("parity-one scheduling failure should reject setup"),
        Err(error) => error,
    };

    assert_eq!(
        error,
        SourceWorkerSetupError::WorkerSchedulingUnavailable { parity: 1 }
    );
    let (shutdown, thread_id) = probe_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("failed setup should report worker shutdown");
    assert_eq!(thread_id, thread::current().id());
    assert_eq!(shutdown.joined_workers, 2);
    assert_eq!(shutdown.destroyed_owner_count, 0);
}
