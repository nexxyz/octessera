use super::source_lane_renderer::{render_sample_voice_frame, sample_lowpass};
use super::source_worker_placement;
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
        let first_inactive_lane = self.sample_voice_pool.first_inactive_lane();
        if self.voice_stealing_mode == VoiceStealingMode::None && first_inactive_lane.is_none() {
            self.record_voice_admission_drop();
            return;
        }
        let victim_lane = if self.voice_stealing_mode == VoiceStealingMode::None {
            None
        } else if active >= MAX_SAMPLE_VOICES_PER_SLOT {
            self.sample_voice_pool
                .first_active_lane_for_slot(slot)
                .or_else(|| {
                    first_inactive_lane.is_none().then(|| {
                        self.sample_voice_pool
                            .first_active_lane_global()
                            .map(|(_, lane)| lane)
                    })?
                })
        } else if first_inactive_lane.is_none() {
            self.sample_voice_pool
                .first_active_lane_global()
                .map(|(_, lane)| lane)
        } else {
            None
        };
        let legacy_lane = victim_lane.or(first_inactive_lane).unwrap_or(0);
        let Some(canonical_lane) = victim_lane
            .and_then(|lane| self.sample_voice_pool.canonical_lane(lane))
            .or_else(|| self.sample_voice_pool.first_free_canonical_lane())
        else {
            self.record_voice_admission_drop();
            return;
        };
        #[cfg(feature = "routing-tree-benchmark")]
        let routing_control = self.routing_tree_assignment.is_some()
            && self.routing_tree_source_event_sample_clock.is_some();
        #[cfg(not(feature = "routing-tree-benchmark"))]
        let routing_control = false;
        let required_worker = source_worker_placement::worker_for_slot(self, slot);
        let lane = if routing_control {
            legacy_lane
        } else if self.source_worker_load.is_some() {
            let inactive_lanes = [
                self.sample_voice_pool.first_inactive_lane_for_parity(0),
                self.sample_voice_pool.first_inactive_lane_for_parity(1),
            ];
            let lane = if let Some(worker) = required_worker {
                source_worker_placement::choose_lane_for_worker(worker, victim_lane, inactive_lanes)
            } else {
                source_worker_placement::choose_lane(
                    self,
                    SOURCE_WORKER_SAMPLE_COST_UNITS,
                    victim_lane,
                    inactive_lanes,
                )
            };
            let Some(lane) = lane else {
                #[cfg(feature = "routing-tree-benchmark")]
                if required_worker.is_some() && first_inactive_lane.is_some() {
                    self.reject_routing_tree_mutation_for_control();
                } else {
                    self.record_voice_admission_drop();
                }
                #[cfg(not(feature = "routing-tree-benchmark"))]
                self.record_voice_admission_drop();
                return;
            };
            lane
        } else {
            legacy_lane
        };
        let voice = SampleVoice {
            active: true,
            canonical_lane: None,
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
        if self
            .sample_voice_pool
            .replace_lane_for_admission(
                lane,
                slot,
                victim_lane,
                canonical_lane,
                voice,
                &mut self.pending_render_retired.sample_voices,
            )
            .is_err()
        {
            self.record_voice_admission_drop();
            return;
        }
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
        if victim_lane.is_some() {
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
        let filters = std::array::from_fn(|slot| {
            self.sample_banks
                .get(slot)
                .map(|bank| (bank.filter_cutoff_hz, bank.filter_resonance))
                .unwrap_or((8000.0, 20.0))
        });
        let (completed, active) = render_preview_sample_voices_frame_into(
            &mut self.preview_sample_voices,
            &filters,
            self.sample_rate,
            1,
            slot_out,
        );
        for voice in completed.into_iter().flatten() {
            self.retire_render_preview(voice);
        }
        active
    }
}

#[cfg(feature = "routing-tree-benchmark")]
pub(super) fn render_preview_sample_voices_block_into(
    voices: &mut [Option<PreviewSampleVoice>; PREVIEW_AUDITION_SLOTS],
    filters: &[(f32, f32); INSTRUMENT_SLOT_COUNT],
    sample_rate: u32,
    frames: usize,
    slot_out: &mut [Vec<f32>; INSTRUMENT_SLOT_COUNT],
    active_frames: &mut [bool],
) -> [Option<PreviewSampleVoice>; PREVIEW_AUDITION_SLOTS] {
    let mut frame = 0;
    while frame < frames {
        active_frames[frame] =
            render_preview_sample_voices_frame(voices, filters, sample_rate, |slot, sample| {
                slot_out[slot][frame] += sample
            });
        frame += 1;
    }
    let mut completed = std::array::from_fn(|_| None);
    for index in 0..voices.len() {
        let complete = voices[index]
            .as_ref()
            .map(|voice| {
                let frames = voice.buffer.samples.len() / voice.buffer.channels.max(1) as usize;
                frames == 0 || voice.pos >= frames as f32
            })
            .unwrap_or(false);
        if complete {
            completed[index] = voices[index].take();
        }
    }
    completed
}

fn render_preview_sample_voices_frame(
    voices: &mut [Option<PreviewSampleVoice>; PREVIEW_AUDITION_SLOTS],
    filters: &[(f32, f32); INSTRUMENT_SLOT_COUNT],
    sample_rate: u32,
    mut write: impl FnMut(usize, f32),
) -> bool {
    let mut active = false;
    for voice in voices.iter_mut().flatten() {
        let voice_frames = voice.buffer.samples.len() / voice.buffer.channels.max(1) as usize;
        if voice_frames == 0
            || voice.pos >= voice_frames as f32
            || voice.slot >= INSTRUMENT_SLOT_COUNT
        {
            voice.pos = voice_frames as f32;
            continue;
        }
        let source_frame = voice.pos.floor() as usize;
        let frac = voice.pos - source_frame as f32;
        let next_frame = (source_frame + 1).min(voice_frames - 1);
        let sample = mono_frame(&voice.buffer, source_frame) * (1.0 - frac)
            + mono_frame(&voice.buffer, next_frame) * frac;
        let (cutoff_hz, resonance) = filters[voice.slot];
        let filtered = sample_lowpass(sample, &mut voice.filt, cutoff_hz, resonance, sample_rate);
        write(voice.slot, filtered * voice.gain);
        voice.pos += voice.step;
        active = true;
    }
    active
}

pub(super) fn render_preview_sample_voices_frame_into(
    voices: &mut [Option<PreviewSampleVoice>; PREVIEW_AUDITION_SLOTS],
    filters: &[(f32, f32); INSTRUMENT_SLOT_COUNT],
    sample_rate: u32,
    frames: usize,
    slot_out: &mut [f32; INSTRUMENT_SLOT_COUNT],
) -> ([Option<PreviewSampleVoice>; PREVIEW_AUDITION_SLOTS], bool) {
    let mut active = false;
    for _ in 0..frames {
        active |=
            render_preview_sample_voices_frame(voices, filters, sample_rate, |slot, sample| {
                slot_out[slot] += sample
            });
    }
    let mut completed = std::array::from_fn(|_| None);
    for index in 0..voices.len() {
        let complete = voices[index]
            .as_ref()
            .map(|voice| {
                let frames = voice.buffer.samples.len() / voice.buffer.channels.max(1) as usize;
                frames == 0 || voice.pos >= frames as f32
            })
            .unwrap_or(false);
        if complete {
            completed[index] = voices[index].take();
        }
    }
    (completed, active || voices.iter().any(Option::is_some))
}
