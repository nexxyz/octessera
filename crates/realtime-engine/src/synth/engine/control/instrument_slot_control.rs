use super::super::*;

#[derive(Clone, Copy)]
pub(in crate::synth::engine) struct NormalizedInstrumentMixer {
    pub(in crate::synth::engine) route: usize,
    pub(in crate::synth::engine) pan_pos: usize,
    pub(in crate::synth::engine) volume: f32,
}

impl SynthEngine {
    pub(in crate::synth::engine) fn apply_normalized_instrument_slot(
        &mut self,
        idx: usize,
        kind: InstrumentKind,
        synth: SynthConfig,
        render_config: SynthVoiceRenderConfig,
        drum_voices: [DrumVoiceConfig; 8],
        mixer: Option<NormalizedInstrumentMixer>,
    ) {
        self.finish_instrument_param_glides(idx);
        let source_generation = self.synth_render_configs[idx]
            .source_generation
            .wrapping_add(u32::from(self.slot_kind[idx] != kind));
        if self.slot_kind[idx] != kind && self.synth_voice_pool.has_home() {
            if let Some(lanes) = self.synth_voice_pool.slot_lanes(idx) {
                let mut indices = [0; SYNTH_VOICE_LANE_CAPACITY];
                let count = lanes.len();
                indices[..count].copy_from_slice(lanes);
                for lane in indices.into_iter().take(count) {
                    if let Some(voice) = self.synth_voice_pool.lane_mut(lane) {
                        voice.active = false;
                    }
                }
                self.synth_voice_pool.compact_slot_lanes(idx);
                self.active_synth_slots[idx] = false;
            }
        }
        if self.slot_kind[idx] == InstrumentKind::Sample && kind != InstrumentKind::Sample {
            if let Some(lanes) = self.sample_voice_pool.slot_lanes(idx) {
                let mut indices = [0; SAMPLE_VOICE_LANE_CAPACITY];
                let count = lanes.len();
                indices[..count].copy_from_slice(lanes);
                for lane in indices.into_iter().take(count) {
                    self.sample_voice_pool.deactivate_lane(lane);
                }
                self.sample_voice_pool.compact_slot_lanes(idx);
                self.active_sample_slots[idx] = false;
            }
        }
        if self.slot_kind[idx] != kind {
            self.mods[idx] = InstrumentMod::new();
        }
        self.slot_kind[idx] = kind;
        if kind == InstrumentKind::Drum {
            self.drum_voices[idx] = drum_voices;
        }
        if matches!(
            kind,
            InstrumentKind::Synth
                | InstrumentKind::Fm
                | InstrumentKind::Pluck
                | InstrumentKind::Drum
        ) {
            self.instruments[idx] = synth;
        }
        let mut render_config = if matches!(
            kind,
            InstrumentKind::Synth
                | InstrumentKind::Fm
                | InstrumentKind::Pluck
                | InstrumentKind::Drum
        ) {
            render_config
        } else {
            render_config.disabled()
        };
        render_config.source_generation = source_generation;
        self.synth_render_configs[idx] = render_config;
        self.synth_render_revisions[idx] = self.synth_render_revisions[idx].wrapping_add(1);
        if let Some(mixer) = mixer {
            self.apply_normalized_instrument_mixer(
                idx,
                Some(mixer.route),
                Some(mixer.pan_pos),
                Some(mixer.volume),
            );
        }
    }

    pub(in crate::synth::engine) fn apply_normalized_instrument_mixer(
        &mut self,
        idx: usize,
        route: Option<usize>,
        pan_pos: Option<usize>,
        volume: Option<f32>,
    ) {
        if let Some(route) = route {
            self.slot_route[idx] = route;
        }
        if let Some(pan_pos) = pan_pos {
            self.slot_pan_pos[idx] = pan_pos;
        }
        if let Some(volume) = volume {
            self.slot_volume[idx] = volume;
        }
    }

    pub(in crate::synth::engine) fn refresh_routed_bus_slot_count(&mut self) {
        let bus_count = self.bus_pan_pos.len();
        self.routed_bus_slot_count = self
            .slot_route
            .iter()
            .filter(|route| **route > 0 && **route <= bus_count)
            .count();
    }
}
