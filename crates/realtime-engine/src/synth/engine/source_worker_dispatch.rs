use super::super::super::types::BUS_COUNT;
use super::super::source_lane_renderer::SampleSourceContext;
use super::super::source_worker_health::SourceWorkerHealth;
use super::super::source_worker_lease::OwnerLease;
use super::super::source_worker_protocol::{WorkStamp, WorkerCommand, WorkerPhase};
use super::super::SynthEngine;
use super::SourceWorkerRuntime;
use crossbeam_channel::TrySendError;
use std::time::Instant;

impl SourceWorkerRuntime {
    pub(super) fn dispatch(&mut self, engine: &mut SynthEngine) -> bool {
        let operation_started_at = Instant::now();
        let expected_frames = self.expected_stamp.map_or(0, |stamp| stamp.frames);
        let deadline = operation_started_at + self.rendezvous_deadline(expected_frames);
        self.dispatch_sources(engine, operation_started_at, deadline)
    }

    pub(super) fn dispatch_sources(
        &mut self,
        engine: &mut SynthEngine,
        operation_started_at: Instant,
        deadline: Instant,
    ) -> bool {
        let _ = (operation_started_at, deadline);
        let sequence = self.next_sequence;
        self.next_sequence = self.next_sequence.wrapping_add(1);
        let mut stamp = self.expected_stamp.unwrap_or(WorkStamp {
            runtime_generation: self.runtime_generation,
            render_plan_generation: engine.render_plan.generation,
            quantum_sequence: sequence,
            frames: 0,
            base_sample_clock: engine.sample_clock,
        });
        stamp.quantum_sequence = sequence;
        self.expected_stamp = Some(stamp);
        self.expected_phase = Some(WorkerPhase::Sources);
        self.source_load_observations = std::array::from_fn(|_| None);
        self.bus_load_observations = std::array::from_fn(|_| None);
        self.bus_dispatch_residency_valid = false;
        self.force_fault_mask = 0;
        #[cfg(feature = "source-worker-benchmark-timing")]
        let dispatch_started_at = self.timing_probe.as_ref().map(|_| operation_started_at);
        #[cfg(feature = "source-worker-benchmark-timing")]
        if let Some(probe) = self.timing_probe.as_ref() {
            let _ = probe.begin_sequence(sequence, deadline.duration_since(operation_started_at));
        }
        let Some(mut first) = self.lease_home(0) else {
            self.latch_dispatch_failure(0b11);
            return false;
        };
        let Some(second) = self.lease_home(1) else {
            first.return_fault();
            self.latch_dispatch_failure(0b11);
            return false;
        };
        #[cfg(feature = "source-worker-benchmark-timing")]
        {
            self.dispatch_started_at = dispatch_started_at;
        }
        let first_sent = self.send_work(
            engine,
            stamp,
            first,
            #[cfg(feature = "source-worker-benchmark-timing")]
            dispatch_started_at,
        );
        let second_sent = self.send_work(
            engine,
            stamp,
            second,
            #[cfg(feature = "source-worker-benchmark-timing")]
            dispatch_started_at,
        );
        #[cfg(feature = "source-worker-benchmark-timing")]
        if let Some(probe) = self.timing_probe.as_ref() {
            probe.record_dispatch(sequence, self.in_flight_mask);
        }
        first_sent && second_sent && self.health.status() == SourceWorkerHealth::Healthy
    }

    pub(super) fn dispatch_buses(&mut self, engine: &SynthEngine, stamp: WorkStamp) -> bool {
        if self.mode != super::super::source_worker_protocol::SourceWorkerMode::Persistent
            || self.health.status() != SourceWorkerHealth::Healthy
            || self.in_flight_mask != 0
            || self.completed_mask != 0
            || !self.home_is_ready()
            || self.expected_stamp != Some(stamp)
        {
            self.latch_dispatch_failure(0b11);
            return false;
        }
        self.expected_phase = Some(WorkerPhase::Buses);
        #[cfg(feature = "source-worker-benchmark-timing")]
        if let Some(probe) = self.timing_probe.as_ref() {
            probe.record_bus_dispatch(stamp.quantum_sequence);
        }
        let Some(mut first) = self.lease_home(0) else {
            self.latch_dispatch_failure(0b11);
            return false;
        };
        let Some(second) = self.lease_home(1) else {
            first.return_fault();
            self.latch_dispatch_failure(0b11);
            return false;
        };
        let Some(residency) = bus_residency(&first, &second) else {
            first.return_fault();
            let mut second = second;
            second.return_fault();
            self.latch_dispatch_failure(0b11);
            return false;
        };
        self.bus_dispatch_residency = residency;
        self.bus_dispatch_residency_valid = true;
        if !self.send_bus_work(engine, stamp, first) {
            let mut second = second;
            second.return_fault();
            self.bus_dispatch_residency_valid = false;
            return false;
        }
        let second = second;
        if self.send_bus_work(engine, stamp, second) {
            return true;
        }
        self.force_fault_mask |= self.in_flight_mask;
        self.health.latch(SourceWorkerHealth::DispatchFailed, 0b11);
        self.bus_dispatch_residency_valid = false;
        false
    }

    fn send_work(
        &mut self,
        engine: &SynthEngine,
        stamp: WorkStamp,
        mut lease: OwnerLease,
        #[cfg(feature = "source-worker-benchmark-timing")] dispatch_started_at: Option<Instant>,
    ) -> bool {
        let Some(owner) = lease.take_owner() else {
            self.latch_dispatch_failure(1 << lease.parity);
            return false;
        };
        let work = WorkerCommand::Sources {
            owner,
            stamp,
            synth_context: engine.synth_source_context(),
            sample_context: SampleSourceContext {
                sample_rate: engine.sample_rate,
            },
            #[cfg(feature = "source-worker-benchmark-timing")]
            dispatch_started_at,
            #[cfg(feature = "source-worker-benchmark-timing")]
            timing_probe: self.timing_probe.clone(),
        };
        let parity = lease.parity;
        let Some(work_tx) = self.work_txs.as_ref().map(|work_txs| &work_txs[parity]) else {
            let WorkerCommand::Sources { owner, .. } = work else {
                unreachable!("source dispatch only creates source commands");
            };
            lease.restore_owner(owner);
            lease.return_home();
            self.latch_dispatch_failure(1 << parity);
            return false;
        };
        match work_tx.try_send(work) {
            Ok(()) => {
                self.in_flight_mask |= 1 << parity;
                true
            }
            Err(
                TrySendError::Full(WorkerCommand::Sources { owner, .. })
                | TrySendError::Disconnected(WorkerCommand::Sources { owner, .. }),
            ) => {
                lease.restore_owner(owner);
                lease.return_home();
                self.latch_dispatch_failure(1 << parity);
                false
            }
            Err(
                TrySendError::Full(WorkerCommand::Buses { .. })
                | TrySendError::Disconnected(WorkerCommand::Buses { .. }),
            ) => {
                self.latch_dispatch_failure(1 << parity);
                false
            }
            #[cfg(feature = "routing-tree-benchmark")]
            Err(
                TrySendError::Full(WorkerCommand::RoutingTree { .. })
                | TrySendError::Disconnected(WorkerCommand::RoutingTree { .. }),
            ) => unreachable!("source dispatch only creates source commands"),
        }
    }

    fn send_bus_work(
        &mut self,
        engine: &SynthEngine,
        stamp: WorkStamp,
        mut lease: OwnerLease,
    ) -> bool {
        let Some(owner) = lease.take_owner() else {
            self.latch_dispatch_failure(1 << lease.parity);
            return false;
        };
        let parity = lease.parity;
        let command = WorkerCommand::Buses {
            owner,
            stamp,
            frames: stamp.frames,
            sample_rate: engine.sample_rate,
            bus_idle_threshold: engine.dsp_config.bus_idle_threshold,
            fx_activity_hold_frames: engine.fx_activity_hold_frames,
            #[cfg(feature = "source-worker-benchmark-timing")]
            dispatch_started_at: self.dispatch_started_at,
            #[cfg(feature = "source-worker-benchmark-timing")]
            timing_probe: self.timing_probe.clone(),
        };
        let Some(work_tx) = self.work_txs.as_ref().map(|work_txs| &work_txs[parity]) else {
            let WorkerCommand::Buses { owner, .. } = command else {
                unreachable!("bus dispatch only creates bus commands");
            };
            lease.restore_owner(owner);
            self.latch_dispatch_failure(1 << parity);
            lease.return_fault();
            return false;
        };
        match work_tx.try_send(command) {
            Ok(()) => {
                self.in_flight_mask |= 1 << parity;
                true
            }
            Err(
                TrySendError::Full(WorkerCommand::Buses { owner, .. })
                | TrySendError::Disconnected(WorkerCommand::Buses { owner, .. }),
            ) => {
                lease.restore_owner(owner);
                self.latch_dispatch_failure(1 << parity);
                lease.return_fault();
                false
            }
            Err(
                TrySendError::Full(WorkerCommand::Sources { .. })
                | TrySendError::Disconnected(WorkerCommand::Sources { .. }),
            ) => unreachable!("bus dispatch only creates bus commands"),
            #[cfg(feature = "routing-tree-benchmark")]
            Err(
                TrySendError::Full(WorkerCommand::RoutingTree { .. })
                | TrySendError::Disconnected(WorkerCommand::RoutingTree { .. }),
            ) => unreachable!("bus dispatch only creates bus commands"),
        }
    }
}

fn bus_residency(first: &OwnerLease, second: &OwnerLease) -> Option<[u8; BUS_COUNT]> {
    let first_owner = first.owner.as_ref()?;
    let second_owner = second.owner.as_ref()?;
    let mut residency = [0; BUS_COUNT];
    for (logical_bus_id, expected_parity) in residency.iter_mut().enumerate() {
        let first_carrier = first_owner.bus_carriers[logical_bus_id].as_ref();
        let second_carrier = second_owner.bus_carriers[logical_bus_id].as_ref();
        if usize::from(first_carrier.is_some()) + usize::from(second_carrier.is_some()) != 1 {
            return None;
        }
        let (carrier, parity) = first_carrier
            .map(|carrier| (carrier, first.parity))
            .or_else(|| second_carrier.map(|carrier| (carrier, second.parity)))?;
        if carrier.logical_bus_id != logical_bus_id || !carrier.within_worker_capacity() {
            return None;
        }
        let assigned_worker = carrier
            .owner
            .as_ref()
            .and_then(|owner| owner.assigned_worker);
        if assigned_worker.is_some_and(|assigned_worker| assigned_worker != parity)
            || assigned_worker.is_none() && parity != logical_bus_id % 2
        {
            return None;
        }
        *expected_parity = parity as u8;
    }
    Some(residency)
}
