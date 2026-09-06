use super::{SampleVoicePartition, SampleVoicePool};

impl SampleVoicePartition {
    pub(crate) fn parity(&self) -> usize {
        self.parity
    }
}

impl SampleVoicePool {
    pub(crate) fn install_partition_after_vacancy_check(
        &mut self,
        parity: usize,
        partition: Box<SampleVoicePartition>,
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
