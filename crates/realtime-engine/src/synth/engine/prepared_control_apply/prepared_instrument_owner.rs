use super::super::prepared_control_prepare::PreparedInstrumentSlot;
use super::super::retired_state::RetiredAudioState;
use super::super::*;

impl SynthEngine {
    pub fn apply_prepared_instrument_owner(
        &mut self,
        index: usize,
        prepared: PreparedInstrumentSlot,
        sample_bank: Option<SampleBankConfig>,
    ) -> RetiredAudioState {
        let mut retired = RetiredAudioState::default();
        if index >= INSTRUMENT_SLOT_COUNT {
            retired.prepared_instrument_slot = Some(prepared);
            retired.sample_bank = sample_bank;
            return retired;
        }
        #[cfg(feature = "routing-tree-executor")]
        if self.routing_tree_assignment.is_some()
            && !self.routing_tree_prepared_instrument_slot_allowed(index, &prepared)
        {
            self.reject_routing_tree_mutation_for_control();
            retired.prepared_instrument_slot = Some(prepared);
            retired.sample_bank = sample_bank;
            return retired;
        }
        if sample_bank.is_some() && !self.sample_voice_pool.has_home() {
            #[cfg(feature = "routing-tree-executor")]
            self.reject_routing_tree_mutation_for_control();
            retired.prepared_instrument_slot = Some(prepared);
            retired.sample_bank = sample_bank;
            return retired;
        }

        self.install_prepared_instrument_slot(index, prepared);

        if let Some(bank) = sample_bank {
            let current = self
                .sample_banks
                .get_mut(index)
                .expect("validated instrument slot");
            retired.sample_bank = Some(std::mem::replace(current, bank));
        }
        if let Some(voices) = self.sample_voice_pool.clear_slot(index) {
            retired.sample_voices = voices;
        }
        retired
    }
}
