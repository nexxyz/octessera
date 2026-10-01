use super::*;

impl SourceWorkerRuntime {
    pub(crate) fn home_bus_carrier_scratch_addresses_for_test(
        &self,
    ) -> [[Option<usize>; super::super::super::types::BUS_COUNT]; SOURCE_WORKER_COUNT] {
        std::array::from_fn(|parity| {
            let owner = self.take_home_owner_for_test(parity).expect("worker home");
            let addresses = owner.bus_carriers.each_ref().map(|carrier| {
                carrier
                    .as_ref()
                    .map(|carrier| carrier.scratch.input.as_ptr() as usize)
            });
            self.return_home_owner_for_test(owner);
            addresses
        })
    }

    pub(crate) fn set_after_bus_dispatch_hook_for_test(
        &mut self,
        hook: fn(&mut SourceWorkerRuntime),
    ) {
        self.after_bus_dispatch = Some(hook);
    }

    pub(crate) fn swap_completion_carrier_for_test(&mut self, logical_bus_id: usize) -> bool {
        if logical_bus_id >= super::super::super::types::BUS_COUNT {
            return false;
        }
        let Some(done_rxs) = self.done_rxs.as_ref() else {
            return false;
        };
        let Some(mut first) = done_rxs[0].try_recv().ok() else {
            return false;
        };
        let Some(mut second) = done_rxs[1].try_recv().ok() else {
            self.done_txs
                .as_ref()
                .expect("source worker lifecycle is active")[0]
                .try_send(first)
                .expect("completion mailbox");
            return false;
        };
        std::mem::swap(
            &mut first.owner.bus_carriers[logical_bus_id],
            &mut second.owner.bus_carriers[logical_bus_id],
        );
        let done_txs = self
            .done_txs
            .as_ref()
            .expect("source worker lifecycle is active");
        done_txs[0].try_send(first).is_ok() && done_txs[1].try_send(second).is_ok()
    }
}
