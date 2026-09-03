use super::super::source_worker_protocol::{WorkStamp, WorkerPhase};
use super::*;
use crossbeam_channel::bounded;

impl SourceWorkerRuntime {
    pub(crate) fn bus_dispatch_residency_for_test(
        &self,
    ) -> Option<[u8; super::super::super::types::BUS_COUNT]> {
        self.bus_dispatch_residency_valid
            .then_some(self.bus_dispatch_residency)
    }

    pub(crate) fn pending_recovery_state_for_test(
        &self,
    ) -> (Option<WorkerPhase>, Option<WorkStamp>, u8, u8) {
        (
            self.expected_phase,
            self.expected_stamp,
            self.in_flight_mask,
            self.completed_mask,
        )
    }

    pub(crate) fn disconnect_recovery_completion_for_test(&mut self, parity: usize) {
        let (sender, receiver) = bounded(0);
        drop(sender);
        self.done_rxs.as_mut().expect("persistent source workers")[parity] = receiver;
    }
}
