use super::retired_state::{
    store_retired_preview, store_retired_preview_buffer, PREVIEW_AUDITION_SLOTS,
};
use super::source_worker_placement;
use super::*;

impl SynthEngine {
    pub fn set_sample_banks(&mut self, banks: Vec<SampleBankConfig>) -> RetiredAudioState {
        let mut retired = RetiredAudioState::default();
        if !self.sample_voice_pool.has_home() {
            retired.sample_banks = Some(banks);
            return retired;
        }
        retired.sample_banks = Some(std::mem::replace(&mut self.sample_banks, banks));
        self.sample_banks
            .resize(INSTRUMENT_SLOT_COUNT, SampleBankConfig::default());
        if let Some(voices) = self.sample_voice_pool.clear_all() {
            retired.sample_voices = voices;
        }
        retired
    }

    pub fn set_sample_bank(
        &mut self,
        instrument_slot: usize,
        bank: SampleBankConfig,
    ) -> RetiredAudioState {
        let mut retired = RetiredAudioState::default();
        if !self.sample_voice_pool.has_home() {
            retired.sample_bank = Some(bank);
            return retired;
        }
        let slot = instrument_slot.min(INSTRUMENT_SLOT_COUNT - 1);
        self.sample_banks
            .resize(INSTRUMENT_SLOT_COUNT, SampleBankConfig::default());
        retired.sample_bank = Some(std::mem::replace(&mut self.sample_banks[slot], bank));
        if let Some(voices) = self.sample_voice_pool.clear_slot(slot) {
            retired.sample_voices = voices;
        }
        retired
    }

    pub fn preview_sample(
        &mut self,
        instrument_slot: u8,
        buffer: SampleBuffer,
        velocity: u8,
    ) -> RetiredAudioState {
        let mut retired = RetiredAudioState::default();
        #[cfg(feature = "routing-tree-benchmark")]
        if self.routing_tree_assignment.is_some() {
            store_retired_preview_buffer(&mut retired.preview_sample_buffers, buffer);
            self.reject_routing_tree_mutation_for_control();
            return retired;
        }
        let slot = (instrument_slot as usize).min(INSTRUMENT_SLOT_COUNT - 1);
        if buffer.samples.is_empty() || buffer.channels == 0 || buffer.sample_rate == 0 {
            store_retired_preview_buffer(&mut retired.preview_sample_buffers, buffer);
            return retired;
        }
        let (velocity_sensitivity_pct, gain_pct, tune_semis) = self
            .sample_banks
            .get(slot)
            .map(|bank| {
                (
                    bank.velocity_sensitivity_pct,
                    bank.gain_pct,
                    bank.tune_semis,
                )
            })
            .unwrap_or((100.0, 100.0, 0.0));
        let vel = (velocity.max(1) as f32 / 127.0).clamp(0.0, 1.0);
        let vel_sens = (velocity_sensitivity_pct / 100.0).clamp(0.0, 1.0);
        let gain = (gain_pct / 100.0).clamp(0.0, 2.0) * ((1.0 - vel_sens) + vel_sens * vel);
        let pitch = 2.0_f32.powf(tune_semis / 12.0);
        let step = pitch * buffer.sample_rate as f32 / self.sample_rate as f32;
        let order = self.preview_sample_next_order;
        self.preview_sample_next_order = self.preview_sample_next_order.saturating_add(1);
        let preview_slot = self
            .preview_sample_voices
            .iter()
            .position(Option::is_none)
            .unwrap_or_else(|| {
                if self.preview_sample_orders[0] <= self.preview_sample_orders[1] {
                    0
                } else {
                    1
                }
            });
        if let Some(displaced) = self.preview_sample_voices[preview_slot].take() {
            store_retired_preview(&mut retired.preview_sample_voices, displaced);
        }
        self.preview_sample_orders[preview_slot] = order;
        self.preview_sample_voices[preview_slot] = Some(PreviewSampleVoice {
            slot,
            buffer,
            pos: 0.0,
            step,
            gain,
            filt: BiquadState::new(),
        });
        retired
    }

    pub fn note_on(&mut self, instrument_slot: u8, midi_note: u8, velocity: u8, duration_ms: u32) {
        #[cfg(feature = "routing-tree-benchmark")]
        if self.routing_tree_assignment.is_some() {
            self.routing_tree_mark_note_event();
        }
        if !self.voice_pools_home() {
            return;
        }
        let slot = (instrument_slot as usize).min(INSTRUMENT_SLOT_COUNT - 1);
        #[cfg(feature = "routing-tree-benchmark")]
        let event_sample_clock = self
            .routing_tree_source_event_sample_clock
            .unwrap_or(self.sample_clock);
        #[cfg(not(feature = "routing-tree-benchmark"))]
        let event_sample_clock = self.sample_clock;
        if self.slot_kind[slot] == InstrumentKind::Sample {
            self.sample_note_on(slot, midi_note, velocity);
            return;
        }
        if self.slot_kind[slot] != InstrumentKind::Synth {
            return;
        }
        if !self.synth_voice_pool.has_home() {
            return;
        }
        let v = velocity.max(1);
        let duration_samples = ms_to_samples(duration_ms as f32, self.sample_rate).max(1) as u64;
        let note_off_sample = event_sample_clock.saturating_add(duration_samples);
        let freq = midi_note_to_hz(midi_note);
        self.synth_voice_pool.compact_slot_lanes(slot);
        let active = self
            .synth_voice_pool
            .active_count_for_slot(slot)
            .unwrap_or(0);
        let first_inactive_lane = self.synth_voice_pool.first_inactive_lane();
        if self.voice_stealing_mode == VoiceStealingMode::None && first_inactive_lane.is_none() {
            self.record_voice_admission_drop();
            return;
        }
        let victim_lane = if self.voice_stealing_mode == VoiceStealingMode::None {
            None
        } else if active >= MAX_SYNTH_VOICES_PER_SLOT {
            self.steal_active_voice_index(slot)
        } else if first_inactive_lane.is_none() {
            self.find_global_steal_candidate_scored(false)
                .map(|(_, lane)| lane)
        } else {
            None
        };
        let legacy_lane = victim_lane.or(first_inactive_lane).unwrap_or(0);
        let Some(canonical_lane) = victim_lane
            .and_then(|lane| self.synth_voice_pool.canonical_lane(lane))
            .or_else(|| self.synth_voice_pool.first_free_canonical_lane())
        else {
            self.record_voice_admission_drop();
            return;
        };
        let required_worker = source_worker_placement::worker_for_slot(self, slot);
        let lane = if self.source_worker_load.is_some() {
            let inactive_lanes = [
                self.synth_voice_pool.first_inactive_lane_for_parity(0),
                self.synth_voice_pool.first_inactive_lane_for_parity(1),
            ];
            let lane = if let Some(worker) = required_worker {
                source_worker_placement::choose_lane_for_worker(worker, victim_lane, inactive_lanes)
            } else {
                source_worker_placement::choose_lane(
                    self,
                    SOURCE_WORKER_SYNTH_COST_UNITS,
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
        let cfg = self.instruments[slot];
        let amp_env = EnvState::note_on(cfg.amp_env, self.sample_rate);
        let filt_env = EnvState::note_on(cfg.filter_env, self.sample_rate);
        let mut voice = Voice {
            active: true,
            canonical_lane: None,
            instrument_slot: slot as u8,
            midi_note,
            velocity: v,
            velocity_norm: 0.0,
            note_off_sample,
            started_sample: event_sample_clock,
            freq_hz: freq,
            osc1_inc: 0.0,
            osc2_inc: 0.0,
            render_revision: 0,
            phase1: 0.0,
            phase2: 0.0,
            amp_env,
            filt_env,
            filt: BiquadState::new(),
        };
        refresh_synth_voice_render_cache(
            &mut voice,
            &self.synth_render_configs[slot],
            self.sample_rate,
            self.synth_render_revisions[slot],
        );
        if !self.synth_voice_pool.replace_lane_for_admission(
            lane,
            slot,
            victim_lane,
            canonical_lane,
            voice,
        ) {
            self.record_voice_admission_drop();
            return;
        }
        if victim_lane.is_some() {
            self.record_voice_steal();
        }
        self.active_synth_slots[slot] = true;

        self.enforce_voice_budgets();
    }

    pub fn cc(&mut self, instrument_slot: u8, controller: u8, value: u8) {
        let slot = (instrument_slot as usize).min(INSTRUMENT_SLOT_COUNT - 1);
        if self.slot_kind[slot] == InstrumentKind::None {
            return;
        }
        if controller == 74 {
            self.mods[slot].cutoff_cc = (value as f32 / 127.0).clamp(0.0, 1.0);
        } else if controller == 71 {
            self.mods[slot].resonance_cc = (value as f32 / 127.0).clamp(0.0, 1.0);
        }
    }

    pub fn note_off(&mut self, instrument_slot: u8, midi_note: u8) {
        if !self.voice_pools_home() {
            return;
        }
        let slot = (instrument_slot as usize).min(INSTRUMENT_SLOT_COUNT - 1);
        #[cfg(feature = "routing-tree-benchmark")]
        let event_sample_clock = self
            .routing_tree_source_event_sample_clock
            .unwrap_or(self.sample_clock);
        #[cfg(not(feature = "routing-tree-benchmark"))]
        let event_sample_clock = self.sample_clock;
        let cfg = self.instruments[slot];
        if self.synth_voice_pool.compact_slot_lanes(slot) {
            let mut lane_indices = [0; SYNTH_VOICE_LANE_CAPACITY];
            let Some(lanes) = self.synth_voice_pool.slot_lanes(slot) else {
                return;
            };
            let lane_count = lanes.len();
            lane_indices[..lane_count].copy_from_slice(lanes);
            for lane in lane_indices.into_iter().take(lane_count) {
                let Some(voice) = self.synth_voice_pool.lane_mut(lane) else {
                    return;
                };
                if !voice.active || voice.midi_note != midi_note {
                    continue;
                }
                voice.amp_env.begin_release(cfg.amp_env, self.sample_rate);
                voice
                    .filt_env
                    .begin_release(cfg.filter_env, self.sample_rate);
                voice.note_off_sample = event_sample_clock;
            }
        }

        let sample_slot = sample_slot_for_note(midi_note);
        if self.sample_voice_pool.compact_slot_lanes(slot) {
            let mut lane_indices = [0; SAMPLE_VOICE_LANE_CAPACITY];
            let Some(lanes) = self.sample_voice_pool.slot_lanes(slot) else {
                return;
            };
            let lane_count = lanes.len();
            lane_indices[..lane_count].copy_from_slice(lanes);
            for lane in lane_indices.into_iter().take(lane_count) {
                let Some(voice) = self.sample_voice_pool.lane_mut(lane) else {
                    return;
                };
                if voice.active && voice.sample_slot == sample_slot {
                    self.sample_voice_pool.deactivate_lane(lane);
                }
            }
            self.sample_voice_pool.compact_slot_lanes(slot);
            self.active_sample_slots[slot] = self
                .sample_voice_pool
                .active_count_for_slot(slot)
                .unwrap_or(0)
                > 0;
        }
    }

    pub fn all_notes_off(&mut self) -> RetiredAudioState {
        let mut retired = RetiredAudioState::default();
        #[cfg(feature = "routing-tree-benchmark")]
        let event_sample_clock = self
            .routing_tree_source_event_sample_clock
            .unwrap_or(self.sample_clock);
        #[cfg(not(feature = "routing-tree-benchmark"))]
        let event_sample_clock = self.sample_clock;
        if !self.voice_pools_home() {
            return retired;
        }
        for voice in &mut self.preview_sample_voices {
            if let Some(voice) = voice.take() {
                store_retired_preview(&mut retired.preview_sample_voices, voice);
            }
        }
        self.preview_sample_orders = [0; PREVIEW_AUDITION_SLOTS];
        self.preview_sample_next_order = 0;
        if let Some(voices) = self.sample_voice_pool.clear_all() {
            retired.sample_voices = voices;
            self.active_sample_slots = [false; INSTRUMENT_SLOT_COUNT];
        }
        for slot in 0..INSTRUMENT_SLOT_COUNT {
            let cfg = self.instruments[slot];
            if !self.synth_voice_pool.compact_slot_lanes(slot) {
                continue;
            }
            let mut lane_indices = [0; SYNTH_VOICE_LANE_CAPACITY];
            let Some(lanes) = self.synth_voice_pool.slot_lanes(slot) else {
                continue;
            };
            let lane_count = lanes.len();
            lane_indices[..lane_count].copy_from_slice(lanes);
            for lane in lane_indices.into_iter().take(lane_count) {
                let Some(voice) = self.synth_voice_pool.lane_mut(lane) else {
                    continue;
                };
                if voice.active {
                    voice.amp_env.begin_release(cfg.amp_env, self.sample_rate);
                    voice
                        .filt_env
                        .begin_release(cfg.filter_env, self.sample_rate);
                    voice.note_off_sample = event_sample_clock;
                }
            }
        }
        retired
    }
}
