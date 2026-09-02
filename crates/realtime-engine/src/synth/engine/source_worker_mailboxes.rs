use super::super::source_worker_lease::OwnerLease;
use super::super::source_worker_load::SourceWorkerLoadObservation;
use super::CompletedEnvelope;
use super::SourceWorkerHealth;
use super::*;

impl SourceWorkerRuntime {
    pub(super) fn accept_completion(&mut self, completion: CompletedEnvelope) {
        let parity = completion.owner.parity;
        let worker_mask = worker_mask(parity);
        let expected = parity < SOURCE_WORKER_COUNT
            && self.in_flight_mask & worker_mask != 0
            && self.expected_sequence == Some(completion.sequence)
            && completion.frames == self.expected_frames
            && completion.base_sample_clock == self.expected_base_sample_clock
            && completion.render_ok
            && completion.active_cost_units
                <= super::super::source_worker_load::SOURCE_WORKER_MAX_COST_UNITS;
        if parity >= SOURCE_WORKER_COUNT
            || completion.worker_exited
            || completion.transport_failed
            || !expected
        {
            self.health.latch(
                if completion.worker_exited {
                    SourceWorkerHealth::WorkerExited
                } else {
                    SourceWorkerHealth::CompletionFailed
                },
                worker_mask,
            );
            self.in_flight_mask &= !worker_mask;
            self.load_observations = std::array::from_fn(|_| None);
            self.return_owner(completion.owner, true);
            return;
        }
        #[cfg(feature = "source-worker-benchmark-timing")]
        if let (Some(probe), Some(dispatch_started_at)) =
            (self.timing_probe.as_ref(), self.dispatch_started_at)
        {
            probe.record_completion(completion.sequence, parity, dispatch_started_at.elapsed());
        }
        self.health.record_completion(completion.sequence);
        self.in_flight_mask &= !worker_mask;
        self.completed_mask |= worker_mask;
        if self.health.status() == SourceWorkerHealth::Healthy {
            self.load_observations[parity] = Some(SourceWorkerLoadObservation {
                dsp_duration_ns: completion.dsp_duration_ns,
                active_cost_units: completion.active_cost_units,
            });
        }
        self.return_owner(completion.owner, true);
    }

    pub(super) fn reclaim_available(&mut self, _engine: &mut SynthEngine) {
        if self.mode == SourceWorkerMode::Inline {
            return;
        }
        for parity in 0..SOURCE_WORKER_COUNT {
            if !self.home_is_empty(parity) {
                continue;
            }
            let Some(done_rxs) = self.done_rxs.as_ref() else {
                self.latch_completion_failure(1 << parity);
                continue;
            };
            match done_rxs[parity].try_recv() {
                Ok(completion) => self.accept_completion(completion),
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {
                    self.latch_completion_failure(1 << parity);
                    self.in_flight_mask &= !(1 << parity);
                }
            }
        }
    }

    fn return_owner(&self, owner: OwnerEnvelope, home: bool) {
        let Some(mut lease) = self.owner_lease(owner) else {
            self.health
                .latch(SourceWorkerHealth::CompletionFailed, 0b11);
            return;
        };
        if home {
            lease.return_home();
        } else {
            lease.return_fault();
        }
    }

    fn owner_lease(&self, owner: OwnerEnvelope) -> Option<OwnerLease> {
        let parity = owner.parity;
        if parity >= SOURCE_WORKER_COUNT {
            return None;
        }
        Some(OwnerLease {
            owner: Some(owner),
            parity,
            home_tx: self.home_txs.as_ref()?[parity].clone(),
            fault_tx: self.fault_txs.as_ref()?[parity].clone(),
            health: Arc::clone(&self.health),
        })
    }
}

fn worker_mask(parity: usize) -> u8 {
    if parity < SOURCE_WORKER_COUNT {
        1 << parity
    } else {
        0b11
    }
}
