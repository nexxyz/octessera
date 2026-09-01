use super::{SourceWorkerRuntime, SOURCE_WORKER_COUNT};
use std::sync::atomic::Ordering;
use std::time::Duration;

impl SourceWorkerRuntime {
    pub fn jobs_started_for_test(&self) -> [u64; SOURCE_WORKER_COUNT] {
        self.jobs_started
            .as_ref()
            .map_or([0; SOURCE_WORKER_COUNT], |jobs| {
                jobs.each_ref().map(|job| job.load(Ordering::Relaxed))
            })
    }

    pub fn set_timing_for_test(&mut self, poll_limit: usize, deadline: Duration) {
        self.timing_override = Some((poll_limit, deadline));
    }

    pub fn render_attempts_for_test(&self) -> u64 {
        self.render_attempts.load(Ordering::Relaxed)
    }

    pub fn completion_states_for_test(&self) -> [bool; SOURCE_WORKER_COUNT] {
        self.done_rxs
            .as_ref()
            .map_or([false; SOURCE_WORKER_COUNT], |done_rxs| {
                done_rxs.each_ref().map(|done_rx| !done_rx.is_empty())
            })
    }
}
