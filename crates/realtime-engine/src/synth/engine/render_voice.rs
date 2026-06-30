use super::*;
use std::f32::consts::PI;

#[derive(Clone, Copy)]
pub(super) struct SynthVoiceRenderConfig {
    osc1: OscRenderConfig,
    osc2: OscRenderConfig,
    amp_velocity_sensitivity: f32,
    amp_gain: f32,
    filter_kind: FilterType,
    filter_cutoff_hz: f32,
    filter_env_amount: f32,
    filter_resonance: f32,
}

#[derive(Clone, Copy)]
struct OscRenderConfig {
    waveform: WaveformId,
    octave_mul: f32,
    detune_mul: f32,
    level: f32,
    pulse_duty: f32,
}

impl SynthVoiceRenderConfig {
    pub(super) fn from_config(cfg: SynthConfig) -> Self {
        Self {
            osc1: OscRenderConfig::from_config(cfg.osc1),
            osc2: OscRenderConfig::from_config(cfg.osc2),
            amp_velocity_sensitivity: (cfg.amp.velocity_sensitivity_pct / 100.0).clamp(0.0, 1.0),
            amp_gain: (cfg.amp.gain_pct / 100.0).clamp(0.0, 1.0),
            filter_kind: cfg.filter.kind,
            filter_cutoff_hz: cfg.filter.cutoff_hz,
            filter_env_amount: (cfg.filter.env_amount_pct / 100.0).clamp(-1.0, 1.0),
            filter_resonance: cfg.filter.resonance,
        }
    }

    pub(super) fn osc_increments(self, base_freq: f32, sample_rate: u32) -> (f32, f32) {
        (
            self.osc1.increment(base_freq, sample_rate),
            self.osc2.increment(base_freq, sample_rate),
        )
    }
}

impl OscRenderConfig {
    fn from_config(cfg: OscConfig) -> Self {
        Self {
            waveform: cfg.waveform,
            octave_mul: 2.0_f32.powi(cfg.octave.clamp(-2, 2)),
            detune_mul: 2.0_f32.powf(cfg.detune_cents.clamp(-1200.0, 1200.0) / 1200.0),
            level: (cfg.level_pct / 100.0).clamp(0.0, 1.0),
            pulse_duty: (cfg.pulse_width_pct / 100.0).clamp(0.05, 0.95),
        }
    }

    fn increment(self, base_freq: f32, sample_rate: u32) -> f32 {
        let freq = base_freq * self.octave_mul * self.detune_mul;
        (freq / (sample_rate as f32)).clamp(0.0, 0.5)
    }
}

pub(super) fn refresh_synth_voice_render_cache(
    voice: &mut Voice,
    cfg: &SynthVoiceRenderConfig,
    sample_rate: u32,
    render_revision: u32,
) {
    voice.velocity_norm = (voice.velocity as f32 / 127.0).clamp(0.0, 1.0);
    let (osc1_inc, osc2_inc) = cfg.osc_increments(voice.freq_hz, sample_rate);
    voice.osc1_inc = osc1_inc;
    voice.osc2_inc = osc2_inc;
    voice.render_revision = render_revision;
}

pub(super) fn render_synth_voice_sample_precomputed(
    sample_rate: u32,
    mods: InstrumentMod,
    cfg: &SynthVoiceRenderConfig,
    v: &mut Voice,
    amp_env: f32,
    filt_env: f32,
) -> f32 {
    let vel_sens = cfg.amp_velocity_sensitivity;
    let vel_gain = (1.0 - vel_sens) + vel_sens * v.velocity_norm;
    let gain = cfg.amp_gain;
    let osc1 = osc_sample_precomputed(cfg.osc1, v.osc1_inc, &mut v.phase1);
    let osc2 = osc_sample_precomputed(cfg.osc2, v.osc2_inc, &mut v.phase2);
    let dry = (osc1 + osc2) * 0.5;
    let cutoff = synth_voice_cutoff(cfg, mods.cutoff_cc, filt_env);
    let q = synth_voice_q(cfg, mods.resonance_cc);
    let filtered = v.filt.process(dry, cfg.filter_kind, cutoff, q, sample_rate);
    filtered * amp_env * vel_gain * gain * 0.35
}

fn osc_sample_precomputed(cfg: OscRenderConfig, inc: f32, phase: &mut f32) -> f32 {
    *phase = (*phase + inc).fract();

    let raw = match cfg.waveform {
        WaveformId::Sine => (2.0 * PI * *phase).sin(),
        WaveformId::Triangle => 4.0 * (*phase - 0.5).abs() - 1.0,
        WaveformId::Saw => 2.0 * *phase - 1.0,
        WaveformId::Square => {
            if *phase < 0.5 {
                1.0
            } else {
                -1.0
            }
        }
        WaveformId::Pulse => {
            if *phase < cfg.pulse_duty {
                1.0
            } else {
                -1.0
            }
        }
    };

    raw * cfg.level
}

fn synth_voice_cutoff(cfg: &SynthVoiceRenderConfig, cutoff_cc: f32, filt_env: f32) -> f32 {
    let cutoff_base = cfg.filter_cutoff_hz;
    let env_amt = cfg.filter_env_amount;
    let cutoff_env = cutoff_base * (1.0 + env_amt * filt_env).max(0.0);
    if cutoff_cc > 0.0 {
        120.0 + cutoff_cc * 15_880.0
    } else {
        cutoff_env
    }
}

fn synth_voice_q(cfg: &SynthVoiceRenderConfig, resonance_cc: f32) -> f32 {
    let resonance = if resonance_cc > 0.0 {
        resonance_cc * 100.0
    } else {
        cfg.filter_resonance
    };
    0.5 + (resonance.clamp(0.0, 100.0) / 100.0) * 11.5
}

#[cfg(test)]
#[path = "render_voice_tests.rs"]
mod render_voice_tests;
