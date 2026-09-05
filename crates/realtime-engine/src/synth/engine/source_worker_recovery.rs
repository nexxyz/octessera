use super::super::super::types::BUS_COUNT;
use super::super::source_worker_carrier_transfer;
use super::super::source_worker_health::SourceWorkerHealth;
use super::super::source_worker_protocol::{SourceWorkerRenderDisposition, WorkerPhase};
use super::super::source_worker_transfer;
use super::super::SynthEngine;
use super::{SourceWorkerRuntime, SOURCE_WORKER_COUNT};
use crossbeam_channel::TryRecvError;
use std::sync::atomic::Ordering;

impl SourceWorkerRuntime {
    pub(super) fn latch_deadline_or_exit(&self) {
        let Some(workers) = self.worker_exited.as_ref() else {
            self.latch_completion_failure(0b11);
            return;
        };
        let exited_mask = workers
            .iter()
            .enumerate()
            .fold(0, |mask, (parity, worker)| {
                if self.in_flight_mask & (1 << parity) != 0 && worker.load(Ordering::Acquire) {
                    mask | (1 << parity)
                } else {
                    mask
                }
            });
        if exited_mask != 0 {
            self.health
                .latch(SourceWorkerHealth::WorkerExited, exited_mask);
        } else if self.in_flight_mask != 0 {
            self.health
                .latch(SourceWorkerHealth::DeadlineMiss, self.in_flight_mask);
        }
    }

    pub fn refresh_recovery(&mut self, engine: &mut SynthEngine) -> bool {
        matches!(
            self.refresh_recovery_disposition(engine),
            SourceWorkerRenderDisposition::Fresh | SourceWorkerRenderDisposition::RecoveredReady
        )
    }

    pub fn refresh_recovery_disposition(
        &mut self,
        engine: &mut SynthEngine,
    ) -> SourceWorkerRenderDisposition {
        if self.mode == super::super::source_worker_protocol::SourceWorkerMode::Inline {
            return SourceWorkerRenderDisposition::Fresh;
        }
        if self.health.status() != SourceWorkerHealth::DeadlineMiss {
            return if self.health.status() == SourceWorkerHealth::Healthy {
                SourceWorkerRenderDisposition::Fresh
            } else {
                SourceWorkerRenderDisposition::Fatal
            };
        }
        self.refresh_recovery_completions();
        if self.health.status() != SourceWorkerHealth::DeadlineMiss {
            return SourceWorkerRenderDisposition::Fatal;
        }
        if self.in_flight_mask != 0 {
            return SourceWorkerRenderDisposition::Recovering;
        }
        if self.completed_mask != 0b11 || !self.home_is_ready() {
            self.latch_completion_failure(0b11);
            return SourceWorkerRenderDisposition::Fatal;
        }
        let Some(phase) = self.expected_phase else {
            self.latch_completion_failure(0b11);
            return SourceWorkerRenderDisposition::Fatal;
        };
        let Some(stamp) = self.expected_stamp else {
            self.latch_completion_failure(0b11);
            return SourceWorkerRenderDisposition::Fatal;
        };
        let recovered = match phase {
            WorkerPhase::Sources => self.recover_source_wave(engine),
            WorkerPhase::Buses => self.recover_bus_wave(engine, stamp.frames),
            #[cfg(feature = "routing-tree-benchmark")]
            WorkerPhase::RoutingTree => self.recover_routing_tree_wave(),
        };
        if !recovered {
            self.latch_completion_failure(0b11);
            return SourceWorkerRenderDisposition::Fatal;
        }
        engine.sample_clock = engine.sample_clock.saturating_add(stamp.frames as u64);
        #[cfg(feature = "routing-tree-benchmark")]
        if self.mode
            == super::super::source_worker_protocol::SourceWorkerMode::RoutingTreePersistent
        {
            self.routing_tree_reprime_pending = true;
        }
        self.clear_recovery_state();
        if self.health.recover() {
            SourceWorkerRenderDisposition::RecoveredReady
        } else {
            SourceWorkerRenderDisposition::Fatal
        }
    }

    fn refresh_recovery_completions(&mut self) {
        for parity in 0..SOURCE_WORKER_COUNT {
            let worker_mask = 1 << parity;
            if self.in_flight_mask & worker_mask == 0 {
                continue;
            }
            let receive_result = self
                .done_rxs
                .as_ref()
                .map(|done_rxs| done_rxs[parity].try_recv());
            match receive_result {
                Some(Ok(completion)) => self.accept_recovery_completion(parity, completion),
                Some(Err(TryRecvError::Empty)) => {
                    if self.worker_exited_for_recovery(parity) {
                        self.latch_recovery_failure(parity, SourceWorkerHealth::WorkerExited);
                    }
                }
                Some(Err(TryRecvError::Disconnected)) | None => {
                    self.latch_recovery_failure(parity, SourceWorkerHealth::CompletionFailed);
                }
            }
        }
    }

    fn accept_recovery_completion(
        &mut self,
        channel_parity: usize,
        completion: super::CompletedEnvelope,
    ) {
        let valid = !completion.worker_exited
            && !completion.transport_failed
            && self.completion_is_valid(channel_parity, &completion);
        if !valid {
            let health = if completion.worker_exited {
                SourceWorkerHealth::WorkerExited
            } else {
                SourceWorkerHealth::CompletionFailed
            };
            self.in_flight_mask &= !(1 << channel_parity);
            self.latch_recovery_failure(channel_parity, health);
            self.return_owner(completion.owner, false, channel_parity);
            return;
        }
        let sequence = completion.stamp.quantum_sequence;
        self.in_flight_mask &= !(1 << channel_parity);
        self.completed_mask |= 1 << channel_parity;
        self.health.record_completion(sequence);
        #[cfg(feature = "source-worker-benchmark-timing")]
        let phase = completion.phase;
        #[cfg(feature = "source-worker-benchmark-timing")]
        self.record_recovery_timing(phase, channel_parity, sequence);
        self.return_owner(completion.owner, true, channel_parity);
    }

    fn worker_exited_for_recovery(&self, parity: usize) -> bool {
        self.worker_exited
            .as_ref()
            .is_some_and(|workers| workers[parity].load(Ordering::Acquire))
    }

    fn latch_recovery_failure(&self, parity: usize, health: SourceWorkerHealth) {
        self.health.latch(health, 1 << parity);
    }

    fn recover_source_wave(&mut self, engine: &mut SynthEngine) -> bool {
        let Some(mut first) = self.lease_home(0) else {
            return false;
        };
        let Some(mut second) = self.lease_home(1) else {
            first.return_fault();
            return false;
        };
        let result = source_worker_carrier_transfer::with_both_source_owners(
            engine,
            &mut first,
            &mut second,
            |engine, _, _| {
                source_worker_transfer::compact_source_pools(engine);
                refresh_active_slots(engine);
                Ok(())
            },
        );
        match result {
            Ok(Ok(())) => {
                first.return_home();
                second.return_home();
                true
            }
            Ok(Err(())) | Err(()) => false,
        }
    }

    fn recover_bus_wave(&mut self, engine: &mut SynthEngine, frames: usize) -> bool {
        let Some(mut first) = self.lease_home(0) else {
            return false;
        };
        let Some(mut second) = self.lease_home(1) else {
            first.return_fault();
            return false;
        };
        let expected_residency = self.bus_dispatch_residency;
        let result = source_worker_carrier_transfer::with_both_source_owners_preserving_carriers(
            engine,
            &mut first,
            &mut second,
            &expected_residency,
            |_, _, carriers| {
                for carrier in carriers.iter_mut().flatten() {
                    if !carrier.prepare(frames) {
                        return Err(());
                    }
                }
                Ok(())
            },
        );
        match result {
            Ok(Ok(())) => {
                first.return_home();
                second.return_home();
                true
            }
            Ok(Err(())) | Err(()) => false,
        }
    }

    #[cfg(feature = "routing-tree-benchmark")]
    fn recover_routing_tree_wave(&mut self) -> bool {
        let Some(mut first) = self.lease_home(0) else {
            return false;
        };
        let Some(mut second) = self.lease_home(1) else {
            first.return_fault();
            return false;
        };
        let valid = first
            .owner
            .as_ref()
            .is_some_and(|owner| owner.routing_tree.is_some())
            && second
                .owner
                .as_ref()
                .is_some_and(|owner| owner.routing_tree.is_some());
        if !valid {
            first.return_fault();
            second.return_fault();
            return false;
        }
        first.return_home();
        second.return_home();
        true
    }

    fn clear_recovery_state(&mut self) {
        self.expected_stamp = None;
        self.expected_phase = None;
        self.in_flight_mask = 0;
        self.completed_mask = 0;
        self.source_load_observations = [None; SOURCE_WORKER_COUNT];
        self.bus_load_observations = [None; SOURCE_WORKER_COUNT];
        self.bus_dispatch_residency = [0; BUS_COUNT];
        self.bus_dispatch_residency_valid = false;
        self.force_fault_mask = 0;
        #[cfg(feature = "source-worker-benchmark-timing")]
        {
            self.dispatch_started_at = None;
            self.coordinator_remainder_started_at = None;
            self.timing_output_sequence = None;
            #[cfg(feature = "routing-tree-benchmark")]
            {
                self.routing_coordinator_remainder_started_at = None;
            }
        }
    }

    #[cfg(feature = "source-worker-benchmark-timing")]
    fn record_recovery_timing(&self, phase: WorkerPhase, parity: usize, sequence: u64) {
        let Some(probe) = self.timing_probe.as_ref() else {
            return;
        };
        let Some(dispatch_started_at) = self.dispatch_started_at else {
            return;
        };
        let elapsed = dispatch_started_at.elapsed();
        if phase == WorkerPhase::Sources {
            probe.record_recovery_completion(sequence, parity, elapsed);
        } else {
            probe.record_recovery_bus_completion(sequence, parity, elapsed);
        }
    }
}

fn refresh_active_slots(engine: &mut SynthEngine) {
    for slot in 0..super::super::super::types::INSTRUMENT_SLOT_COUNT {
        engine.active_synth_slots[slot] = engine
            .synth_voice_pool
            .active_count_for_slot(slot)
            .unwrap_or(0)
            > 0;
        engine.active_sample_slots[slot] = engine
            .sample_voice_pool
            .active_count_for_slot(slot)
            .unwrap_or(0)
            > 0;
    }
}
