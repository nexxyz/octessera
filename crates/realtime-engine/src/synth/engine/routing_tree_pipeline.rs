use super::super::routing_tree_worker::RoutingTreeWorkerContext;
use super::super::source_worker_carrier_transfer;
use super::super::source_worker_health::SourceWorkerHealth;
use super::super::source_worker_protocol::{
    SourceWorkerRenderDisposition, WorkerCommand, WorkerPhase,
};
use super::super::{SynthEngine, BLOCK_SLOT_SCRATCH_FRAMES};
use super::{OwnerLease, SourceWorkerRuntime, SOURCE_WORKER_COUNT};
use crossbeam_channel::TrySendError;

impl SourceWorkerRuntime {
    pub(in crate::synth::engine) fn prime_routing_tree(
        &mut self,
        engine: &mut SynthEngine,
        frames: usize,
    ) -> bool {
        if self.mode
            != super::super::source_worker_protocol::SourceWorkerMode::RoutingTreePersistent
            || !self.dispatch_routing_tree(engine, frames, engine.sample_clock)
        {
            return false;
        }
        let primed = self
            .collect_routing_tree_output_with_deadline(
                engine,
                true,
                std::time::Instant::now() + std::time::Duration::from_secs(1),
            )
            .is_some();
        self.clear_routing_absolute_deadline();
        primed
    }

    pub(in crate::synth::engine) fn render_routing_tree_persistent_block(
        &mut self,
        engine: &mut SynthEngine,
        frames: usize,
        left: &mut [f32],
        right: &mut [f32],
        apply_controls: impl FnOnce(&mut SynthEngine) -> Result<(), ()>,
    ) -> SourceWorkerRenderDisposition {
        if self.mode
            != super::super::source_worker_protocol::SourceWorkerMode::RoutingTreePersistent
        {
            return SourceWorkerRenderDisposition::Fatal;
        }
        #[cfg(feature = "source-worker-benchmark-timing")]
        {
            self.timing_output_sequence = None;
        }
        if frames == 0
            || frames > self.lookahead_frames
            || frames > BLOCK_SLOT_SCRATCH_FRAMES
            || left.len() < frames
            || right.len() < frames
        {
            self.latch_invalid_block();
            return SourceWorkerRenderDisposition::Fatal;
        }
        if self.health.status() != SourceWorkerHealth::Healthy {
            self.reclaim_available(engine);
            return if self.health.status().is_recovering() {
                SourceWorkerRenderDisposition::Recovering
            } else {
                self.routing_tree_failure_disposition()
            };
        }
        if (self.in_flight_mask != 0 || self.completed_mask != 0)
            && self.collect_routing_tree_output(engine, true).is_none()
        {
            return self.routing_tree_failure_disposition();
        }
        let recovered = self.routing_tree_reprime_pending;
        self.routing_tree_reprime_pending = false;
        let dispatch_sample_clock = if recovered {
            engine.sample_clock
        } else {
            engine.sample_clock.saturating_add(frames as u64)
        };
        let Some(()) =
            self.with_routing_tree_controls_ready(engine, dispatch_sample_clock, apply_controls)
        else {
            self.latch_completion_failure(0b11);
            return SourceWorkerRenderDisposition::Fatal;
        };
        if !self.routing_output_ready {
            if !self.dispatch_routing_tree(engine, frames, dispatch_sample_clock) {
                return self.routing_tree_failure_disposition();
            }
            return SourceWorkerRenderDisposition::Recovering;
        }
        if !engine.routing_tree_assignment_is_valid() {
            self.latch_invalid_block();
            return SourceWorkerRenderDisposition::Fatal;
        }
        if !self.dispatch_routing_tree(engine, frames, dispatch_sample_clock) {
            self.reclaim_available(engine);
            return self.routing_tree_failure_disposition();
        }
        let outputs = self
            .routing_output_spares
            .as_ref()
            .expect("routing-tree output spares");
        let Some(_output_stamp) = self.routing_output_stamp else {
            self.latch_completion_failure(0b11);
            return SourceWorkerRenderDisposition::Fatal;
        };
        #[cfg(feature = "source-worker-benchmark-timing")]
        let reduction_started_at = self
            .timing_probe
            .as_ref()
            .map(|_| std::time::Instant::now());
        for frame in 0..frames {
            left[frame] = outputs[0].left[frame] + outputs[1].left[frame];
            right[frame] = outputs[0].right[frame] + outputs[1].right[frame];
            engine.block_slot_scratch.source_active[frame] =
                outputs[0].source_active[frame] || outputs[1].source_active[frame];
            engine.block_slot_scratch.bus_active[frame] =
                outputs[0].bus_active[frame] || outputs[1].bus_active[frame];
        }
        #[cfg(feature = "source-worker-benchmark-timing")]
        if let Some(started_at) = reduction_started_at {
            if let Some(probe) = self.timing_probe.as_ref() {
                probe.record_reduction(_output_stamp.quantum_sequence, started_at.elapsed());
            }
        }
        #[cfg(feature = "source-worker-benchmark-timing")]
        let coordinator_remainder_started_at = std::time::Instant::now();
        engine.set_routing_tree_profile([&outputs[0], &outputs[1]]);
        #[cfg(any(test, feature = "test-support"))]
        if let Some(probe) = self.routing_tree_probe.as_ref() {
            probe.record_coordinator(
                _output_stamp.quantum_sequence,
                _output_stamp.base_sample_clock,
            );
        }
        #[cfg(feature = "source-worker-benchmark-timing")]
        {
            self.routing_coordinator_remainder_started_at = Some((
                _output_stamp.quantum_sequence,
                coordinator_remainder_started_at,
            ));
        }
        engine.finish_persistent_block(frames, left, right);
        #[cfg(feature = "source-worker-benchmark-timing")]
        {
            self.record_output_sequence(_output_stamp.quantum_sequence);
        }
        self.routing_output_ready = false;
        self.routing_output_stamp = None;
        SourceWorkerRenderDisposition::Fresh
    }

    fn dispatch_routing_tree(
        &mut self,
        engine: &SynthEngine,
        frames: usize,
        base_sample_clock: u64,
    ) -> bool {
        self.clear_routing_absolute_deadline();
        if self.in_flight_mask != 0
            || self.completed_mask != 0
            || !self.home_is_ready()
            || !engine.routing_tree_assignment_is_valid()
        {
            self.latch_dispatch_failure(0b11);
            return false;
        }
        let Some(assignment) = engine.routing_tree_assignment() else {
            self.latch_dispatch_failure(0b11);
            return false;
        };
        let Some(context) = RoutingTreeWorkerContext::from_engine(engine, &assignment) else {
            self.latch_invalid_block();
            return false;
        };
        let sequence = self.next_sequence;
        self.next_sequence = self.next_sequence.wrapping_add(1);
        let dispatch_started_at = std::time::Instant::now();
        let stamp = super::super::source_worker_protocol::WorkStamp {
            runtime_generation: self.runtime_generation,
            render_plan_generation: assignment.plan.generation,
            quantum_sequence: sequence,
            frames,
            base_sample_clock,
        };
        self.expected_stamp = Some(stamp);
        self.expected_phase = Some(WorkerPhase::RoutingTree);
        self.source_load_observations = [None; SOURCE_WORKER_COUNT];
        self.force_fault_mask = 0;
        #[cfg(feature = "source-worker-benchmark-timing")]
        {
            self.dispatch_started_at = self.timing_probe.as_ref().map(|_| dispatch_started_at);
            if let Some(probe) = self.timing_probe.as_ref() {
                let _ = probe.begin_sequence(sequence, self.routing_tree_deadline_duration(frames));
                probe.record_dispatch(sequence, 0b11);
            }
        }
        let Some(mut first) = self.lease_home(0) else {
            self.latch_dispatch_failure(0b11);
            return false;
        };
        let Some(mut second) = self.lease_home(1) else {
            first.return_fault();
            self.latch_dispatch_failure(0b11);
            return false;
        };
        reassert_routing_tree_bus_assignments(&mut first, &mut second, &context);
        if !routing_owner_pair_valid(&first, &second, &context) {
            first.return_fault();
            second.return_fault();
            self.latch_invalid_block();
            return false;
        }
        let first_sent = self.send_routing_tree_work(&mut first, stamp, context);
        let second_sent = self.send_routing_tree_work(&mut { second }, stamp, context);
        #[cfg(any(test, feature = "test-support"))]
        if first_sent && second_sent {
            if let Some(probe) = self.routing_tree_probe.as_ref() {
                probe.record_dispatch(sequence, stamp.base_sample_clock);
            }
        }
        let sent = first_sent && second_sent && self.health.status() == SourceWorkerHealth::Healthy;
        if sent {
            self.set_routing_absolute_deadline(dispatch_started_at, frames);
        } else {
            self.clear_routing_absolute_deadline();
        }
        sent
    }

    fn send_routing_tree_work(
        &mut self,
        lease: &mut OwnerLease,
        stamp: super::super::source_worker_protocol::WorkStamp,
        context: RoutingTreeWorkerContext,
    ) -> bool {
        let Some(owner) = lease.take_owner() else {
            self.latch_dispatch_failure(1 << lease.parity);
            return false;
        };
        let command = WorkerCommand::RoutingTree {
            stamp,
            owner,
            context,
            #[cfg(feature = "source-worker-benchmark-timing")]
            dispatch_started_at: self.dispatch_started_at,
            #[cfg(feature = "source-worker-benchmark-timing")]
            timing_probe: self.timing_probe.clone(),
        };
        let parity = lease.parity;
        let Some(work_tx) = self.work_txs.as_ref().map(|txs| &txs[parity]) else {
            let WorkerCommand::RoutingTree { owner, .. } = command else {
                unreachable!("routing-tree dispatch only creates routing commands");
            };
            lease.restore_owner(owner);
            lease.return_fault();
            self.latch_dispatch_failure(1 << parity);
            return false;
        };
        match work_tx.try_send(command) {
            Ok(()) => {
                self.in_flight_mask |= 1 << parity;
                true
            }
            Err(TrySendError::Full(WorkerCommand::RoutingTree { owner, .. }))
            | Err(TrySendError::Disconnected(WorkerCommand::RoutingTree { owner, .. })) => {
                lease.restore_owner(owner);
                lease.return_fault();
                self.latch_dispatch_failure(1 << parity);
                false
            }
            Err(_) => unreachable!("routing-tree dispatch only creates routing commands"),
        }
    }

    pub(super) fn collect_routing_tree_output(
        &mut self,
        engine: &mut SynthEngine,
        wait: bool,
    ) -> Option<()> {
        let Some(deadline) = self.routing_absolute_deadline.take() else {
            self.latch_completion_failure(0b11);
            return None;
        };
        self.collect_routing_tree_output_with_deadline(engine, wait, deadline)
    }

    fn collect_routing_tree_output_with_deadline(
        &mut self,
        engine: &mut SynthEngine,
        wait: bool,
        deadline: std::time::Instant,
    ) -> Option<()> {
        self.clear_routing_absolute_deadline();
        let stamp = self.expected_stamp?;
        self.collect_wave_with_deadline(engine, wait, WorkerPhase::RoutingTree, deadline, true)?;
        if self.completed_mask != 0b11 || !self.home_is_ready() {
            self.latch_completion_failure(0b11);
            return None;
        }
        let mut first = self.lease_home(0)?;
        let mut second = self.lease_home(1)?;
        if !routing_output_is_valid(first.owner.as_ref()?)
            || !routing_output_is_valid(second.owner.as_ref()?)
        {
            first.return_fault();
            second.return_fault();
            self.latch_completion_failure(0b11);
            return None;
        }
        std::mem::swap(
            &mut first.owner.as_mut()?.routing_tree.as_mut()?.output,
            &mut self
                .routing_output_spares
                .as_mut()
                .expect("routing-tree output spares")[0],
        );
        std::mem::swap(
            &mut second.owner.as_mut()?.routing_tree.as_mut()?.output,
            &mut self
                .routing_output_spares
                .as_mut()
                .expect("routing-tree output spares")[1],
        );
        first.return_home();
        second.return_home();
        self.completed_mask = 0;
        self.routing_output_stamp = Some(stamp);
        self.routing_output_ready = true;
        if let (Some(load), [Some(first), Some(second)]) =
            (self.load.as_mut(), self.source_load_observations)
        {
            if !load.observe_pair([first, second]) {
                self.latch_completion_failure(0b11);
                self.source_load_observations = [None; SOURCE_WORKER_COUNT];
                return None;
            }
            if let Some(utilization_ppm) = load.snapshot().utilization_ppm {
                engine.observe_worker_utilization(utilization_ppm, stamp.frames);
            }
        }
        self.source_load_observations = [None; SOURCE_WORKER_COUNT];
        Some(())
    }

    fn with_routing_tree_controls_ready<R>(
        &mut self,
        engine: &mut SynthEngine,
        effective_sample_clock: u64,
        apply: impl FnOnce(&mut SynthEngine) -> Result<R, ()>,
    ) -> Option<R> {
        if self.in_flight_mask != 0 || self.completed_mask != 0 || !self.home_is_ready() {
            return None;
        }
        let mut first = self.lease_home(0)?;
        let Some(mut second) = self.lease_home(1) else {
            first.return_fault();
            return None;
        };
        let Some(assignment) = engine.routing_tree_assignment() else {
            first.return_fault();
            second.return_fault();
            return None;
        };
        let Some(context) = RoutingTreeWorkerContext::from_engine(engine, &assignment) else {
            first.return_fault();
            second.return_fault();
            return None;
        };
        reassert_routing_tree_bus_assignments(&mut first, &mut second, &context);
        let load = self.load_snapshot();
        let result =
            source_worker_carrier_transfer::with_both_source_owners_for_routing_tree_controls(
                engine,
                &mut first,
                &mut second,
                |engine, _, _| {
                    engine.with_source_worker_load(load, |engine| {
                        engine.with_routing_tree_source_event_sample_clock(
                            effective_sample_clock,
                            apply,
                        )
                    })
                },
            );
        match result {
            Ok(result) => {
                first.return_home();
                second.return_home();
                Some(result)
            }
            Err(()) => {
                self.latch_completion_failure(0b11);
                None
            }
        }
    }

    fn routing_tree_failure_disposition(&mut self) -> SourceWorkerRenderDisposition {
        self.clear_routing_absolute_deadline();
        if self.health.status() == SourceWorkerHealth::DeadlineMiss {
            SourceWorkerRenderDisposition::NewlyMissed
        } else {
            SourceWorkerRenderDisposition::Fatal
        }
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn set_routing_tree_probe_for_test(
        &mut self,
        probe: std::sync::Arc<super::super::source_worker::RoutingTreePipelineProbe>,
    ) {
        self.routing_tree_probe = Some(probe);
    }

    #[cfg(test)]
    pub(crate) fn dispatch_routing_tree_for_test(
        &mut self,
        engine: &SynthEngine,
        frames: usize,
        base_sample_clock: u64,
    ) -> bool {
        self.dispatch_routing_tree(engine, frames, base_sample_clock)
    }
}

fn routing_owner_pair_valid(
    first: &OwnerLease,
    second: &OwnerLease,
    context: &RoutingTreeWorkerContext,
) -> bool {
    let Some(first) = first.owner.as_ref() else {
        return false;
    };
    let Some(second) = second.owner.as_ref() else {
        return false;
    };
    [first, second].iter().enumerate().all(|(parity, owner)| {
        owner.parity == parity
            && owner.routing_tree.is_some()
            && owner.partitions.synth.parity() == parity
            && owner.partitions.sample.parity() == parity
    }) && (0..context.bus_count).all(|bus| {
        let first_carrier = first.bus_carriers[bus].as_ref();
        let second_carrier = second.bus_carriers[bus].as_ref();
        let Some((carrier, parity)) = first_carrier
            .map(|carrier| (carrier, 0))
            .or_else(|| second_carrier.map(|carrier| (carrier, 1)))
        else {
            return false;
        };
        first_carrier.is_some() != second_carrier.is_some()
            && parity == usize::from(context.bus_worker[bus])
            && carrier.logical_bus_id == bus
            && carrier.routing_tree_spread_state.is_some()
            && carrier
                .owner
                .as_ref()
                .is_some_and(|owner| owner.assigned_worker == Some(parity))
    })
}

fn reassert_routing_tree_bus_assignments(
    first: &mut OwnerLease,
    second: &mut OwnerLease,
    context: &RoutingTreeWorkerContext,
) {
    for bus in 0..context.bus_count {
        let worker = usize::from(context.bus_worker[bus]);
        if worker >= SOURCE_WORKER_COUNT {
            continue;
        }
        let carrier = first
            .owner
            .as_mut()
            .and_then(|owner| owner.bus_carriers[bus].as_mut())
            .or_else(|| {
                second
                    .owner
                    .as_mut()
                    .and_then(|owner| owner.bus_carriers[bus].as_mut())
            });
        if let Some(chain) = carrier.and_then(|carrier| carrier.owner.as_mut()) {
            chain.assigned_worker = Some(worker);
        }
    }
}

fn routing_output_is_valid(owner: &super::super::source_worker_owner::OwnerEnvelope) -> bool {
    owner.routing_tree.as_ref().is_some_and(|routing| {
        routing.output.left.len() >= BLOCK_SLOT_SCRATCH_FRAMES
            && routing.output.right.len() >= BLOCK_SLOT_SCRATCH_FRAMES
    })
}
