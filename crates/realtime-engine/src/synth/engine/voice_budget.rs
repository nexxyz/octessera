use super::*;

impl SynthEngine {
    pub(super) fn steal_active_voice_index(pool: &[Voice; VOICES_PER_SLOT]) -> usize {
        let mut best_i = 0;
        let mut best_score = f32::MAX;
        for (i, v) in pool.iter().enumerate() {
            if !v.active {
                continue;
            }
            let score = v.amp_env.level;
            if score < best_score {
                best_score = score;
                best_i = i;
            }
        }
        best_i
    }

    pub(in crate::synth::engine) fn active_synth_voice_total(&self) -> usize {
        self.voices
            .iter()
            .map(|pool| pool.iter().filter(|v| v.active).count())
            .sum()
    }

    pub(in crate::synth::engine) fn active_sample_voice_total(&self) -> usize {
        self.sample_voices
            .iter()
            .map(|pool| pool.iter().filter(|v| v.active).count())
            .sum()
    }

    pub(in crate::synth::engine) fn active_synth_voice_count(&self, slot: usize) -> usize {
        self.voices
            .get(slot)
            .map(|pool| pool.iter().filter(|v| v.active).count())
            .unwrap_or(0)
    }

    pub(in crate::synth::engine) fn active_sample_voice_count(&self, slot: usize) -> usize {
        self.sample_voices
            .get(slot)
            .map(|pool| pool.iter().filter(|v| v.active).count())
            .unwrap_or(0)
    }

    fn global_voice_budget(&self) -> usize {
        let max_voices = INSTRUMENT_SLOT_COUNT * VOICES_PER_SLOT;
        let (target_load, min_budget_pct) = match self.voice_stealing_mode {
            VoiceStealingMode::Off => return max_voices,
            VoiceStealingMode::Lenient => (0.88_f32, 0.75_f32),
            VoiceStealingMode::Balanced => (0.78_f32, 0.60_f32),
            VoiceStealingMode::Aggressive => (0.68_f32, 0.45_f32),
        };
        if self.smoothed_load_ratio <= target_load {
            return max_voices;
        }
        let severity =
            ((self.smoothed_load_ratio - target_load) / (1.20_f32 - target_load)).clamp(0.0, 1.0);
        let min_budget = ((max_voices as f32) * min_budget_pct).round() as usize;
        let budget =
            (max_voices as f32 - severity * ((max_voices - min_budget) as f32)).round() as usize;
        budget.clamp(min_budget.max(1), max_voices)
    }

    pub(super) fn enforce_voice_budgets(&mut self) {
        self.enforce_slot_voice_budgets();
        if self.voice_stealing_mode != VoiceStealingMode::Off {
            self.enforce_global_voice_budget();
            self.enforce_global_sample_budget();
        }
    }

    fn enforce_slot_voice_budgets(&mut self) {
        for slot in 0..INSTRUMENT_SLOT_COUNT {
            while self.active_synth_voice_count(slot) > MAX_SYNTH_VOICES_PER_SLOT {
                let idx = Self::steal_active_voice_index(&self.voices[slot]);
                self.voices[slot][idx].active = false;
                self.record_voice_steal();
            }
            while self.active_sample_voice_count(slot) > MAX_SAMPLE_VOICES_PER_SLOT {
                let idx = Self::steal_active_sample_voice_index(&self.sample_voices[slot]);
                self.sample_voices[slot][idx].active = false;
                self.record_voice_steal();
            }
        }
    }

    pub(super) fn enforce_global_voice_budget(&mut self) {
        let budget = self.global_voice_budget().min(MAX_SYNTH_VOICES);
        while self.active_synth_voice_total() > budget {
            let Some((slot, idx)) = self.find_global_steal_candidate() else {
                break;
            };
            self.voices[slot][idx].active = false;
            self.record_voice_steal();
        }
    }

    fn enforce_global_sample_budget(&mut self) {
        while self.active_sample_voice_total() > MAX_SAMPLE_VOICES {
            let Some((slot, idx)) = self.find_global_sample_steal_candidate() else {
                break;
            };
            self.sample_voices[slot][idx].active = false;
            self.record_voice_steal();
        }
    }

    fn find_global_steal_candidate(&self) -> Option<(usize, usize)> {
        let mut best: Option<(usize, usize, f32)> = None;
        for (slot_idx, pool) in self.voices.iter().enumerate() {
            for (voice_idx, voice) in pool.iter().enumerate() {
                if !voice.active {
                    continue;
                }
                let age_samples = self.sample_clock.saturating_sub(voice.started_sample);
                let age_ms = (age_samples as f32) * 1000.0 / (self.sample_rate as f32);
                let mut score = voice.amp_env.level;
                if voice.amp_env.is_releasing() {
                    score -= 0.5;
                }
                score += (voice.velocity as f32 / 127.0) * 0.2;
                if age_ms < 30.0 {
                    score += 1.0;
                }
                match best {
                    Some((_, _, best_score)) if score >= best_score => {}
                    _ => best = Some((slot_idx, voice_idx, score)),
                }
            }
        }
        best.map(|(s, i, _)| (s, i))
    }

    fn find_global_sample_steal_candidate(&self) -> Option<(usize, usize)> {
        for (slot_idx, pool) in self.sample_voices.iter().enumerate() {
            for (voice_idx, voice) in pool.iter().enumerate() {
                if voice.active {
                    return Some((slot_idx, voice_idx));
                }
            }
        }
        None
    }

    pub(super) fn steal_active_sample_voice_index(pool: &[SampleVoice; VOICES_PER_SLOT]) -> usize {
        for (i, voice) in pool.iter().enumerate() {
            if voice.active {
                return i;
            }
        }
        0
    }
}
