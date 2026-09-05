use super::*;

impl SourceWorkerRuntime {
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
}
