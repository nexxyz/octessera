use super::super::source_worker_lifecycle::{OwnerEnvelope, SourceWorkerOwnerIdentity};
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
        self.expected_frames = frames;
        self.expected_base_sample_clock = engine.sample_clock;
        self.dispatch(engine)
    }

    pub(crate) fn collect_for_test(&mut self, engine: &mut SynthEngine) -> bool {
        self.collect(engine, false)
    }

    pub(crate) fn in_flight_mask_for_test(&self) -> u8 {
        self.in_flight_mask
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
        let receiver = &self.done_rxs.as_ref().expect("persistent source workers")[parity];
        let Ok(mut completion) = receiver.try_recv() else {
            return false;
        };
        completion.sequence = completion.sequence.wrapping_add(1);
        self.done_txs
            .as_ref()
            .expect("source worker lifecycle is active")[parity]
            .try_send(completion)
            .is_ok()
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
