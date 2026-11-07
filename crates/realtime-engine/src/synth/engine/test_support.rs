use super::*;

impl SynthEngine {
    #[cfg(test)]
    pub(in crate::synth) fn process_momentary_frame_for_test(
        &mut self,
        target: MomentaryFxTarget,
        left: f32,
        right: f32,
    ) -> (f32, f32) {
        self.process_momentary_fx_target(target, left, right)
    }

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

    /// Renders long enough for every voice a steal is fading out to finish and free its lane.
    #[cfg(test)]
    pub(in crate::synth) fn render_through_steal_fades(&mut self) {
        for _ in 0..crate::synth::steal_declick::STEAL_FADE_FRAMES {
            self.next_stereo_sample();
        }
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
        match self.bus_chains.get(bus)?.slot_state.get(slot)? {
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
        self.momentary_fx
            .iter()
            .find(|fx| fx.id == id && fx.kind == MomentaryFxKind::PitchShift)
            .map(|fx| fx.voice.pitch().shifter.write_pos)
    }

    #[cfg(test)]
    #[allow(clippy::type_complexity)]
    pub(in crate::synth) fn stutter_buf_for_id(
        &self,
        id: &str,
    ) -> Option<(Vec<f32>, Vec<f32>, usize, bool, usize)> {
        self.momentary_fx
            .iter()
            .find(|fx| fx.id == id && fx.kind == MomentaryFxKind::Stutter)
            .map(|fx| {
                let voice = fx.voice.stutter();
                (
                    voice.left.clone(),
                    voice.right.clone(),
                    voice.write,
                    voice.ready,
                    voice.ramp_pos,
                )
            })
    }

    #[cfg(test)]
    #[allow(clippy::type_complexity)]
    pub(in crate::synth) fn freeze_state_probe(
        &self,
        id: &str,
    ) -> Option<([Vec<f32>; 4], [usize; 4], [f32; 4], u32, u32, u32, u32)> {
        self.momentary_fx
            .iter()
            .find(|fx| fx.id == id && fx.kind == MomentaryFxKind::Freeze)
            .map(|fx| {
                let voice = fx.voice.freeze();
                (
                    voice.bufs.clone(),
                    voice.idxs,
                    voice.lp,
                    voice.inject_pos,
                    voice.ready_len,
                    voice.activation_pos,
                    voice.activation_len,
                )
            })
    }

    /// Test shorthand for a Play FX press through the prepared path.
    pub(in crate::synth) fn momentary_fx_start(
        &mut self,
        id: String,
        fx_type: String,
        params: BTreeMap<String, Value>,
        target: MomentaryFxTarget,
    ) {
        let Some(start) =
            super::prepare_momentary_fx_start(id, fx_type, params, target, self.sample_rate)
        else {
            return;
        };
        drop(self.apply_prepared_momentary_fx_start(start));
    }

    /// Test shorthand for the release of the Play FX press `id`.
    pub(in crate::synth) fn momentary_fx_stop(&mut self, id: &str) -> RetiredAudioState {
        match self.momentary_fx.iter().position(|fx| fx.id == id) {
            Some(pos) => self.stop_momentary_fx_at(pos),
            None => RetiredAudioState::default(),
        }
    }

    /// Test shorthand for a live update of the Play FX press `id`.
    pub(in crate::synth) fn momentary_fx_update(
        &mut self,
        id: &str,
        params: &BTreeMap<String, Value>,
    ) {
        let sample_rate = self.sample_rate;
        if let Some(fx) = self.momentary_fx.iter_mut().find(|fx| fx.id == id) {
            let update = super::momentary_fx_voice::prepare_voice_update(
                fx.kind,
                fx.epoch,
                params,
                sample_rate,
            );
            let _ = fx.voice.apply_update(update);
        }
    }
}
