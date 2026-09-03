use super::super::source_worker_protocol::WorkerPhase;
use super::*;
use std::time::Instant;

impl SourceWorkerRuntime {
    pub(crate) fn collect_for_test(&mut self, engine: &mut SynthEngine) -> bool {
        if self.health.status().is_recovering() {
            self.refresh_recovery(engine)
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
        } else if self.expected_phase == Some(WorkerPhase::Buses) {
            let deadline = Instant::now()
                + self.rendezvous_deadline(self.expected_stamp.map_or(0, |stamp| stamp.frames));
            self.collect_wave_with_deadline(engine, true, WorkerPhase::Buses, deadline, false)
                .is_some()
        } else {
            self.collect(engine, true)
        }
    }
}
