use super::super::source_worker_lifecycle::{OwnerEnvelope, SourceWorkerOwnerIdentity};
use super::*;
use std::time::Duration;

impl SourceWorkerRuntime {
    pub(crate) fn home_owner_identities_for_test(&self) -> [SourceWorkerOwnerIdentity; 2] {
        std::array::from_fn(|parity| {
            let owner = self.take_home_owner_for_test(parity).expect("worker home");
            let identity = owner_identity(&owner);
            self.return_home_owner_for_test(owner);
            identity
        })
    }

    pub(in crate::synth::engine) fn take_home_owners_for_test(&self) -> Option<[OwnerEnvelope; 2]> {
        let first = self.take_home_owner_for_test(0)?;
        let Some(second) = self.take_home_owner_for_test(1) else {
            self.return_home_owner_for_test(first);
            return None;
        };
        Some([first, second])
    }

    pub(in crate::synth::engine) fn return_home_owners_for_test(&self, owners: [OwnerEnvelope; 2]) {
        for owner in owners {
            self.return_home_owner_for_test(owner);
        }
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

    #[cfg(feature = "routing-tree-executor")]
    pub(crate) fn set_home_bus_assignment_for_test(&self, bus: usize, worker: usize) {
        let mut owners = self.take_home_owners_for_test().expect("worker homes");
        for owner in &mut owners {
            if let Some(chain) = owner.bus_carriers[bus]
                .as_mut()
                .and_then(|carrier| carrier.owner.as_mut())
            {
                chain.assigned_worker = Some(worker);
            }
        }
        self.return_home_owners_for_test(owners);
    }

    pub(crate) fn completion_ready_for_test(&self, parity: usize) -> bool {
        !self.done_rxs.as_ref().expect("persistent source workers")[parity].is_empty()
    }

    pub(crate) fn rewrite_completion_measurement_for_test(
        &mut self,
        parity: usize,
        dsp_duration_ns: u64,
        active_cost_units: u16,
    ) -> bool {
        let receiver = &self.done_rxs.as_ref().expect("persistent source workers")[parity];
        let Ok(mut completion) = receiver.recv_timeout(Duration::from_secs(1)) else {
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

    pub(super) fn take_home_owner_for_test(&self, parity: usize) -> Option<OwnerEnvelope> {
        self.home_rxs.as_ref()?.get(parity)?.try_recv().ok()
    }

    pub(super) fn return_home_owner_for_test(&self, owner: OwnerEnvelope) {
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
        owner
            .partitions
            .sample
            .active_sample_buffer_address_for_test(),
    )
}
