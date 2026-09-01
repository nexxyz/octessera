use super::inline_source_executor::{render_sample_voice_frame, sample_lowpass};
use super::*;

impl SynthEngine {
    pub(super) fn sample_note_on(&mut self, slot: usize, midi_note: u8, velocity: u8) {
        let sample_slot = sample_slot_for_note(midi_note);
        if !self.sample_voice_pool.has_home() {
            return;
        }
        let Some((
            velocity_sensitivity_pct,
            gain_pct,
            tune_semis,
            filter_cutoff_hz,
            filter_resonance,
            sample_count,
            sample_channels,
            sample_rate,
        )) = self.sample_banks.get(slot).and_then(|bank| {
            bank.slots
                .get(sample_slot)
                .and_then(|slot| slot.buffer.as_ref())
                .map(|buffer| {
                    (
                        bank.velocity_sensitivity_pct,
                        bank.gain_pct,
                        bank.tune_semis,
                        bank.filter_cutoff_hz,
                        bank.filter_resonance,
                        buffer.samples.len(),
                        buffer.channels,
                        buffer.sample_rate,
                    )
                })
        })
        else {
            return;
        };
        if sample_count == 0 || sample_channels == 0 || sample_rate == 0 {
            return;
        }
        let vel = (velocity.max(1) as f32 / 127.0).clamp(0.0, 1.0);
        let vel_sens = (velocity_sensitivity_pct / 100.0).clamp(0.0, 1.0);
        let gain = (gain_pct / 100.0).clamp(0.0, 2.0) * ((1.0 - vel_sens) + vel_sens * vel);
        let pitch = 2.0_f32.powf(tune_semis / 12.0);
        let step = pitch * sample_rate as f32 / self.sample_rate as f32;
        self.sample_voice_pool.compact_slot_lanes(slot);
        let active = self
            .sample_voice_pool
            .active_count_for_slot(slot)
            .unwrap_or(0);
        let lane = if self.voice_stealing_mode == VoiceStealingMode::None {
            let Some(lane) = self.sample_voice_pool.first_inactive_lane() else {
                self.record_voice_admission_drop();
                return;
            };
            lane
        } else if active >= MAX_SAMPLE_VOICES_PER_SLOT {
            self.sample_voice_pool
                .first_active_lane_for_slot(slot)
                .or_else(|| self.sample_voice_pool.first_inactive_lane())
                .or_else(|| {
                    self.sample_voice_pool
                        .first_active_lane_global()
                        .map(|(_, lane)| lane)
                })
                .unwrap_or(0)
        } else {
            self.sample_voice_pool
                .first_inactive_lane()
                .or_else(|| {
                    self.sample_voice_pool
                        .first_active_lane_global()
                        .map(|(_, lane)| lane)
                })
                .unwrap_or(0)
        };
        let voice = SampleVoice {
            active: true,
            instrument_slot: slot as u8,
            sample_slot,
            buffer: None,
            filter_cutoff_hz,
            filter_resonance,
            pos: 0.0,
            step,
            gain,
            filt: BiquadState::new(),
        };
        let stole_voice = match self.sample_voice_pool.replace_lane(
            lane,
            slot,
            voice,
            &mut self.pending_render_retired.sample_voices,
        ) {
            Ok(stole_voice) => stole_voice,
            Err(_voice) => {
                self.record_voice_admission_drop();
                return;
            }
        };
        let Some(target) = self.sample_voice_pool.lane_mut(lane) else {
            self.record_voice_admission_drop();
            return;
        };
        target.buffer = Some(
            self.sample_banks[slot].slots[sample_slot]
                .buffer
                .as_ref()
                .expect("sample bank buffer must remain available")
                .clone(),
        );
        if stole_voice {
            self.record_voice_steal();
        }
        self.active_sample_slots[slot] = true;

        self.enforce_voice_budgets();
    }

    pub(super) fn render_sample_voices(
        &mut self,
        slot_out: &mut [f32; INSTRUMENT_SLOT_COUNT],
    ) -> bool {
        let mut active = false;
        for (slot, out) in slot_out.iter_mut().enumerate().take(INSTRUMENT_SLOT_COUNT) {
            let rendered = self.render_sample_slot(slot);
            *out += rendered.sample;
            active |= rendered.active;
        }
        active
    }

    pub(super) fn render_sample_slot(&mut self, slot: usize) -> SlotFrameOutput {
        if !self.active_sample_slots[slot] {
            return SlotFrameOutput::default();
        }
        let mut out = 0.0;
        let mut slot_active = false;
        if !self.sample_voice_pool.compact_slot_lanes(slot) {
            return SlotFrameOutput::default();
        }
        let mut lane_indices = [0; SAMPLE_VOICE_LANE_CAPACITY];
        let Some(lanes) = self.sample_voice_pool.slot_lanes(slot) else {
            return SlotFrameOutput::default();
        };
        let lane_count = lanes.len();
        lane_indices[..lane_count].copy_from_slice(lanes);
        for lane in lane_indices.into_iter().take(lane_count) {
            let Some(voice) = self.sample_voice_pool.lane_mut(lane) else {
                return SlotFrameOutput::default();
            };
            debug_assert_eq!(voice.instrument_slot as usize, slot);
            if let Some(sample) = render_sample_voice_frame(voice, self.sample_rate) {
                out += sample;
                slot_active = true;
            }
        }
        self.sample_voice_pool.compact_slot_lanes(slot);
        self.active_sample_slots[slot] = slot_active;
        SlotFrameOutput {
            sample: out,
            active: slot_active,
        }
    }

    pub(super) fn render_preview_sample_voices(
        &mut self,
        slot_out: &mut [f32; INSTRUMENT_SLOT_COUNT],
    ) -> bool {
        let mut active = false;
        for voice in self.preview_sample_voices.iter_mut().flatten() {
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
            let bank = self.sample_banks.get(voice.slot);
            let cutoff_hz = bank.map(|bank| bank.filter_cutoff_hz).unwrap_or(8000.0);
            let resonance = bank.map(|bank| bank.filter_resonance).unwrap_or(20.0);
            let filtered = sample_lowpass(
                sample,
                &mut voice.filt,
                cutoff_hz,
                resonance,
                self.sample_rate,
            );
            slot_out[voice.slot] += filtered * voice.gain;
            voice.pos += voice.step;
            active = true;
        }
        for index in 0..self.preview_sample_voices.len() {
            let complete = self.preview_sample_voices[index]
                .as_ref()
                .map(|voice| {
                    let frames = voice.buffer.samples.len() / voice.buffer.channels as usize;
                    frames == 0 || voice.pos >= frames as f32
                })
                .unwrap_or(false);
            if complete {
                let voice = self.preview_sample_voices[index]
                    .take()
                    .expect("completed preview slot must contain a voice");
                self.retire_render_preview(voice);
            }
        }
        active || self.preview_sample_voices.iter().any(Option::is_some)
    }
}
