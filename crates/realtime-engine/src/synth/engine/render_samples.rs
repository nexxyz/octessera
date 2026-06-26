use super::*;

impl SynthEngine {
    pub(super) fn sample_note_on(&mut self, slot: usize, midi_note: u8, velocity: u8) {
        let sample_slot = sample_slot_for_note(midi_note);
        let Some(bank) = self.sample_banks.get(slot) else {
            return;
        };
        let Some(Some(buffer)) = bank.slots.get(sample_slot).map(|s| s.buffer.as_ref()) else {
            return;
        };
        if buffer.samples.is_empty() || buffer.channels == 0 || buffer.sample_rate == 0 {
            return;
        }
        let vel = (velocity.max(1) as f32 / 127.0).clamp(0.0, 1.0);
        let vel_sens = (bank.velocity_sensitivity_pct / 100.0).clamp(0.0, 1.0);
        let gain = (bank.gain_pct / 100.0).clamp(0.0, 2.0) * ((1.0 - vel_sens) + vel_sens * vel);
        let pitch = 2.0_f32.powf(bank.tune_semis / 12.0);
        let step = pitch * buffer.sample_rate as f32 / self.sample_rate as f32;
        let (voice_index, stole_voice) = {
            let pool = &mut self.sample_voices[slot];
            let active = pool.iter().filter(|voice| voice.active).count();
            if active >= MAX_SAMPLE_VOICES_PER_SLOT {
                (Self::steal_active_sample_voice_index(pool), true)
            } else {
                match pool.iter().position(|voice| !voice.active) {
                    Some(i) => (i, false),
                    None => (Self::steal_active_sample_voice_index(pool), true),
                }
            }
        };
        if stole_voice {
            self.record_voice_steal();
        }
        let pool = &mut self.sample_voices[slot];
        pool[voice_index] = SampleVoice {
            active: true,
            sample_slot,
            pos: 0.0,
            step,
            gain,
        };
        self.active_sample_slots[slot] = true;

        self.enforce_voice_budgets();
    }

    pub(super) fn render_sample_voices(
        &mut self,
        slot_out: &mut [f32; INSTRUMENT_SLOT_COUNT],
    ) -> bool {
        let mut active = false;
        for (slot, out) in slot_out.iter_mut().enumerate().take(INSTRUMENT_SLOT_COUNT) {
            if !self.active_sample_slots[slot] {
                continue;
            }
            let Some(bank) = self.sample_banks.get(slot) else {
                self.active_sample_slots[slot] = false;
                continue;
            };
            let mut slot_active = false;
            for voice in self.sample_voices[slot].iter_mut() {
                if !voice.active {
                    continue;
                }
                let Some(Some(buffer)) =
                    bank.slots.get(voice.sample_slot).map(|s| s.buffer.as_ref())
                else {
                    voice.active = false;
                    continue;
                };
                let frames = buffer.samples.len() / buffer.channels as usize;
                if frames == 0 || voice.pos >= frames as f32 {
                    voice.active = false;
                    continue;
                }
                let frame = voice.pos.floor() as usize;
                let frac = voice.pos - frame as f32;
                let next_frame = (frame + 1).min(frames - 1);
                let sample = mono_frame(buffer, frame) * (1.0 - frac)
                    + mono_frame(buffer, next_frame) * frac;
                *out += sample * voice.gain;
                voice.pos += voice.step;
                active = true;
                slot_active = true;
            }
            self.active_sample_slots[slot] = slot_active;
        }
        active
    }

    pub(super) fn render_preview_sample_voices(
        &mut self,
        slot_out: &mut [f32; INSTRUMENT_SLOT_COUNT],
    ) -> bool {
        let mut active = false;
        for voice in self.preview_sample_voices.iter_mut() {
            let frames = voice.buffer.samples.len() / voice.buffer.channels as usize;
            if frames == 0 || voice.pos >= frames as f32 {
                voice.pos = frames as f32;
                continue;
            }
            let frame = voice.pos.floor() as usize;
            let frac = voice.pos - frame as f32;
            let next_frame = (frame + 1).min(frames - 1);
            let sample = mono_frame(&voice.buffer, frame) * (1.0 - frac)
                + mono_frame(&voice.buffer, next_frame) * frac;
            slot_out[voice.slot] += sample * voice.gain;
            voice.pos += voice.step;
            active = true;
        }
        self.preview_sample_voices.retain(|voice| {
            let frames = voice.buffer.samples.len() / voice.buffer.channels as usize;
            frames > 0 && voice.pos < frames as f32
        });
        active || !self.preview_sample_voices.is_empty()
    }
}
