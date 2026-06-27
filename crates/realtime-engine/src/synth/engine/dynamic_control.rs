use super::*;
use crate::synth::engine::control::active_fx_bus_slots;

impl SynthEngine {
    pub fn set_master_volume(&mut self, volume_pct: f32) {
        self.master_volume = (volume_pct / 100.0).clamp(0.0, 1.0);
    }

    pub fn set_instrument_mixer(
        &mut self,
        instrument_slot: usize,
        volume_pct: Option<f32>,
        pan_pos: Option<usize>,
    ) {
        let slot = instrument_slot.min(INSTRUMENT_SLOT_COUNT - 1);
        if let Some(volume_pct) = volume_pct {
            self.slot_volume[slot] = (volume_pct / 100.0).clamp(0.0, 1.0);
        }
        if let Some(pan_pos) = pan_pos {
            self.slot_pan_pos[slot] = pan_pos.min(self.pan_positions - 1);
            self.slot_pan_gains[slot] = pan_gains(self.slot_pan_pos[slot], self.pan_positions);
        }
    }

    pub fn set_fx_bus_mixer(&mut self, bus_index: usize, pan_pos: Option<usize>) {
        if bus_index >= self.bus_pan_pos.len() {
            return;
        }
        if let Some(pan_pos) = pan_pos {
            self.bus_pan_pos[bus_index] = pan_pos.min(self.pan_positions - 1);
            self.bus_pan_gains_cache[bus_index] =
                pan_gains(self.bus_pan_pos[bus_index], self.pan_positions);
        }
    }

    pub fn set_synth_param(&mut self, instrument_slot: usize, path: &str, value: f32) {
        let slot = instrument_slot.min(INSTRUMENT_SLOT_COUNT - 1);
        if self.slot_kind[slot] != InstrumentKind::Synth {
            return;
        }
        let synth = &mut self.instruments[slot];
        match path {
            "synth.amp.gainPct" => synth.amp.gain_pct = value.clamp(0.0, 100.0),
            "synth.filter.cutoffHz" => synth.filter.cutoff_hz = value.clamp(20.0, 20_000.0),
            "synth.filter.resonance" => synth.filter.resonance = value.clamp(0.0, 255.0),
            _ => return,
        }
        self.synth_render_configs[slot] = SynthVoiceRenderConfig::from_config(*synth);
        self.synth_render_revisions[slot] = self.synth_render_revisions[slot].wrapping_add(1);
    }

    pub fn set_sample_bank_param(&mut self, instrument_slot: usize, path: &str, value: f32) {
        let slot = instrument_slot.min(INSTRUMENT_SLOT_COUNT - 1);
        let Some(bank) = self.sample_banks.get_mut(slot) else {
            return;
        };
        match path {
            "sample.tuneSemis" => bank.tune_semis = value.clamp(-24.0, 24.0),
            "sample.amp.gainPct" => bank.gain_pct = value.clamp(0.0, 100.0),
            "sample.amp.velocitySensitivityPct" => {
                bank.velocity_sensitivity_pct = value.clamp(0.0, 100.0)
            }
            _ => (),
        }
    }

    pub fn set_fx_bus_slot(
        &mut self,
        bus_index: usize,
        slot_index: usize,
        fx_type: String,
        params: BTreeMap<String, Value>,
    ) {
        if bus_index >= self.bus_slot_params.len() || slot_index >= BUS_SLOTS_PER_BUS {
            return;
        }
        let config = FxBusSlotConfig::Config {
            kind: fx_type,
            params,
        };
        let next_params = compile_fx_bus_params(&config);
        if !fx_bus_state_matches_params(&self.bus_slot_state[bus_index][slot_index], &next_params) {
            self.bus_slot_state[bus_index][slot_index] =
                fx_bus_state_from_params(&next_params, self.sample_rate);
        }
        self.bus_slot_params[bus_index][slot_index] = next_params;
        let (active_indices, active_count) = active_fx_bus_slots(&self.bus_slot_params[bus_index]);
        self.bus_active_slot_indices[bus_index] = active_indices;
        self.bus_active_slot_counts[bus_index] = active_count;
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
        let next_params = compile_fx_bus_params(&config);
        if !master_fx_state_matches_params(&self.master_slot_state[slot_index], &next_params) {
            self.master_slot_state[slot_index] = master_fx_state_from_params(&next_params);
        }
        self.master_slot_params[slot_index] = next_params;
        self.refresh_master_active_slot_indices();
    }
}
