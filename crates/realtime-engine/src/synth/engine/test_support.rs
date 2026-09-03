use super::*;

impl SynthEngine {
    #[cfg(test)]
    pub(in crate::synth) fn active_voice_count_for_slot(&self, slot: usize) -> usize {
        self.synth_voice_pool
            .active_count_for_slot(slot)
            .unwrap_or(0)
    }

    #[cfg(test)]
    pub(in crate::synth) fn active_sample_voice_count_for_slot(&self, slot: usize) -> usize {
        self.sample_voice_pool
            .active_count_for_slot(slot)
            .unwrap_or(0)
    }

    #[cfg(test)]
    pub(in crate::synth) fn assert_voice_pool_invariants(&self) {
        self.synth_voice_pool.assert_invariants();
        self.sample_voice_pool.assert_invariants();
    }

    #[cfg(test)]
    pub(in crate::synth) fn active_synth_lane_indices_for_slot(&self, slot: usize) -> Vec<usize> {
        self.synth_voice_pool
            .slot_lanes(slot)
            .unwrap_or(&[])
            .iter()
            .copied()
            .filter(|lane| {
                self.synth_voice_pool
                    .lane(*lane)
                    .is_some_and(|voice| voice.active)
            })
            .collect()
    }

    #[cfg(test)]
    pub(in crate::synth) fn active_synth_canonical_lane_indices_for_slot(
        &self,
        slot: usize,
    ) -> Vec<usize> {
        let mut lanes: Vec<usize> = self
            .synth_voice_pool
            .slot_lanes(slot)
            .unwrap_or(&[])
            .iter()
            .filter_map(|lane| self.synth_voice_pool.lane(*lane))
            .filter(|voice| voice.active)
            .map(|voice| voice.canonical_lane.expect("active canonical lane") as usize)
            .collect();
        lanes.sort_unstable();
        lanes
    }

    #[cfg(test)]
    pub(in crate::synth) fn active_sample_lane_indices_for_slot(&self, slot: usize) -> Vec<usize> {
        self.sample_voice_pool
            .slot_lanes(slot)
            .unwrap_or(&[])
            .iter()
            .copied()
            .filter(|lane| {
                self.sample_voice_pool
                    .lane(*lane)
                    .is_some_and(|voice| voice.active)
            })
            .collect()
    }

    #[cfg(test)]
    pub(in crate::synth) fn active_sample_canonical_lane_indices_for_slot(
        &self,
        slot: usize,
    ) -> Vec<usize> {
        let mut lanes: Vec<usize> = self
            .sample_voice_pool
            .slot_lanes(slot)
            .unwrap_or(&[])
            .iter()
            .filter_map(|lane| self.sample_voice_pool.lane(*lane))
            .filter(|voice| voice.active)
            .map(|voice| voice.canonical_lane.expect("active canonical lane") as usize)
            .collect();
        lanes.sort_unstable();
        lanes
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
            FxBusState::Delay { buf, idx, .. } => Some((*idx, buf.iter().map(|v| v.abs()).sum())),
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
