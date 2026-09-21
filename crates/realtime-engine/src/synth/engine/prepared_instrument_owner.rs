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
        #[cfg(feature = "routing-tree-benchmark")]
        if self.routing_tree_assignment.is_some()
            && !self.routing_tree_prepared_instrument_slot_allowed(index, &prepared)
        {
            self.reject_routing_tree_mutation_for_control();
            retired.prepared_instrument_slot = Some(prepared);
            retired.sample_bank = sample_bank;
            return retired;
        }
        if sample_bank.is_some() && !self.sample_voice_pool.has_home() {
            #[cfg(feature = "routing-tree-benchmark")]
            self.reject_routing_tree_mutation_for_control();
            retired.prepared_instrument_slot = Some(prepared);
            retired.sample_bank = sample_bank;
            return retired;
        }

        let has_mixer = prepared.route.is_some();
        self.apply_normalized_instrument_slot(
            index,
            prepared.kind,
            prepared.synth,
            prepared.render_config,
            prepared
                .route
                .map(|route| super::control::NormalizedInstrumentMixer {
                    route,
                    pan_pos: prepared.pan_pos.min(self.pan_positions - 1),
                    volume: prepared.volume,
                }),
        );
        if has_mixer {
            self.slot_pan_gains[index] =
                super::support::pan_gains(self.slot_pan_pos[index], self.pan_positions);
        }
        let mut render_plan = prepared.render_plan;
        render_plan.route = render_plan.route.map(|route| match route {
            super::render_plan::RenderPlanRoute::Bus(bus) if bus >= self.bus_chains.len() => {
                super::render_plan::RenderPlanRoute::Direct
            }
            route => route,
        });
        self.render_plan.install_instrument_slot(index, render_plan);
        self.refresh_routed_bus_slot_count();
        #[cfg(feature = "routing-tree-benchmark")]
        if self.routing_tree_assignment.is_some() {
            let _ = self.refresh_routing_tree_assignment();
        }

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
