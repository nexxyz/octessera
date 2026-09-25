use super::drum_config::{DrumSound, DrumVoiceConfig};
use std::f32::consts::TAU;

#[derive(Clone, Copy, Debug)]
pub(super) struct DrumState {
    pub(super) sound: DrumSound,
    pub(super) phase1: f32,
    pub(super) phase2: f32,
    pub(super) inc1: f32,
    pub(super) inc2: f32,
    pub(super) sweep_step: f32,
    pub(super) sweep_remaining: u32,
    pub(super) noise_alpha: f32,
    pub(super) noise_state: f32,
    pub(super) noise_mix: f32,
    pub(super) upper_mix: f32,
    pub(super) level: f32,
    pub(super) decay_step: f32,
    pub(super) elapsed: u32,
    pub(super) attack_frames: u32,
    pub(super) burst_frames: u32,
    pub(super) seed: u32,
}

impl DrumState {
    pub(super) fn off() -> Self {
        Self {
            sound: DrumSound::Kick,
            phase1: 0.0,
            phase2: 0.0,
            inc1: 0.0,
            inc2: 0.0,
            sweep_step: 1.0,
            sweep_remaining: 0,
            noise_alpha: 0.0,
            noise_state: 0.0,
            noise_mix: 0.0,
            upper_mix: 0.0,
            level: 0.0,
            decay_step: 0.0,
            elapsed: 0,
            attack_frames: 0,
            burst_frames: 0,
            seed: 1,
        }
    }

    pub(super) fn note_on(
        voice: DrumVoiceConfig,
        local_tune: i8,
        velocity: u8,
        lane: usize,
        sample_rate: u32,
    ) -> Self {
        let note = voice.sound.base_midi()
            + i32::from(voice.tune_semis.clamp(-12, 12))
            + i32::from(local_tune.clamp(-24, 24));
        let hz = (440.0_f32 * 2.0_f32.powf((note as f32 - 69.0) / 12.0))
            .clamp(1.0, sample_rate as f32 * 0.45);
        let (sweep_st, sweep_ms) = voice.sound.sweep();
        let sweep_ratio = 2.0_f32.powf(sweep_st / 12.0);
        let sweep_frames = (sweep_ms * sample_rate as f32 * 0.001) as u32;
        let first_hz = (hz * sweep_ratio).min(sample_rate as f32 * 0.45);
        let (noise_mix, upper_mix) = voice.sound.source_mix();
        let tone = voice.tone_pct.clamp(0.0, 100.0) * 0.01;
        let decay_frames =
            (voice.decay_ms.clamp(20.0, 2_000.0) * sample_rate as f32 * 0.001).max(1.0);
        let mut result = Self {
            sound: voice.sound,
            inc1: first_hz / sample_rate as f32,
            inc2: if first_hz * 1.59 < sample_rate as f32 * 0.45 {
                first_hz * 1.59 / sample_rate as f32
            } else {
                0.0
            },
            sweep_step: if sweep_frames > 0 {
                (hz / first_hz).powf(1.0 / sweep_frames as f32)
            } else {
                1.0
            },
            sweep_remaining: sweep_frames,
            noise_alpha: (TAU * (400.0 + tone * 11_600.0) / sample_rate as f32).min(1.0),
            noise_mix,
            upper_mix: upper_mix * (0.2 + 0.8 * tone),
            level: 1.0,
            decay_step: (-6.907_755 / decay_frames).exp(),
            attack_frames: (voice.attack_ms.clamp(0.0, 50.0) * sample_rate as f32 * 0.001) as u32,
            burst_frames: (sample_rate as f32 * 0.012) as u32,
            seed: (lane as u32).wrapping_mul(0x9e37_79b9)
                ^ (note as u32).wrapping_mul(0x85eb_ca6b)
                ^ (velocity as u32).wrapping_mul(0xc2b2_ae35)
                ^ 0x6d2b_79f5,
            ..Self::off()
        };
        if result.seed == 0 {
            result.seed = 1;
        }
        result
    }

    pub(super) fn next(&mut self) -> f32 {
        self.phase1 = (self.phase1 + self.inc1).fract();
        self.phase2 = (self.phase2 + self.inc2).fract();
        if self.sweep_remaining > 0 {
            self.inc1 *= self.sweep_step;
            self.inc2 *= self.sweep_step;
            self.sweep_remaining -= 1;
        }
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 17;
        self.seed ^= self.seed << 5;
        let noise = self.seed as f32 / u32::MAX as f32 * 2.0 - 1.0;
        self.noise_state += (noise - self.noise_state) * self.noise_alpha;
        let noise_gate = if self.sound == DrumSound::Clap {
            let burst = self.elapsed % self.burst_frames.max(1);
            if self.elapsed < self.burst_frames * 3 && burst < self.burst_frames / 2 {
                1.0
            } else if self.elapsed < self.burst_frames * 3 {
                0.0
            } else {
                0.4
            }
        } else {
            1.0
        };
        let modal = (TAU * self.phase1).sin()
            + if self.inc2 > 0.0 {
                (TAU * self.phase2).sin() * self.upper_mix
            } else {
                0.0
            };
        let onset = if self.attack_frames == 0 {
            1.0
        } else {
            ((self.elapsed + 1) as f32 / self.attack_frames as f32).min(1.0)
        };
        let output = (modal * (1.0 - self.noise_mix)
            + self.noise_state * self.noise_mix * noise_gate)
            * self.level
            * onset
            * 0.6;
        self.level *= self.decay_step;
        self.elapsed = self.elapsed.saturating_add(1);
        output
    }
}
