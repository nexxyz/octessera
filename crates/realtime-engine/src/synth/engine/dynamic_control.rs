use super::super::dsp_config::DspRuntimeConfig;
use super::super::fx_param::{apply_fx_param, FxParamId, FxParamMutation};
use super::super::scalar_param::{SampleBankParamId, ScalarMutation, SynthParamId};
use super::bus_chain_owner::fx_kind_cost;
use super::*;
use crate::synth::engine::render_plan::render_plan_fx_slot;

#[path = "drum_param_control.rs"]
mod drum_param_control;
#[path = "fm_param_control.rs"]
mod fm_param_control;
#[path = "pluck_param_control.rs"]
mod pluck_param_control;

impl SynthEngine {
    pub fn set_dsp_config(&mut self, config: DspRuntimeConfig) {
        self.worker_load_warning
            .set_threshold(config.worker_warning_threshold);
        if self.dsp_config.bus_idle_threshold != config.bus_idle_threshold {
            for chain in &mut self.bus_chains {
                chain.reset_quiet();
            }
        }
        self.dsp_config = config;
    }

    pub fn set_master_volume(&mut self, volume_pct: f32) -> ScalarMutation {
        #[cfg(feature = "routing-tree-benchmark")]
        if self.routing_tree_assignment.is_some()
            && self.routing_tree_source_event_sample_clock.is_none()
        {
            self.reject_routing_tree_mutation_for_control();
            return ScalarMutation::Rejected;
        }
        set_clamped_f32(&mut self.master_volume, volume_pct, 0.0, 100.0, 100.0)
    }

    pub fn set_instrument_mixer(
        &mut self,
        instrument_slot: usize,
        volume_pct: Option<f32>,
        pan_pos: Option<usize>,
    ) -> ScalarMutation {
        #[cfg(feature = "routing-tree-benchmark")]
        if self.routing_tree_assignment.is_some()
            && self.routing_tree_source_event_sample_clock.is_none()
        {
            self.reject_routing_tree_mutation_for_control();
            return ScalarMutation::Rejected;
        }
        let slot = instrument_slot.min(INSTRUMENT_SLOT_COUNT - 1);
        let mut mutation = ScalarMutation::Unchanged;
        if let Some(volume_pct) = volume_pct {
            mutation = combine_mutation(
                mutation,
                set_clamped_f32(&mut self.slot_volume[slot], volume_pct, 0.0, 100.0, 100.0),
            );
        }
        if let Some(pan_pos) = pan_pos {
            let normalized_pan = pan_pos.min(self.pan_positions - 1);
            if self.slot_pan_pos[slot] != normalized_pan {
                self.slot_pan_pos[slot] = normalized_pan;
                self.slot_pan_gains[slot] = pan_gains(normalized_pan, self.pan_positions);
                mutation = combine_mutation(mutation, ScalarMutation::Changed);
            }
        }
        mutation
    }

    pub fn set_fx_bus_mixer(
        &mut self,
        bus_index: usize,
        pan_pos: Option<usize>,
        volume_pct: Option<f32>,
    ) -> ScalarMutation {
        #[cfg(feature = "routing-tree-benchmark")]
        if self.routing_tree_assignment.is_some()
            && self.routing_tree_source_event_sample_clock.is_none()
        {
            self.reject_routing_tree_mutation_for_control();
            return ScalarMutation::Rejected;
        }
        if bus_index >= self.bus_pan_pos.len() {
            return ScalarMutation::Rejected;
        }
        let mut mutation = ScalarMutation::Unchanged;
        if let Some(pan_pos) = pan_pos {
            let normalized_pan = pan_pos.min(self.pan_positions - 1);
            if self.bus_pan_pos[bus_index] != normalized_pan {
                self.bus_pan_pos[bus_index] = normalized_pan;
                self.bus_pan_gains_cache[bus_index] = pan_gains(normalized_pan, self.pan_positions);
                mutation = combine_mutation(mutation, ScalarMutation::Changed);
            }
        }
        if let Some(volume_pct) = volume_pct {
            mutation = combine_mutation(
                mutation,
                set_clamped_f32(
                    &mut self.bus_volume[bus_index],
                    volume_pct,
                    0.0,
                    100.0,
                    100.0,
                ),
            );
        }
        mutation
    }

    pub fn set_synth_param_typed(
        &mut self,
        instrument_slot: usize,
        id: SynthParamId,
        value: f32,
    ) -> ScalarMutation {
        #[cfg(feature = "routing-tree-benchmark")]
        if self.routing_tree_assignment.is_some()
            && self.routing_tree_source_event_sample_clock.is_none()
        {
            self.reject_routing_tree_mutation_for_control();
            return ScalarMutation::Rejected;
        }
        let slot = instrument_slot.min(INSTRUMENT_SLOT_COUNT - 1);
        if self.slot_kind[slot] != InstrumentKind::Synth {
            return ScalarMutation::Rejected;
        }
        let mutation = {
            let synth = &mut self.instruments[slot];
            match id {
                SynthParamId::Osc1LevelPct => {
                    set_clamped_f32(&mut synth.osc1.level_pct, value, 0.0, 100.0, 1.0)
                }
                SynthParamId::Osc1DetuneCents => {
                    set_clamped_f32(&mut synth.osc1.detune_cents, value, -50.0, 50.0, 1.0)
                }
                SynthParamId::Osc1PulseWidthPct => {
                    set_clamped_f32(&mut synth.osc1.pulse_width_pct, value, 5.0, 95.0, 1.0)
                }
                SynthParamId::Osc2LevelPct => {
                    set_clamped_f32(&mut synth.osc2.level_pct, value, 0.0, 100.0, 1.0)
                }
                SynthParamId::Osc2DetuneCents => {
                    set_clamped_f32(&mut synth.osc2.detune_cents, value, -50.0, 50.0, 1.0)
                }
                SynthParamId::Osc2PulseWidthPct => {
                    set_clamped_f32(&mut synth.osc2.pulse_width_pct, value, 5.0, 95.0, 1.0)
                }
                SynthParamId::AmpGainPct => {
                    set_clamped_f32(&mut synth.amp.gain_pct, value, 0.0, 100.0, 1.0)
                }
                SynthParamId::AmpVelocitySensitivityPct => set_clamped_f32(
                    &mut synth.amp.velocity_sensitivity_pct,
                    value,
                    0.0,
                    100.0,
                    1.0,
                ),
                SynthParamId::AmpEnvAttackMs => {
                    set_clamped_f32(&mut synth.amp_env.attack_ms, value, 0.0, 5000.0, 1.0)
                }
                SynthParamId::AmpEnvDecayMs => {
                    set_clamped_f32(&mut synth.amp_env.decay_ms, value, 0.0, 5000.0, 1.0)
                }
                SynthParamId::AmpEnvSustainPct => {
                    set_clamped_f32(&mut synth.amp_env.sustain_pct, value, 0.0, 100.0, 1.0)
                }
                SynthParamId::AmpEnvReleaseMs => {
                    set_clamped_f32(&mut synth.amp_env.release_ms, value, 0.0, 10000.0, 1.0)
                }
                SynthParamId::FilterCutoffHz => {
                    set_clamped_f32(&mut synth.filter.cutoff_hz, value, 20.0, 20_000.0, 1.0)
                }
                SynthParamId::FilterResonance => {
                    set_clamped_f32(&mut synth.filter.resonance, value, 0.0, 255.0, 1.0)
                }
                SynthParamId::FilterEnvAmountPct => {
                    set_clamped_f32(&mut synth.filter.env_amount_pct, value, -100.0, 100.0, 1.0)
                }
                SynthParamId::FilterKeyTrackingPct => {
                    set_clamped_f32(&mut synth.filter.key_tracking_pct, value, 0.0, 100.0, 1.0)
                }
                SynthParamId::FilterEnvAttackMs => {
                    set_clamped_f32(&mut synth.filter_env.attack_ms, value, 0.0, 5000.0, 1.0)
                }
                SynthParamId::FilterEnvDecayMs => {
                    set_clamped_f32(&mut synth.filter_env.decay_ms, value, 0.0, 5000.0, 1.0)
                }
                SynthParamId::FilterEnvSustainPct => {
                    set_clamped_f32(&mut synth.filter_env.sustain_pct, value, 0.0, 100.0, 1.0)
                }
                SynthParamId::FilterEnvReleaseMs => {
                    set_clamped_f32(&mut synth.filter_env.release_ms, value, 0.0, 10000.0, 1.0)
                }
            }
        };
        if mutation == ScalarMutation::Changed {
            let source_generation = self.synth_render_configs[slot].source_generation;
            let mut render = SynthVoiceRenderConfig::from_config(self.instruments[slot]);
            render.source_generation = source_generation;
            self.synth_render_configs[slot] = render;
            self.synth_render_revisions[slot] = self.synth_render_revisions[slot].wrapping_add(1);
        }
        mutation
    }

    pub fn set_synth_param(
        &mut self,
        instrument_slot: usize,
        path: &str,
        value: f32,
    ) -> ScalarMutation {
        let Some(id) = SynthParamId::from_path(path) else {
            return ScalarMutation::Rejected;
        };
        self.set_synth_param_typed(instrument_slot, id, value)
    }

    pub fn set_sample_bank_param_typed(
        &mut self,
        instrument_slot: usize,
        id: SampleBankParamId,
        value: f32,
    ) -> ScalarMutation {
        #[cfg(feature = "routing-tree-benchmark")]
        if self.routing_tree_assignment.is_some()
            && self.routing_tree_source_event_sample_clock.is_none()
        {
            self.reject_routing_tree_mutation_for_control();
            return ScalarMutation::Rejected;
        }
        let slot = instrument_slot.min(INSTRUMENT_SLOT_COUNT - 1);
        if !self.sample_voice_pool.has_home() {
            return ScalarMutation::Rejected;
        }
        let (mutation, cutoff_hz, resonance) = {
            let Some(bank) = self.sample_banks.get_mut(slot) else {
                return ScalarMutation::Rejected;
            };
            let mutation = match id {
                SampleBankParamId::TuneSemis => {
                    set_clamped_f32(&mut bank.tune_semis, value, -24.0, 24.0, 1.0)
                }
                SampleBankParamId::AmpGainPct => {
                    set_clamped_f32(&mut bank.gain_pct, value, 0.0, 100.0, 1.0)
                }
                SampleBankParamId::AmpVelocitySensitivityPct => {
                    set_clamped_f32(&mut bank.velocity_sensitivity_pct, value, 0.0, 100.0, 1.0)
                }
                SampleBankParamId::FilterCutoffHz => {
                    set_clamped_f32(&mut bank.filter_cutoff_hz, value, 20.0, 20_000.0, 1.0)
                }
                SampleBankParamId::FilterResonance => {
                    set_clamped_f32(&mut bank.filter_resonance, value, 0.0, 255.0, 1.0)
                }
            };
            (mutation, bank.filter_cutoff_hz, bank.filter_resonance)
        };
        if mutation == ScalarMutation::Changed
            && matches!(
                id,
                SampleBankParamId::FilterCutoffHz | SampleBankParamId::FilterResonance
            )
        {
            let _ = self
                .sample_voice_pool
                .update_filter_for_slot(slot, cutoff_hz, resonance);
        }
        mutation
    }

    pub fn set_sample_bank_param(
        &mut self,
        instrument_slot: usize,
        path: &str,
        value: f32,
    ) -> ScalarMutation {
        let Some(id) = SampleBankParamId::from_path(path) else {
            return ScalarMutation::Rejected;
        };
        self.set_sample_bank_param_typed(instrument_slot, id, value)
    }

    pub fn set_fx_bus_slot(
        &mut self,
        bus_index: usize,
        slot_index: usize,
        fx_type: String,
        params: BTreeMap<String, Value>,
    ) {
        if bus_index >= self.bus_chains.len() || slot_index >= BUS_SLOTS_PER_BUS {
            return;
        }
        let config = FxBusSlotConfig::Config {
            kind: fx_type,
            params,
        };
        let render_plan = render_plan_fx_slot(&config);
        let next_params = compile_fx_bus_params(&config);
        let next_state = fx_bus_state_from_params(&next_params, self.sample_rate);
        let retired_slot = self.bus_chains[bus_index].replace_slot(
            slot_index,
            next_params,
            next_state,
            fx_kind_cost(render_plan.kind),
        );
        if let Some(retired_slot) = retired_slot {
            self.pending_render_retired
                .bus_chains
                .push(BusChainOwner::from_slot(
                    bus_index,
                    slot_index,
                    retired_slot,
                ));
        }
        self.render_plan
            .install_bus_fx_slot(bus_index, slot_index, render_plan);
    }

    pub fn set_global_fx_slot(
        &mut self,
        slot_index: usize,
        fx_type: String,
        params: BTreeMap<String, Value>,
    ) {
        if slot_index >= self.master_slot_params.len() {
            return;
        }
        let config = FxBusSlotConfig::Config {
            kind: fx_type,
            params,
        };
        let render_plan = render_plan_fx_slot(&config);
        let next_params = compile_fx_bus_params(&config);
        if !master_fx_state_matches_params(&self.master_slot_state[slot_index], &next_params) {
            self.master_slot_state[slot_index] = master_fx_state_from_params(&next_params);
        }
        self.master_slot_params[slot_index] = next_params;
        self.render_plan
            .install_master_fx_slot(slot_index, render_plan);
        self.refresh_master_active_slot_indices();
    }

    pub fn set_fx_bus_param(
        &mut self,
        bus_index: usize,
        slot_index: usize,
        id: FxParamId,
        value: f32,
    ) -> FxParamMutation {
        #[cfg(feature = "routing-tree-benchmark")]
        if self.routing_tree_assignment.is_some()
            && (self.routing_tree_source_event_sample_clock.is_none() || self.bus_chains.is_empty())
        {
            self.reject_routing_tree_mutation_for_control();
            return FxParamMutation::Rejected;
        }
        let Some(chain) = self.bus_chains.get_mut(bus_index) else {
            return FxParamMutation::Rejected;
        };
        chain.set_slot_param(slot_index, id, value)
    }

    pub fn set_global_fx_param(
        &mut self,
        slot_index: usize,
        id: FxParamId,
        value: f32,
    ) -> FxParamMutation {
        #[cfg(feature = "routing-tree-benchmark")]
        if self.routing_tree_assignment.is_some()
            && self.routing_tree_source_event_sample_clock.is_none()
        {
            self.reject_routing_tree_mutation_for_control();
            return FxParamMutation::Rejected;
        }
        let Some(params) = self.master_slot_params.get_mut(slot_index) else {
            return FxParamMutation::Rejected;
        };
        apply_fx_param(params, id, value)
    }
}

fn set_clamped_f32(
    current: &mut f32,
    value: f32,
    minimum: f32,
    maximum: f32,
    divisor: f32,
) -> ScalarMutation {
    if !value.is_finite() {
        return ScalarMutation::Rejected;
    }
    let next = (value / divisor).clamp(minimum / divisor, maximum / divisor);
    if current.to_bits() == next.to_bits() {
        ScalarMutation::Unchanged
    } else {
        *current = next;
        ScalarMutation::Changed
    }
}

fn combine_mutation(current: ScalarMutation, next: ScalarMutation) -> ScalarMutation {
    match (current, next) {
        (ScalarMutation::Changed, _) | (_, ScalarMutation::Changed) => ScalarMutation::Changed,
        (ScalarMutation::Rejected, _) | (_, ScalarMutation::Rejected) => ScalarMutation::Rejected,
        _ => ScalarMutation::Unchanged,
    }
}
