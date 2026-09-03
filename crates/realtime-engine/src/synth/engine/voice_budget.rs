use super::super::types::SYNTH_VOICE_LANE_CAPACITY;
use super::*;

impl SynthEngine {
    pub(super) fn steal_active_voice_index(&self, slot: usize) -> Option<usize> {
        let mut best_lane = None;
        let mut best_score = f32::MAX;
        let lanes = self.synth_voice_pool.slot_lanes(slot)?;
        for &lane in lanes {
            let voice = self.synth_voice_pool.lane(lane)?;
            if !voice.active {
                continue;
            }
            let score = voice.amp_env.level;
            let canonical_lane = voice.canonical_lane.expect("active canonical lane");
            let best_canonical_lane =
                best_lane.and_then(|best| self.synth_voice_pool.canonical_lane(best));
            if score < best_score
                || (score == best_score
                    && best_canonical_lane.is_none_or(|best| canonical_lane < best))
            {
                best_score = score;
                best_lane = Some(lane);
            }
        }
        best_lane
    }

    pub(in crate::synth::engine) fn active_synth_voice_total(&self) -> usize {
        self.synth_voice_pool.active_total().unwrap_or(0)
    }

    pub(in crate::synth::engine) fn active_sample_voice_total(&self) -> usize {
        self.sample_voice_pool.active_total().unwrap_or(0)
    }

    pub(in crate::synth::engine) fn active_synth_voice_count(&self, slot: usize) -> usize {
        self.synth_voice_pool
            .active_count_for_slot(slot)
            .unwrap_or(0)
    }

    pub(in crate::synth::engine) fn active_sample_voice_count(&self, slot: usize) -> usize {
        self.sample_voice_pool
            .active_count_for_slot(slot)
            .unwrap_or(0)
    }

    pub(super) fn source_worker_active_cost_units(&self) -> [u16; 2] {
        std::array::from_fn(|parity| {
            let synth_units = self
                .synth_voice_pool
                .active_count_for_parity(parity)
                .unwrap_or(0)
                * SOURCE_WORKER_SYNTH_COST_UNITS as usize;
            let sample_units = self
                .sample_voice_pool
                .active_count_for_parity(parity)
                .unwrap_or(0)
                * SOURCE_WORKER_SAMPLE_COST_UNITS as usize;
            (synth_units + sample_units) as u16
        })
    }

    fn global_voice_budget(&self) -> usize {
        let max_voices = SYNTH_VOICE_LANE_CAPACITY;
        let (target_load, min_budget_pct) = match self.voice_stealing_mode {
            VoiceStealingMode::None => return max_voices,
            VoiceStealingMode::Fixed12 => return 12,
            VoiceStealingMode::Fixed16 => return MAX_SYNTH_VOICES,
            VoiceStealingMode::AutoSoft => (0.88_f32, 0.75_f32),
            VoiceStealingMode::AutoBalanced => (0.78_f32, 0.60_f32),
            VoiceStealingMode::AutoHard => (0.68_f32, 0.45_f32),
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
        if self.voice_stealing_mode == VoiceStealingMode::None {
            return;
        }
        self.enforce_slot_voice_budgets();
        self.enforce_global_voice_budget();
        self.enforce_global_sample_budget();
    }

    fn enforce_slot_voice_budgets(&mut self) {
        for slot in 0..INSTRUMENT_SLOT_COUNT {
            while self.active_synth_voice_count(slot) > MAX_SYNTH_VOICES_PER_SLOT {
                let Some(lane) = self.steal_active_voice_index(slot) else {
                    break;
                };
                if !self.synth_voice_pool.deactivate_lane(lane) {
                    break;
                }
                self.record_voice_steal();
            }
            while self.active_sample_voice_count(slot) > MAX_SAMPLE_VOICES_PER_SLOT {
                let Some(lane) = self.sample_voice_pool.first_active_lane_for_slot(slot) else {
                    break;
                };
                if !self.sample_voice_pool.deactivate_lane(lane) {
                    break;
                }
                self.record_voice_steal();
            }
        }
    }

    pub(super) fn enforce_global_voice_budget(&mut self) {
        let budget = self.global_voice_budget();
        while self.active_synth_voice_total() > budget {
            let active_slot_count = self.active_synth_slot_count();
            let fair_share =
                (budget + active_slot_count.saturating_sub(1)) / active_slot_count.max(1);
            let preserve_final_voice = budget >= active_slot_count;
            let candidate = self
                .find_over_share_steal_candidate(fair_share, preserve_final_voice)
                .or_else(|| self.find_global_steal_candidate_scored(preserve_final_voice));
            let Some((_slot, idx)) = candidate else {
                break;
            };
            if !self.synth_voice_pool.deactivate_lane(idx) {
                break;
            }
            self.record_voice_steal();
        }
    }

    fn enforce_global_sample_budget(&mut self) {
        while self.active_sample_voice_total() > MAX_SAMPLE_VOICES {
            let Some((_slot, lane)) = self.sample_voice_pool.first_active_lane_global() else {
                break;
            };
            if !self.sample_voice_pool.deactivate_lane(lane) {
                break;
            }
            self.record_voice_steal();
        }
    }

    pub(super) fn find_global_steal_candidate_scored(
        &self,
        preserve_final_voice: bool,
    ) -> Option<(usize, usize)> {
        let mut best: Option<(usize, usize, f32)> = None;
        for slot_idx in 0..INSTRUMENT_SLOT_COUNT {
            let active_count = self.active_synth_voice_count(slot_idx);
            if preserve_final_voice && active_count <= 1 {
                continue;
            }
            let lanes = self.synth_voice_pool.slot_lanes(slot_idx)?;
            for &voice_idx in lanes {
                let voice = self.synth_voice_pool.lane(voice_idx)?;
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
                let canonical_lane = voice.canonical_lane.expect("active canonical lane");
                match best {
                    Some((_, best_lane, best_score))
                        if score > best_score
                            || (score == best_score
                                && canonical_lane
                                    >= self
                                        .synth_voice_pool
                                        .canonical_lane(best_lane)
                                        .expect("active canonical lane")) => {}
                    _ => best = Some((slot_idx, voice_idx, score)),
                }
            }
        }
        best.map(|(s, i, _)| (s, i))
    }

    fn find_over_share_steal_candidate(
        &self,
        fair_share: usize,
        preserve_final_voice: bool,
    ) -> Option<(usize, usize)> {
        let mut best_slot: Option<(usize, usize)> = None;
        for slot_idx in 0..INSTRUMENT_SLOT_COUNT {
            let active_count = self.active_synth_voice_count(slot_idx);
            if active_count <= fair_share || (preserve_final_voice && active_count <= 1) {
                continue;
            }
            let excess = active_count - fair_share;
            match best_slot {
                Some((_, best_excess)) if excess <= best_excess => {}
                _ => best_slot = Some((slot_idx, excess)),
            }
        }
        best_slot.and_then(|(slot_idx, _)| self.find_slot_steal_candidate(slot_idx))
    }

    fn find_slot_steal_candidate(&self, slot_idx: usize) -> Option<(usize, usize)> {
        let mut best: Option<(usize, f32)> = None;
        let lanes = self.synth_voice_pool.slot_lanes(slot_idx)?;
        for &voice_idx in lanes {
            let voice = self.synth_voice_pool.lane(voice_idx)?;
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
            let canonical_lane = voice.canonical_lane.expect("active canonical lane");
            match best {
                Some((best_lane, best_score))
                    if score > best_score
                        || (score == best_score
                            && canonical_lane
                                >= self
                                    .synth_voice_pool
                                    .canonical_lane(best_lane)
                                    .expect("active canonical lane")) => {}
                _ => best = Some((voice_idx, score)),
            }
        }
        best.map(|(voice_idx, _)| (slot_idx, voice_idx))
    }

    fn active_synth_slot_count(&self) -> usize {
        self.synth_voice_pool
            .active_counts_by_slot()
            .unwrap_or_default()
            .into_iter()
            .filter(|count| *count > 0)
            .count()
    }
}
