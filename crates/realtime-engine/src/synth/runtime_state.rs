use super::types::{EnvConfig, FilterType};
use std::f32::consts::PI;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum EnvStage {
    Attack,
    Decay,
    Sustain,
    Release,
    Off,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct EnvState {
    pub(super) stage: EnvStage,
    pub(super) level: f32,
    pub(super) stage_pos: u32,
    pub(super) stage_len: u32,
    pub(super) sustain: f32,
    pub(super) release_start: f32,
}

impl EnvState {
    pub(super) fn note_on(cfg: EnvConfig, sample_rate: u32) -> Self {
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
        }
    }

    pub(super) fn begin_release(&mut self, cfg: EnvConfig, sample_rate: u32) {
        if self.stage == EnvStage::Release || self.stage == EnvStage::Off {
            return;
        }
        self.stage = EnvStage::Release;
        self.stage_pos = 0;
        self.stage_len = ms_to_samples(cfg.release_ms, sample_rate).max(1);
        self.release_start = self.level;
    }

    pub(super) fn next(&mut self) -> f32 {
        match self.stage {
            EnvStage::Attack => {
                if self.stage_len == 0 {
                    self.stage = EnvStage::Decay;
                    self.stage_pos = 0;
                    self.stage_len = 0;
                    self.level = 1.0;
                    return self.level;
                }
                let t = (self.stage_pos as f32) / (self.stage_len as f32);
                self.level = t.clamp(0.0, 1.0);
                self.stage_pos = self.stage_pos.saturating_add(1);
                if self.stage_pos >= self.stage_len {
                    self.stage = EnvStage::Decay;
                    self.stage_pos = 0;
                    self.stage_len = 0;
                    self.level = 1.0;
                }
                self.level
            }
            EnvStage::Decay => {
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
            EnvStage::Sustain => self.level,
            EnvStage::Release => {
                let t = (self.stage_pos as f32) / (self.stage_len as f32);
                self.level = (self.release_start * (1.0 - t)).clamp(0.0, 1.0);
                self.stage_pos = self.stage_pos.saturating_add(1);
                if self.stage_pos >= self.stage_len {
                    self.stage = EnvStage::Off;
                    self.level = 0.0;
                }
                self.level
            }
            EnvStage::Off => 0.0,
        }
    }

    pub(super) fn is_off(&self) -> bool {
        self.stage == EnvStage::Off
    }
    pub(super) fn is_releasing(&self) -> bool {
        self.stage == EnvStage::Release
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct BiquadCoeffs {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct BiquadState {
    pub(super) x1: f32,
    pub(super) x2: f32,
    pub(super) y1: f32,
    pub(super) y2: f32,
    mode: FilterType,
    cutoff_hz: f32,
    q: f32,
    sample_rate: u32,
    coeffs: BiquadCoeffs,
}

impl BiquadState {
    pub(super) fn new() -> Self {
        Self {
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
            mode: FilterType::Lowpass,
            cutoff_hz: 0.0,
            q: 0.0,
            sample_rate: 0,
            coeffs: BiquadCoeffs {
                b0: 1.0,
                b1: 0.0,
                b2: 0.0,
                a1: 0.0,
                a2: 0.0,
            },
        }
    }

    pub(super) fn process(
        &mut self,
        x: f32,
        mode: FilterType,
        cutoff_hz: f32,
        q: f32,
        sample_rate: u32,
    ) -> f32 {
        let cutoff = cutoff_hz.clamp(20.0, 20_000.0);
        let qv = q.clamp(0.25, 20.0);
        if self.mode != mode
            || self.cutoff_hz != cutoff
            || self.q != qv
            || self.sample_rate != sample_rate
        {
            self.mode = mode;
            self.cutoff_hz = cutoff;
            self.q = qv;
            self.sample_rate = sample_rate;
            self.coeffs = biquad_coeffs(mode, cutoff, qv, sample_rate);
        }

        let y = self.coeffs.b0 * x + self.coeffs.b1 * self.x1 + self.coeffs.b2 * self.x2
            - self.coeffs.a1 * self.y1
            - self.coeffs.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }
}

fn biquad_coeffs(mode: FilterType, cutoff: f32, qv: f32, sample_rate: u32) -> BiquadCoeffs {
    let w0 = 2.0 * PI * cutoff / (sample_rate as f32);
    let cos_w0 = w0.cos();
    let sin_w0 = w0.sin();
    let alpha = sin_w0 / (2.0 * qv);

    let (b0, b1, b2, a0, a1, a2) = match mode {
        FilterType::Lowpass => (
            (1.0 - cos_w0) * 0.5,
            1.0 - cos_w0,
            (1.0 - cos_w0) * 0.5,
            1.0 + alpha,
            -2.0 * cos_w0,
            1.0 - alpha,
        ),
        FilterType::Highpass => (
            (1.0 + cos_w0) * 0.5,
            -(1.0 + cos_w0),
            (1.0 + cos_w0) * 0.5,
            1.0 + alpha,
            -2.0 * cos_w0,
            1.0 - alpha,
        ),
        FilterType::Bandpass => (alpha, 0.0, -alpha, 1.0 + alpha, -2.0 * cos_w0, 1.0 - alpha),
        FilterType::Notch => (
            1.0,
            -2.0 * cos_w0,
            1.0,
            1.0 + alpha,
            -2.0 * cos_w0,
            1.0 - alpha,
        ),
    };

    let nb0 = b0 / a0;
    let nb1 = b1 / a0;
    let nb2 = b2 / a0;
    let na1 = a1 / a0;
    let na2 = a2 / a0;

    BiquadCoeffs {
        b0: nb0,
        b1: nb1,
        b2: nb2,
        a1: na1,
        a2: na2,
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Voice {
    pub(super) active: bool,
    pub(super) instrument_slot: u8,
    pub(super) midi_note: u8,
    pub(super) velocity: u8,
    pub(super) velocity_norm: f32,
    pub(super) note_off_sample: u64,
    pub(super) started_sample: u64,
    pub(super) freq_hz: f32,
    pub(super) osc1_inc: f32,
    pub(super) osc2_inc: f32,
    pub(super) render_revision: u32,
    pub(super) phase1: f32,
    pub(super) phase2: f32,
    pub(super) amp_env: EnvState,
    pub(super) filt_env: EnvState,
    pub(super) filt: BiquadState,
}

impl Voice {
    pub(super) fn off() -> Self {
        Self {
            active: false,
            instrument_slot: 0,
            midi_note: 0,
            velocity: 0,
            velocity_norm: 0.0,
            note_off_sample: 0,
            started_sample: 0,
            freq_hz: 440.0,
            osc1_inc: 0.0,
            osc2_inc: 0.0,
            render_revision: 0,
            phase1: 0.0,
            phase2: 0.0,
            amp_env: EnvState {
                stage: EnvStage::Off,
                level: 0.0,
                stage_pos: 0,
                stage_len: 0,
                sustain: 0.0,
                release_start: 0.0,
            },
            filt_env: EnvState {
                stage: EnvStage::Off,
                level: 0.0,
                stage_pos: 0,
                stage_len: 0,
                sustain: 0.0,
                release_start: 0.0,
            },
            filt: BiquadState::new(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct InstrumentMod {
    pub(super) cutoff_cc: f32,
    pub(super) resonance_cc: f32,
}

impl InstrumentMod {
    pub(super) fn new() -> Self {
        Self {
            cutoff_cc: 0.0,
            resonance_cc: 0.0,
        }
    }
}

pub(super) fn ms_to_samples(ms: f32, sample_rate: u32) -> u32 {
    if ms <= 0.0 {
        return 0;
    }
    ((ms / 1000.0) * (sample_rate as f32)).round().max(0.0) as u32
}
