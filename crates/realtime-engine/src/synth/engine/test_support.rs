use super::*;

impl SynthEngine {
    #[cfg(test)]
    pub(in crate::synth) fn active_voice_count_for_slot(&self, slot: usize) -> usize {
        self.voices[slot].iter().filter(|v| v.active).count()
    }

    #[cfg(test)]
    pub(in crate::synth) fn mod_values_for_slot(&self, slot: usize) -> (f32, f32) {
        let s = slot.min(INSTRUMENT_SLOT_COUNT - 1);
        (self.mods[s].cutoff_cc, self.mods[s].resonance_cc)
    }

    #[cfg(test)]
    pub(in crate::synth) fn delay_state_probe(
        &self,
        bus: usize,
        slot: usize,
    ) -> Option<(usize, f32)> {
        match self.bus_slot_state.get(bus)?.get(slot)? {
            FxBusState::Delay { buf, idx } => Some((*idx, buf.iter().map(|v| v.abs()).sum())),
            _ => None,
        }
    }

    #[cfg(test)]
    pub(in crate::synth) fn master_compressor_env_probe(&self, slot: usize) -> Option<f32> {
        match self.master_slot_state.get(slot)? {
            MasterFxState::Compressor { env } => Some(*env),
            _ => None,
        }
    }

    #[cfg(test)]
    pub(in crate::synth) fn pitch_buf_probe(&self, id: &str) -> Option<usize> {
        for fx in &self.momentary_fx {
            if fx.id == id && matches!(fx.kind, MomentaryFxKind::PitchShift) {
                return Some(fx.pitch_shifter.write_pos);
            }
        }
        None
    }

    #[cfg(test)]
    #[allow(clippy::type_complexity)]
    pub(in crate::synth) fn stutter_buf_for_id(
        &self,
        id: &str,
    ) -> Option<(Vec<f32>, Vec<f32>, usize, bool, usize)> {
        for fx in &self.momentary_fx {
            if fx.id == id && matches!(fx.kind, MomentaryFxKind::Stutter) {
                return Some((
                    fx.stutter_l.clone(),
                    fx.stutter_r.clone(),
                    fx.stutter_write,
                    fx.stutter_ready,
                    fx.stutter_ramp_pos,
                ));
            }
        }
        None
    }
}
