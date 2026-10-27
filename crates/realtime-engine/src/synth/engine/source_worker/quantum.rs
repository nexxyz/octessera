use super::super::source_worker_health::SourceWorkerHealth;
use super::super::source_worker_protocol::WorkerPhase;
use super::super::SynthEngine;
use super::{SourceWorkerRuntime, SOURCE_WORKER_COUNT};
use crossbeam_channel::TryRecvError;
use std::time::Instant;

impl SourceWorkerRuntime {
    pub(super) fn collect_wave_with_deadline(
        &mut self,
        engine: &mut SynthEngine,
        wait: bool,
        phase: WorkerPhase,
        deadline: Instant,
    ) -> Option<()> {
        if self.expected_phase != Some(phase) {
            self.latch_completion_failure(0b11);
            return None;
        }
        while self.in_flight_mask != 0 {
            for parity in 0..SOURCE_WORKER_COUNT {
                if self.in_flight_mask & (1 << parity) == 0 || !self.home_is_empty(parity) {
                    continue;
                }
                let receive_result = self
                    .done_rxs
                    .as_ref()
                    .map(|done_rxs| done_rxs[parity].try_recv());
                match receive_result {
                    Some(Ok(completion)) => self.accept_completion(parity, completion),
                    Some(Err(TryRecvError::Empty)) | None => {}
                    Some(Err(TryRecvError::Disconnected)) => {
                        self.latch_completion_failure(1 << parity);
                        self.in_flight_mask &= !(1 << parity);
                    }
                }
            }
            if self.health.status() != SourceWorkerHealth::Healthy {
                if self.health.status().is_recovering() {
                    return None;
                }
                self.reclaim_available(engine);
            }
            if self.in_flight_mask == 0 {
                break;
            }
            if !wait && self.health.status() == SourceWorkerHealth::Healthy {
                break;
            }
            if Instant::now() >= deadline {
                if self.health.status() == SourceWorkerHealth::Healthy {
                    self.latch_deadline_or_exit();
                }
                if self.health.status().is_recovering() {
                    return None;
                }
                self.reclaim_available(engine);
                return None;
            }
        }
        if self.health.status() != SourceWorkerHealth::Healthy {
            if self.health.status().is_recovering() {
                return None;
            }
            self.reclaim_available(engine);
            return None;
        }
        if self.in_flight_mask != 0 {
            return None;
        }
        Some(())
    }
}
