use super::super::types::EnvConfig;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::synth) enum EnvStage {
    Attack,
    Decay,
    Sustain,
    Release,
    Off,
}

#[derive(Clone, Copy, Debug)]
pub(in crate::synth) struct EnvState {
    pub(in crate::synth) stage: EnvStage,
    pub(in crate::synth) level: f32,
    pub(in crate::synth) stage_pos: u32,
    pub(in crate::synth) stage_len: u32,
    pub(in crate::synth) sustain: f32,
    pub(in crate::synth) release_start: f32,
    /// Decay length, kept so the decay stage still runs after a non-zero attack.
    pub(in crate::synth) decay_len: u32,
}

impl EnvState {
    pub(in crate::synth) fn note_on(cfg: EnvConfig, sample_rate: u32) -> Self {
        let a = ms_to_samples(cfg.attack_ms, sample_rate);
        let d = ms_to_samples(cfg.decay_ms, sample_rate);
        let sustain = (cfg.sustain_pct / 100.0).clamp(0.0, 1.0);
        let stage = if a == 0 {
            EnvStage::Decay
        } else {
            EnvStage::Attack
        };
        let stage_len = if stage == EnvStage::Attack { a } else { d };
        Self {
            stage,
            level: if stage == EnvStage::Attack { 0.0 } else { 1.0 },
            stage_pos: 0,
            stage_len,
            sustain,
            release_start: 0.0,
            decay_len: d,
        }
    }

    pub(in crate::synth) fn off() -> Self {
        Self {
            stage: EnvStage::Off,
            level: 0.0,
            stage_pos: 0,
            stage_len: 0,
            sustain: 0.0,
            release_start: 0.0,
            decay_len: 0,
        }
    }

    /// True while the envelope sits at full level and will stay there until released.
    pub(in crate::synth) fn holds_full_level(&self) -> bool {
        self.sustain >= 1.0
            && (self.stage == EnvStage::Sustain
                || (self.stage == EnvStage::Decay && self.stage_len == 0))
    }

    pub(in crate::synth) fn begin_release(&mut self, cfg: EnvConfig, sample_rate: u32) {
        if self.stage == EnvStage::Release || self.stage == EnvStage::Off {
            return;
        }
        self.stage = EnvStage::Release;
        self.stage_pos = 0;
        self.stage_len = ms_to_samples(cfg.release_ms, sample_rate).max(1);
        self.release_start = self.level;
    }

    pub(in crate::synth) fn next(&mut self) -> f32 {
        match self.stage {
            EnvStage::Attack => self.next_attack(),
            EnvStage::Decay => self.next_decay(),
            EnvStage::Sustain => self.level,
            EnvStage::Release => self.next_release(),
            EnvStage::Off => 0.0,
        }
    }

    fn next_attack(&mut self) -> f32 {
        if self.stage_len == 0 {
            self.stage = EnvStage::Decay;
            self.stage_pos = 0;
            self.level = 1.0;
            return self.level;
        }
        let t = (self.stage_pos as f32) / (self.stage_len as f32);
        self.level = t.clamp(0.0, 1.0);
        self.stage_pos = self.stage_pos.saturating_add(1);
        if self.stage_pos >= self.stage_len {
            self.stage = EnvStage::Decay;
            self.stage_pos = 0;
            self.stage_len = self.decay_len;
            self.level = 1.0;
        }
        self.level
    }

    fn next_decay(&mut self) -> f32 {
        if self.stage_len == 0 {
            self.stage = EnvStage::Sustain;
            self.level = self.sustain;
            return self.level;
        }
        let t = (self.stage_pos as f32) / (self.stage_len as f32);
        self.level = (1.0 + (self.sustain - 1.0) * t).clamp(0.0, 1.0);
        self.stage_pos = self.stage_pos.saturating_add(1);
        if self.stage_pos >= self.stage_len {
            self.stage = EnvStage::Sustain;
            self.level = self.sustain;
        }
        self.level
    }

    fn next_release(&mut self) -> f32 {
        let t = (self.stage_pos as f32) / (self.stage_len as f32);
        self.level = (self.release_start * (1.0 - t)).clamp(0.0, 1.0);
        self.stage_pos = self.stage_pos.saturating_add(1);
        if self.stage_pos >= self.stage_len {
            self.stage = EnvStage::Off;
            self.level = 0.0;
        }
        self.level
    }

    pub(in crate::synth) fn is_off(&self) -> bool {
        self.stage == EnvStage::Off
    }

    pub(in crate::synth) fn is_releasing(&self) -> bool {
        self.stage == EnvStage::Release
    }
}

pub(in crate::synth) fn ms_to_samples(ms: f32, sample_rate: u32) -> u32 {
    if ms <= 0.0 {
        return 0;
    }
    ((ms / 1000.0) * (sample_rate as f32)).round().max(0.0) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decay_runs_after_a_non_zero_attack_without_a_jump() {
        let cfg = EnvConfig {
            attack_ms: 5.0,
            decay_ms: 300.0,
            sustain_pct: 70.0,
            release_ms: 350.0,
        };
        let mut env = EnvState::note_on(cfg, 48_000);
        let levels = (0..48_000 / 2).map(|_| env.next()).collect::<Vec<_>>();
        let largest_step = levels
            .windows(2)
            .map(|pair| (pair[1] - pair[0]).abs())
            .fold(0.0_f32, f32::max);
        assert!(largest_step < 0.01, "envelope jumps by {largest_step}");
        let after_attack = 240 + 48 * 150;
        assert!(levels[after_attack] > 0.8 && levels[after_attack] < 0.9);
        assert!((levels[levels.len() - 1] - 0.7).abs() < 1.0e-6);
    }
}
