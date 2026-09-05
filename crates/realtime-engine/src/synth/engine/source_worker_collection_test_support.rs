use super::super::source_worker_protocol::WorkerPhase;
use super::*;
use std::time::Instant;

impl SourceWorkerRuntime {
    #[cfg(feature = "routing-tree-benchmark")]
    pub(crate) fn routing_tree_worker_outputs_for_test(
        &self,
        frame: usize,
    ) -> [(f32, f32); super::SOURCE_WORKER_COUNT] {
        self.routing_output_spares
            .as_ref()
            .expect("routing-tree output spares")
            .each_ref()
            .map(|output| (output.left[frame], output.right[frame]))
    }

    pub(crate) fn collect_for_test(&mut self, engine: &mut SynthEngine) -> bool {
        if self.health.status().is_recovering() {
            self.refresh_recovery(engine)
        } else if let Some(result) = self.collect_routing_tree_for_test(engine, false) {
            result
        } else if self.expected_phase == Some(WorkerPhase::Buses) {
            let deadline = Instant::now()
                + self.rendezvous_deadline(self.expected_stamp.map_or(0, |stamp| stamp.frames));
            self.collect_wave_with_deadline(engine, false, WorkerPhase::Buses, deadline, false)
                .is_some()
        } else {
            self.collect(engine, false)
        }
    }

    pub(crate) fn collect_wait_for_test(&mut self, engine: &mut SynthEngine) -> bool {
        if self.health.status().is_recovering() {
            self.refresh_recovery(engine)
        } else if let Some(result) = self.collect_routing_tree_for_test(engine, true) {
            result
        } else if self.expected_phase == Some(WorkerPhase::Buses) {
            let deadline = Instant::now()
                + self.rendezvous_deadline(self.expected_stamp.map_or(0, |stamp| stamp.frames));
            self.collect_wave_with_deadline(engine, true, WorkerPhase::Buses, deadline, false)
                .is_some()
        } else {
            self.collect(engine, true)
        }
    }

    fn collect_routing_tree_for_test(
        &mut self,
        engine: &mut SynthEngine,
        wait: bool,
    ) -> Option<bool> {
        #[cfg(feature = "routing-tree-benchmark")]
        if self.expected_phase == Some(WorkerPhase::RoutingTree) {
            return Some(self.collect_routing_tree_output(engine, wait).is_some());
        }
        let _ = (engine, wait);
        None
    }
}
