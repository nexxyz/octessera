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

    pub fn set_fx_bus_mixer(
        &mut self,
        bus_index: usize,
        pan_pos: Option<usize>,
        volume_pct: Option<f32>,
    ) {
        if bus_index >= self.bus_pan_pos.len() {
            return;
        }
        if let Some(pan_pos) = pan_pos {
            self.bus_pan_pos[bus_index] = pan_pos.min(self.pan_positions - 1);
            self.bus_pan_gains_cache[bus_index] =
                pan_gains(self.bus_pan_pos[bus_index], self.pan_positions);
        }
        if let Some(volume_pct) = volume_pct {
            self.bus_volume[bus_index] = (volume_pct / 100.0).clamp(0.0, 1.0);
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
            "synth.amp.velocitySensitivityPct" => {
                synth.amp.velocity_sensitivity_pct = value.clamp(0.0, 100.0)
            }
            "synth.ampEnv.attackMs" => synth.amp_env.attack_ms = value.clamp(0.0, 5000.0),
            "synth.ampEnv.decayMs" => synth.amp_env.decay_ms = value.clamp(0.0, 5000.0),
            "synth.ampEnv.sustainPct" => synth.amp_env.sustain_pct = value.clamp(0.0, 100.0),
            "synth.ampEnv.releaseMs" => synth.amp_env.release_ms = value.clamp(0.0, 10000.0),
            "synth.filter.cutoffHz" => synth.filter.cutoff_hz = value.clamp(20.0, 20_000.0),
            "synth.filter.resonance" => synth.filter.resonance = value.clamp(0.0, 255.0),
            "synth.filter.envAmountPct" => synth.filter.env_amount_pct = value.clamp(-100.0, 100.0),
            "synth.filter.keyTrackingPct" => {
                synth.filter.key_tracking_pct = value.clamp(0.0, 100.0)
            }
            "synth.filterEnv.attackMs" => synth.filter_env.attack_ms = value.clamp(0.0, 5000.0),
            "synth.filterEnv.decayMs" => synth.filter_env.decay_ms = value.clamp(0.0, 5000.0),
            "synth.filterEnv.sustainPct" => synth.filter_env.sustain_pct = value.clamp(0.0, 100.0),
            "synth.filterEnv.releaseMs" => synth.filter_env.release_ms = value.clamp(0.0, 10000.0),
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
            "sample.filter.cutoffHz" => bank.filter_cutoff_hz = value.clamp(20.0, 20_000.0),
            "sample.filter.resonance" => bank.filter_resonance = value.clamp(0.0, 255.0),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dynamic_fx_bus_slot_accepts_third_slot_and_ignores_fourth() {
        let mut engine = SynthEngine::new(48_000);
        engine.set_instruments(InstrumentsConfig {
            instruments: Vec::new(),
            mixer: Some(MixerConfig {
                buses: vec![FxBusConfig::default()],
                master: None,
            }),
            pan_positions: DEFAULT_PAN_POSITIONS,
            master_volume: 100.0,
        });

        engine.set_fx_bus_slot(0, 2, "tremolo".into(), BTreeMap::new());
        assert_eq!(engine.bus_active_slot_counts[0], 1);
        assert!(matches!(
            engine.bus_slot_params[0][2],
            FxBusParams::Tremolo { .. }
        ));

        engine.set_fx_bus_slot(0, 3, "delay".into(), BTreeMap::new());
        assert_eq!(engine.bus_active_slot_counts[0], 1);
    }

    #[test]
    fn master_fx_config_ignores_third_slot() {
        let mut engine = SynthEngine::new(48_000);
        engine.set_instruments(InstrumentsConfig {
            instruments: Vec::new(),
            mixer: Some(MixerConfig {
                buses: Vec::new(),
                master: Some(MasterFxConfig {
                    slots: vec![
                        FxBusSlotConfig::Kind("none".into()),
                        FxBusSlotConfig::Kind("none".into()),
                        FxBusSlotConfig::Kind("tremolo".into()),
                    ],
                }),
            }),
            pan_positions: DEFAULT_PAN_POSITIONS,
            master_volume: 100.0,
        });

        assert_eq!(engine.master_slot_params.len(), GLOBAL_FX_SLOT_COUNT);
        assert!(engine.master_active_slot_indices.is_empty());
    }
}
