use super::super::super::source_worker_owner::CompletedEnvelope;
use super::finish_worker;
use super::{send_completion, SourceWorkerThreadState, WorkerExit};
use crossbeam_channel::Sender;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::Ordering;
use std::time::Instant;

pub(super) fn process(
    command: super::super::super::source_worker_owner::WorkerCommand,
    parity: usize,
    done_tx: &Sender<CompletedEnvelope>,
    state: &SourceWorkerThreadState,
) -> Option<WorkerExit> {
    let Some(mut work) = command.into_routing_tree_work() else {
        unreachable!("routing-tree worker received a non-routing command");
    };
    state.jobs_started.fetch_add(1, Ordering::Relaxed);
    #[cfg(any(test, feature = "test-support"))]
    if state.exit_on_job.swap(false, Ordering::AcqRel) {
        return Some(finish_worker(
            state,
            send_completion(
                done_tx,
                CompletedEnvelope::from_routing_tree_work(work, true, false, 0, 0),
            ),
        ));
    }
    #[cfg(any(test, feature = "test-support"))]
    state.pause_entered.store(true, Ordering::Release);
    #[cfg(any(test, feature = "test-support"))]
    while state.pause.load(Ordering::Acquire) {
        std::hint::spin_loop();
    }
    #[cfg(any(test, feature = "test-support"))]
    state.pause_entered.store(false, Ordering::Release);
    #[cfg(any(test, feature = "test-support"))]
    let should_panic = state.panic_on_job.swap(false, Ordering::AcqRel);
    #[cfg(not(any(test, feature = "test-support")))]
    let should_panic = false;
    #[cfg(feature = "source-worker-benchmark-timing")]
    let timing_start = work.timing_probe.as_ref().map(|probe| probe.worker_start());
    let render_started_at = {
        #[cfg(feature = "source-worker-benchmark-timing")]
        {
            timing_start.map_or_else(Instant::now, |start| {
                super::SourceWorkerTimingProbe::render_start(start)
            })
        }
        #[cfg(not(feature = "source-worker-benchmark-timing"))]
        {
            Instant::now()
        }
    };
    let source_cost_units = work.active_cost_units();
    let render_result = catch_unwind(AssertUnwindSafe(|| {
        if work.owner.parity != parity {
            return Err(());
        }
        if should_panic {
            panic!("routing-tree worker test panic");
        }
        work.render()
    }));
    let (render_ok, bus_cost_units) = match render_result {
        Ok(Ok(bus_cost_units)) => (true, bus_cost_units),
        Ok(Err(())) | Err(_) => (false, 0),
    };
    let active_cost_units = source_cost_units.saturating_add(bus_cost_units);
    let dsp_duration_ns = render_started_at
        .elapsed()
        .as_nanos()
        .min(u128::from(u64::MAX)) as u64;
    #[cfg(feature = "source-worker-benchmark-timing")]
    if render_ok {
        if let (Some(probe), Some(start)) = (work.timing_probe.as_ref(), timing_start) {
            probe.record_worker(
                parity,
                work.stamp.quantum_sequence,
                start,
                dsp_duration_ns,
                work.dispatch_started_at,
            );
        }
    }
    if !render_ok {
        return Some(finish_worker(
            state,
            send_completion(
                done_tx,
                CompletedEnvelope::from_routing_tree_work(
                    work,
                    true,
                    false,
                    dsp_duration_ns,
                    active_cost_units,
                ),
            ),
        ));
    }
    #[cfg(test)]
    super::wait_for_reverse_completion(state, parity);
    let completion = CompletedEnvelope::from_routing_tree_work(
        work,
        false,
        render_ok,
        dsp_duration_ns,
        active_cost_units,
    );
    let send_result = send_completion(done_tx, completion);
    #[cfg(test)]
    if send_result.is_none() {
        state.reverse_completion.observe_send(parity);
    }
    send_result
}
