use super::super::source_worker_lifecycle::{OwnerEnvelope, SourceWorkerOwnerIdentity};
use super::super::source_worker_protocol::{WorkStamp, WorkerPhase};
use super::*;
use crossbeam_channel::bounded;

impl SourceWorkerRuntime {
    pub(crate) fn scratch_shape_for_test(&self) -> [(usize, usize); SOURCE_WORKER_COUNT] {
        std::array::from_fn(|parity| {
            let owner = self.take_home_owner_for_test(parity).expect("worker home");
            let shape = (
                owner.scratch.synth.samples.len(),
                owner.scratch.synth.samples[0].len(),
            );
            self.return_home_owner_for_test(owner);
            shape
        })
    }

    pub(crate) fn dispatch_only_for_test(
        &mut self,
        engine: &mut SynthEngine,
        frames: usize,
    ) -> bool {
        self.expected_stamp = Some(WorkStamp {
            runtime_generation: self.runtime_generation,
            render_plan_generation: engine.render_plan.generation,
            quantum_sequence: self.next_sequence,
            frames,
            base_sample_clock: engine.sample_clock,
        });
        self.dispatch(engine)
    }

    pub(crate) fn expected_stamp_for_test(&self) -> Option<WorkStamp> {
        self.expected_stamp
    }

    pub(crate) fn stamp_for_test(&self, engine: &SynthEngine, frames: usize) -> WorkStamp {
        WorkStamp {
            runtime_generation: self.runtime_generation,
            render_plan_generation: engine.render_plan.generation,
            quantum_sequence: self.next_sequence,
            frames,
            base_sample_clock: engine.sample_clock,
        }
    }

    pub(crate) fn dispatch_buses_for_test(&mut self, stamp: WorkStamp, parity: usize) -> bool {
        let Some(work_tx) = self
            .work_txs
            .as_ref()
            .and_then(|work_txs| work_txs.get(parity))
        else {
            return false;
        };
        let Some(owner) = self
            .home_rxs
            .as_ref()
            .and_then(|home_rxs| home_rxs.get(parity))
            .and_then(|home_rx| home_rx.try_recv().ok())
        else {
            return false;
        };
        let command = WorkerCommand::RenderBuses { stamp, owner };
        match work_tx.try_send(command) {
            Ok(()) => {
                self.expected_stamp = Some(stamp);
                self.in_flight_mask |= 1 << parity;
                true
            }
            Err(error) => {
                let WorkerCommand::RenderBuses { owner, .. } = error.into_inner() else {
                    unreachable!("test bus dispatch only creates bus commands");
                };
                self.home_txs
                    .as_ref()
                    .expect("source worker lifecycle is active")[parity]
                    .try_send(owner)
                    .expect("home owner mailbox");
                false
            }
        }
    }

    pub(crate) fn completion_evidence_for_test(
        &mut self,
        parity: usize,
    ) -> Option<(
        SourceWorkerOwnerIdentity,
        WorkerPhase,
        WorkStamp,
        bool,
        bool,
        bool,
    )> {
        let receiver = &self.done_rxs.as_ref().expect("persistent source workers")[parity];
        let completion = receiver.try_recv().ok()?;
        let evidence = (
            owner_identity(&completion.owner),
            completion.phase,
            completion.stamp,
            completion.render_ok,
            completion.worker_exited,
            completion.transport_failed,
        );
        self.done_txs
            .as_ref()
            .expect("source worker lifecycle is active")[parity]
            .try_send(completion)
            .ok()?;
        Some(evidence)
    }

    pub(crate) fn collect_for_test(&mut self, engine: &mut SynthEngine) -> bool {
        self.collect(engine, false)
    }

    pub(crate) fn in_flight_mask_for_test(&self) -> u8 {
        self.in_flight_mask
    }

    pub(crate) fn workers_exited_for_test(&self) -> [bool; SOURCE_WORKER_COUNT] {
        self.worker_exited
            .as_ref()
            .map(|workers| std::array::from_fn(|parity| workers[parity].load(Ordering::Acquire)))
            .unwrap_or([false; SOURCE_WORKER_COUNT])
    }

    pub(crate) fn partitions_home_for_test(&self) -> bool {
        self.home_is_ready()
    }

    pub(crate) fn home_owner_identities_for_test(&self) -> [SourceWorkerOwnerIdentity; 2] {
        std::array::from_fn(|parity| {
            let owner = self.take_home_owner_for_test(parity).expect("worker home");
            let identity = owner_identity(&owner);
            self.return_home_owner_for_test(owner);
            identity
        })
    }

    pub(crate) fn home_owner_identity_for_test(
        &self,
        parity: usize,
    ) -> Option<SourceWorkerOwnerIdentity> {
        let owner = self.take_home_owner_for_test(parity)?;
        let identity = owner_identity(&owner);
        self.return_home_owner_for_test(owner);
        Some(identity)
    }

    pub(crate) fn home_bus_carrier_ids_for_test(
        &self,
    ) -> [[Option<usize>; super::super::super::types::BUS_COUNT]; SOURCE_WORKER_COUNT] {
        std::array::from_fn(|parity| {
            let owner = self.take_home_owner_for_test(parity).expect("worker home");
            let ids = owner
                .bus_carriers
                .each_ref()
                .map(|carrier| carrier.as_ref().map(|carrier| carrier.logical_bus_id));
            self.return_home_owner_for_test(owner);
            ids
        })
    }

    pub(crate) fn home_bus_carrier_assignments_for_test(
        &self,
    ) -> [[Option<Option<usize>>; super::super::super::types::BUS_COUNT]; SOURCE_WORKER_COUNT] {
        std::array::from_fn(|parity| {
            let owner = self.take_home_owner_for_test(parity).expect("worker home");
            let assignments = owner.bus_carriers.each_ref().map(|carrier| {
                carrier.as_ref().map(|carrier| {
                    carrier
                        .owner
                        .as_ref()
                        .and_then(|owner| owner.assigned_worker)
                })
            });
            self.return_home_owner_for_test(owner);
            assignments
        })
    }

    pub(crate) fn bus_carrier_scratch_shape_for_test(
        &self,
    ) -> Option<(
        usize,
        [usize; super::super::super::types::BUS_SLOTS_PER_BUS],
        usize,
        usize,
    )> {
        let owner = self.take_home_owner_for_test(0)?;
        let carrier = owner.bus_carriers.iter().flatten().next()?;
        let shape = (
            carrier.scratch.input.len(),
            carrier.scratch.resolved_duck.each_ref().map(Vec::len),
            carrier.scratch.mono_output.len(),
            carrier.scratch.auto_pan_pos.len(),
        );
        self.return_home_owner_for_test(owner);
        Some(shape)
    }

    pub(crate) fn bus_carrier_scratch_addresses_for_test(
        &self,
    ) -> [Option<usize>; super::super::super::types::BUS_COUNT] {
        let owner = self.take_home_owner_for_test(0).expect("worker home");
        let addresses = owner.bus_carriers.each_ref().map(|carrier| {
            carrier
                .as_ref()
                .map(|carrier| carrier.scratch.input.as_ptr() as usize)
        });
        self.return_home_owner_for_test(owner);
        addresses
    }

    pub(crate) fn bus_carrier_scratch_bytes_for_test(&self) -> Option<usize> {
        let owner = self.take_home_owner_for_test(0)?;
        let carrier = owner.bus_carriers.iter().flatten().next()?;
        let bytes = carrier.scratch.input.len() * std::mem::size_of::<f32>()
            + carrier
                .scratch
                .resolved_duck
                .iter()
                .map(|buffer| buffer.len() * std::mem::size_of::<f32>())
                .sum::<usize>()
            + carrier.scratch.mono_output.len() * std::mem::size_of::<f32>()
            + carrier.scratch.auto_pan_pos.len() * std::mem::size_of::<f32>();
        self.return_home_owner_for_test(owner);
        Some(bytes)
    }

    pub(crate) fn home_sample_buffer_addresses_for_test(&self) -> [Option<usize>; 2] {
        std::array::from_fn(|parity| {
            let owner = self.take_home_owner_for_test(parity)?;
            let identity = owner
                .partitions
                .sample
                .active_sample_buffer_address_for_test();
            self.return_home_owner_for_test(owner);
            identity
        })
    }

    pub(crate) fn collect_wait_for_test(&mut self, engine: &mut SynthEngine) -> bool {
        self.collect(engine, true)
    }

    pub(crate) fn disconnect_work_for_test(&mut self, parity: usize) {
        let (work_tx, work_rx) = bounded(0);
        drop(work_rx);
        self.work_txs.as_mut().expect("persistent source workers")[parity] = work_tx;
    }

    pub(crate) fn completion_ready_for_test(&self, parity: usize) -> bool {
        !self.done_rxs.as_ref().expect("persistent source workers")[parity].is_empty()
    }

    pub(crate) fn rewrite_completion_sequence_for_test(&mut self, parity: usize) -> bool {
        self.rewrite_completion_stamp_for_test(parity, |stamp| {
            stamp.quantum_sequence = stamp.quantum_sequence.wrapping_add(1);
        })
    }

    pub(crate) fn rewrite_completion_stamp_for_test(
        &mut self,
        parity: usize,
        mutate: impl FnOnce(&mut WorkStamp),
    ) -> bool {
        let receiver = &self.done_rxs.as_ref().expect("persistent source workers")[parity];
        let Ok(mut completion) = receiver.try_recv() else {
            return false;
        };
        mutate(&mut completion.stamp);
        self.done_txs
            .as_ref()
            .expect("source worker lifecycle is active")[parity]
            .try_send(completion)
            .is_ok()
    }

    pub(crate) fn rewrite_completion_phase_for_test(&mut self, parity: usize) -> bool {
        self.rewrite_completion(parity, |completion| {
            completion.phase = WorkerPhase::Buses;
        })
    }

    pub(crate) fn rewrite_completion_owner_generation_for_test(
        &mut self,
        parity: usize,
        generation: u64,
    ) -> bool {
        self.rewrite_completion(parity, |completion| {
            completion.owner.runtime_generation = generation;
        })
    }

    pub(crate) fn rewrite_completion_owner_parity_for_test(
        &mut self,
        parity: usize,
        owner_parity: usize,
    ) -> bool {
        self.rewrite_completion(parity, |completion| {
            completion.owner.parity = owner_parity;
        })
    }

    pub(crate) fn rewrite_completion_render_ok_for_test(
        &mut self,
        parity: usize,
        render_ok: bool,
    ) -> bool {
        self.rewrite_completion(parity, |completion| {
            completion.render_ok = render_ok;
        })
    }

    fn rewrite_completion(
        &mut self,
        parity: usize,
        mutate: impl FnOnce(&mut CompletedEnvelope),
    ) -> bool {
        let receiver = &self.done_rxs.as_ref().expect("persistent source workers")[parity];
        let Ok(mut completion) = receiver.try_recv() else {
            return false;
        };
        mutate(&mut completion);
        self.done_txs
            .as_ref()
            .expect("source worker lifecycle is active")[parity]
            .try_send(completion)
            .is_ok()
    }

    pub(crate) fn rewrite_completion_measurement_for_test(
        &mut self,
        parity: usize,
        dsp_duration_ns: u64,
        active_cost_units: u16,
    ) -> bool {
        let receiver = &self.done_rxs.as_ref().expect("persistent source workers")[parity];
        let Ok(mut completion) = receiver.try_recv() else {
            return false;
        };
        completion.dsp_duration_ns = dsp_duration_ns;
        completion.active_cost_units = active_cost_units;
        self.done_txs
            .as_ref()
            .expect("source worker lifecycle is active")[parity]
            .try_send(completion)
            .is_ok()
    }

    pub(crate) fn completion_measurement_for_test(&mut self, parity: usize) -> Option<(u64, u16)> {
        let receiver = &self.done_rxs.as_ref().expect("persistent source workers")[parity];
        let completion = receiver.try_recv().ok()?;
        let measurement = (completion.dsp_duration_ns, completion.active_cost_units);
        self.done_txs
            .as_ref()
            .expect("source worker lifecycle is active")[parity]
            .try_send(completion)
            .ok()?;
        Some(measurement)
    }

    fn take_home_owner_for_test(&self, parity: usize) -> Option<OwnerEnvelope> {
        self.home_rxs.as_ref()?.get(parity)?.try_recv().ok()
    }

    fn return_home_owner_for_test(&self, owner: OwnerEnvelope) {
        let parity = owner.parity;
        self.home_txs.as_ref().expect("persistent source workers")[parity]
            .try_send(owner)
            .expect("home mailbox");
    }
}

fn owner_identity(owner: &OwnerEnvelope) -> SourceWorkerOwnerIdentity {
    (
        owner.parity,
        (&*owner.partitions.synth) as *const _ as usize,
        (&*owner.partitions.sample) as *const _ as usize,
        owner.scratch.synth.samples[0].as_ptr() as usize,
        owner.scratch.sample.samples[0].as_ptr() as usize,
        owner
            .partitions
            .sample
            .active_sample_buffer_address_for_test(),
    )
}
