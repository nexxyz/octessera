use super::super::source_worker_carrier_transfer;
use super::super::source_worker_lease::OwnerLease;
use super::super::source_worker_load::SourceWorkerLoadObservation;
use super::CompletedEnvelope;
use super::SourceWorkerHealth;
use super::*;
use crossbeam_channel::TryRecvError;

impl SourceWorkerRuntime {
    pub(super) fn accept_completion(
        &mut self,
        channel_parity: usize,
        completion: CompletedEnvelope,
    ) {
        let parity = completion.owner.parity;
        let worker_mask = worker_mask(parity);
        let expected = self.completion_is_valid(channel_parity, &completion);
        if parity >= SOURCE_WORKER_COUNT
            || completion.worker_exited
            || completion.transport_failed
            || self.force_fault_mask & worker_mask != 0
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
            self.source_load_observations = std::array::from_fn(|_| None);
            self.bus_load_observations = std::array::from_fn(|_| None);
            self.force_fault_mask &= !worker_mask;
            self.return_owner(completion.owner, false, channel_parity);
            return;
        }
        #[cfg(feature = "source-worker-benchmark-timing")]
        if let (Some(probe), Some(dispatch_started_at)) =
            (self.timing_probe.as_ref(), self.dispatch_started_at)
        {
            if completion.phase != super::super::source_worker_protocol::WorkerPhase::Buses {
                probe.record_completion(
                    completion.stamp.quantum_sequence,
                    parity,
                    dispatch_started_at.elapsed(),
                );
            } else {
                probe.record_bus_completion(
                    completion.stamp.quantum_sequence,
                    parity,
                    dispatch_started_at.elapsed(),
                );
            }
        }
        self.health
            .record_completion(completion.stamp.quantum_sequence);
        self.in_flight_mask &= !worker_mask;
        self.completed_mask |= worker_mask;
        if self.health.status() == SourceWorkerHealth::Healthy {
            let observation = Some(SourceWorkerLoadObservation {
                dsp_duration_ns: completion.dsp_duration_ns,
                active_cost_units: completion.active_cost_units,
            });
            if completion.phase != super::super::source_worker_protocol::WorkerPhase::Buses {
                self.source_load_observations[parity] = observation;
            } else {
                self.bus_load_observations[parity] = observation;
            }
        }
        self.return_owner(completion.owner, true, parity);
    }

    pub(super) fn completion_is_valid(
        &self,
        channel_parity: usize,
        completion: &CompletedEnvelope,
    ) -> bool {
        let parity = completion.owner.parity;
        let worker_mask = worker_mask(parity);
        #[cfg(feature = "routing-tree-benchmark")]
        let max_cost_units = if completion.phase == WorkerPhase::RoutingTree {
            super::super::routing_tree_worker::ROUTING_TREE_MAX_COST_UNITS
        } else {
            super::super::source_worker_load::SOURCE_WORKER_MAX_COST_UNITS
        };
        #[cfg(not(feature = "routing-tree-benchmark"))]
        let max_cost_units = super::super::source_worker_load::SOURCE_WORKER_MAX_COST_UNITS;
        parity < SOURCE_WORKER_COUNT
            && channel_parity == parity
            && self.in_flight_mask & worker_mask != 0
            && completion.owner.runtime_generation == self.runtime_generation
            && self.expected_phase == Some(completion.phase)
            && self.expected_stamp == Some(completion.stamp)
            && completion.render_ok
            && (completion.phase != super::super::source_worker_protocol::WorkerPhase::Buses
                || (self.bus_dispatch_residency_valid
                    && source_worker_carrier_transfer::valid_bus_completion_owner(
                        &completion.owner,
                        &self.bus_dispatch_residency,
                    )))
            && completion.active_cost_units <= max_cost_units
    }

    pub(super) fn reclaim_available(&mut self, _engine: &mut SynthEngine) {
        if self.mode == SourceWorkerMode::Inline {
            return;
        }
        if self.health.status().is_recovering() {
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
                Ok(completion) => self.accept_completion(parity, completion),
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {
                    self.latch_completion_failure(1 << parity);
                    self.in_flight_mask &= !(1 << parity);
                }
            }
        }
    }

    pub(super) fn return_owner(&self, owner: OwnerEnvelope, home: bool, fallback_parity: usize) {
        let Some(mut lease) = self.owner_lease(owner, fallback_parity) else {
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

    pub(super) fn owner_lease(
        &self,
        owner: OwnerEnvelope,
        fallback_parity: usize,
    ) -> Option<OwnerLease> {
        let parity = if owner.parity < SOURCE_WORKER_COUNT {
            owner.parity
        } else {
            fallback_parity
        };
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
