use super::SourceWorkerRuntime;
use std::time::Duration;
#[cfg(feature = "routing-tree-executor")]
use std::time::Instant;

#[cfg(feature = "routing-tree-executor")]
const ROUTING_TREE_DEADLINE_FRACTION: f64 = 0.85;

impl SourceWorkerRuntime {
    #[cfg(feature = "routing-tree-executor")]
    pub(super) fn routing_tree_deadline_duration(&self, frames: usize) -> Duration {
        #[cfg(all(any(test, feature = "test-support"), feature = "routing-tree-executor"))]
        if let Some(deadline) = self.deadline_override {
            return deadline;
        }
        let quantum_seconds = if self.sample_rate == 0 {
            0.0
        } else {
            frames as f64 / self.sample_rate as f64
        };
        Duration::from_secs_f64(quantum_seconds * ROUTING_TREE_DEADLINE_FRACTION)
    }

    #[cfg(feature = "routing-tree-executor")]
    pub(super) fn set_routing_absolute_deadline(
        &mut self,
        dispatch_started_at: Instant,
        frames: usize,
    ) {
        self.routing_absolute_deadline = Some(
            dispatch_started_at
                .checked_add(self.routing_tree_deadline_duration(frames))
                .expect("routing-tree deadline overflow"),
        );
    }

    #[cfg(feature = "routing-tree-executor")]
    pub(super) fn clear_routing_absolute_deadline(&mut self) {
        self.routing_absolute_deadline = None;
    }
}
