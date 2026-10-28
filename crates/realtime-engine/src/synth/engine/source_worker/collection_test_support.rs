use super::*;

impl SourceWorkerRuntime {
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

    pub(crate) fn collect_wait_for_test(&mut self, engine: &mut SynthEngine) -> bool {
        self.collect_routing_tree_or_recovery(engine, true)
    }

    fn collect_routing_tree_or_recovery(&mut self, engine: &mut SynthEngine, wait: bool) -> bool {
        if self.health.status().is_recovering() {
            self.refresh_recovery(engine)
        } else {
            self.collect_routing_tree_for_test(engine, wait)
                .unwrap_or(false)
        }
    }

    fn collect_routing_tree_for_test(
        &mut self,
        engine: &mut SynthEngine,
        wait: bool,
    ) -> Option<bool> {
        if self.expected_stamp.is_some() {
            return Some(self.collect_routing_tree_output(engine, wait).is_some());
        }
        let _ = (engine, wait);
        None
    }
}
