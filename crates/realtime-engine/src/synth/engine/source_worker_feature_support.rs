use super::{SourceWorkerRuntime, SOURCE_WORKER_COUNT};
use std::sync::atomic::Ordering;
use std::time::Duration;

impl SourceWorkerRuntime {
    pub fn set_pause_for_parity_for_test(&self, parity: usize, paused: bool) {
        self.worker_pauses
            .as_ref()
            .expect("source worker pause controls")
            .get(parity)
            .expect("source worker parity")
            .store(paused, Ordering::Release);
    }

    pub fn jobs_started_for_test(&self) -> [u64; SOURCE_WORKER_COUNT] {
        self.jobs_started
            .as_ref()
            .map_or([0; SOURCE_WORKER_COUNT], |jobs| {
                jobs.each_ref().map(|job| job.load(Ordering::Relaxed))
            })
    }

    pub fn set_deadline_for_test(&mut self, deadline: Duration) {
        self.deadline_override = Some(deadline);
    }

    pub fn deadline_for_test(&self, frames: usize) -> Duration {
        self.rendezvous_deadline(frames)
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
