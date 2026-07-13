use super::render_voice::render_synth_voice_sample_precomputed;
use super::*;

impl SynthEngine {
    pub(super) fn render_synth_voices(
        &mut self,
        slot_out: &mut [f32; INSTRUMENT_SLOT_COUNT],
    ) -> bool {
        let mut active = false;
        for (slot_idx, out) in slot_out.iter_mut().enumerate().take(INSTRUMENT_SLOT_COUNT) {
            let rendered = self.render_synth_slot(slot_idx);
            *out += rendered.sample;
            active |= rendered.active;
        }
        active
    }

    pub(super) fn render_synth_slot(&mut self, slot_idx: usize) -> SlotFrameOutput {
        if !self.active_synth_slots[slot_idx] {
            return SlotFrameOutput::default();
        }
        let mut out = 0.0;
        let mut slot_active = false;
        let pool = &mut self.voices[slot_idx];
        for v in pool.iter_mut() {
            if !v.active {
                continue;
            }
            debug_assert_eq!(v.instrument_slot as usize, slot_idx);
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
            out += render_synth_voice_sample_precomputed(
                self.sample_rate,
                self.mods[slot],
                &self.synth_render_configs[slot],
                v,
                amp_env,
                filt_env,
            );
            slot_active = true;
        }
        self.active_synth_slots[slot_idx] = slot_active;
        SlotFrameOutput {
            sample: out,
            active: slot_active,
        }
    }
}
