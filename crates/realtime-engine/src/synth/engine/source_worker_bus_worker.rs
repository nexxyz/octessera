use super::super::super::source_worker_protocol::WorkerCommand;
use super::{
    finish_worker, send_completion, CompletedEnvelope, SourceWorkerThreadState, WorkerExit,
};
use crossbeam_channel::Sender;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::Ordering;
use std::time::Instant;

pub(super) fn process(
    command: WorkerCommand,
    parity: usize,
    done_tx: &Sender<CompletedEnvelope>,
    state: &SourceWorkerThreadState,
) -> Option<WorkerExit> {
    let WorkerCommand::RenderBuses {
        mut owner,
        stamp,
        frames,
        sample_rate,
        bus_idle_threshold,
        fx_activity_hold_frames,
        #[cfg(feature = "source-worker-benchmark-timing")]
        dispatch_started_at,
        #[cfg(feature = "source-worker-benchmark-timing")]
        timing_probe,
    } = command
    else {
        unreachable!("bus worker received a source command");
    };
    state.jobs_started.fetch_add(1, Ordering::Relaxed);
    #[cfg(any(test, feature = "test-support"))]
    let should_panic = state.panic_on_job.swap(false, Ordering::AcqRel);
    #[cfg(not(any(test, feature = "test-support")))]
    let should_panic = false;
    #[cfg(any(test, feature = "test-support"))]
    let should_exit = state.exit_on_bus.swap(false, Ordering::AcqRel);
    #[cfg(not(any(test, feature = "test-support")))]
    let should_exit = false;
    #[cfg(any(test, feature = "test-support"))]
    while state.pause.load(Ordering::Acquire) {
        std::hint::spin_loop();
    }
    #[cfg(any(test, feature = "test-support"))]
    let should_panic_bus = state.panic_on_bus.swap(false, Ordering::AcqRel);
    #[cfg(not(any(test, feature = "test-support")))]
    let should_panic_bus = false;
    let render_started_at = Instant::now();
    let render_result = catch_unwind(AssertUnwindSafe(|| {
        if should_exit {
            panic!("bus worker test exit");
        }
        if should_panic || should_panic_bus {
            panic!("bus worker test panic");
        }
        super::super::super::source_worker_bus::render_bus_block(
            &mut owner,
            parity,
            stamp,
            frames,
            sample_rate,
            bus_idle_threshold,
            fx_activity_hold_frames,
        )
    }));
    let dsp_duration_ns = render_started_at
        .elapsed()
        .as_nanos()
        .min(u128::from(u64::MAX)) as u64;
    #[cfg(feature = "source-worker-benchmark-timing")]
    if matches!(render_result, Ok(Ok(_))) {
        if let Some(probe) = timing_probe.as_ref() {
            probe.record_bus_worker(
                parity,
                stamp.quantum_sequence,
                dsp_duration_ns,
                dispatch_started_at,
            );
        }
    }
    let (render_ok, worker_exited, active_cost_units) = match render_result {
        Ok(Ok(active_cost_units)) => (true, false, active_cost_units),
        Ok(Err(())) => (false, false, 0),
        Err(_) => (false, true, 0),
    };
    let completion = CompletedEnvelope::from_bus_work(
        owner,
        stamp,
        worker_exited,
        render_ok,
        dsp_duration_ns,
        active_cost_units,
    );
    if let Some(exit) = send_completion(done_tx, completion) {
        return Some(finish_worker(state, Some(exit)));
    }
    if worker_exited {
        return Some(finish_worker(state, None));
    }
    None
}
