use super::{SourceWorkerRuntime, SOURCE_WORKER_COUNT};
#[cfg(test)]
use crossbeam_channel::bounded;
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

    pub fn wait_until_paused_for_test(&self, parity: usize, timeout: Duration) -> bool {
        let entered = self
            .worker_pause_entries
            .as_ref()
            .expect("source worker pause entry controls")
            .get(parity)
            .expect("source worker parity");
        let deadline = std::time::Instant::now() + timeout;
        while !entered.load(Ordering::Acquire) {
            if std::time::Instant::now() >= deadline {
                return false;
            }
            std::thread::yield_now();
        }
        true
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

    #[cfg(feature = "routing-tree-benchmark")]
    pub fn routing_tree_deadline_for_test(&self, frames: usize) -> Duration {
        self.routing_tree_deadline_duration(frames)
    }

    #[cfg(feature = "routing-tree-benchmark")]
    pub fn routing_absolute_deadline_for_test(&self) -> Option<std::time::Instant> {
        self.routing_absolute_deadline
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

    #[cfg(test)]
    pub fn disconnect_completion_for_test(&mut self, parity: usize) {
        let (sender, receiver) = bounded(0);
        drop(sender);
        self.done_rxs.as_mut().expect("persistent source workers")[parity] = receiver;
    }
}
