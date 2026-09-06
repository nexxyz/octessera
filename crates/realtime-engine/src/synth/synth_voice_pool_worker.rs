use super::{SynthVoicePartition, SynthVoicePool};

impl SynthVoicePartition {
    pub(crate) fn parity(&self) -> usize {
        self.parity
    }
}

impl SynthVoicePool {
    pub(crate) fn install_partition_after_vacancy_check(
        &mut self,
        parity: usize,
        partition: Box<SynthVoicePartition>,
    ) {
        self.partitions[parity] = Some(partition);
    }

    pub(crate) fn partition_is_vacant(&self, parity: usize) -> bool {
        matches!(self.partitions.get(parity), Some(None))
    }

    pub(crate) fn partition_is_present(&self, parity: usize) -> bool {
        matches!(self.partitions.get(parity), Some(Some(_)))
    }
}
