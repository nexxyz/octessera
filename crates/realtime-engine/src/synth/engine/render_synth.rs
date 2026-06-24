use super::render_voice::render_synth_voice_sample_precomputed;
use super::*;

impl SynthEngine {
    pub(super) fn render_synth_voices(
        &mut self,
        slot_out: &mut [f32; INSTRUMENT_SLOT_COUNT],
    ) -> bool {
        let mut active = false;
        for (slot_idx, pool) in self.voices.iter_mut().enumerate() {
            if !self.active_synth_slots[slot_idx] {
                continue;
            }
            let mut slot_active = false;
            for v in pool.iter_mut() {
                if !v.active {
                    continue;
                }
                let slot = (v.instrument_slot as usize).min(INSTRUMENT_SLOT_COUNT - 1);
                let cfg = self.instruments[slot];
                if self.sample_clock >= v.note_off_sample {
                    v.amp_env.begin_release(cfg.amp_env, self.sample_rate);
                    v.filt_env.begin_release(cfg.filter_env, self.sample_rate);
                }
                let amp_env = v.amp_env.next();
                let filt_env = v.filt_env.next();
                if v.amp_env.is_off() {
                    v.active = false;
                    continue;
                }
                if v.render_revision != self.synth_render_revisions[slot] {
                    refresh_synth_voice_render_cache(
                        v,
                        &self.synth_render_configs[slot],
                        self.sample_rate,
                        self.synth_render_revisions[slot],
                    );
                }
                let sample = render_synth_voice_sample_precomputed(
                    self.sample_rate,
                    self.mods[slot],
                    &self.synth_render_configs[slot],
                    v,
                    amp_env,
                    filt_env,
                );
                slot_out[slot] += sample;
                active = true;
                slot_active = true;
            }
            self.active_synth_slots[slot_idx] =
                slot_active || pool.iter().any(|voice| voice.active);
        }
        active
    }
}
