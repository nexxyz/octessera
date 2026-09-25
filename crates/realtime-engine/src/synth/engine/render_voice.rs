use super::*;
use std::f32::consts::{PI, TAU};

#[path = "synth_voice_render.rs"]
pub(super) mod synth_voice_render;

#[derive(Clone, Copy)]
pub(super) struct SynthVoiceRenderConfig {
    osc1: OscRenderConfig,
    osc2: OscRenderConfig,
    amp_velocity_sensitivity: f32,
    amp_gain: f32,
    pub(super) filter_kind: FilterType,
    filter_cutoff_hz: f32,
    filter_env_amount: f32,
    filter_resonance: f32,
    filter_key_tracking: f32,
    pub(super) source: VoiceSource,
    pub(super) source_generation: u32,
}

#[derive(Clone, Copy)]
pub(super) enum VoiceSource {
    Synth,
    Fm {
        ratio: f32,
        index: f32,
        index_env: EnvConfig,
    },
    Pluck {
        decay_ms: f32,
        brightness_pct: f32,
        pick_position_pct: f32,
    },
    Drum,
    Disabled,
}

#[derive(Clone, Copy)]
struct OscRenderConfig {
    waveform: WaveformId,
    octave_mul: f32,
    detune_mul: f32,
    level: f32,
    pulse_duty: f32,
}

#[derive(Clone, Copy)]
pub(super) struct SynthVoiceBlockRenderState {
    pub(super) velocity_gain: f32,
    pub(super) q: f32,
    pub(super) static_cutoff: Option<f32>,
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
            filter_key_tracking: (cfg.filter.key_tracking_pct / 100.0).clamp(0.0, 1.0),
            source: VoiceSource::Synth,
            source_generation: 0,
        }
    }

    pub(super) fn from_fm(cfg: FmConfig) -> Self {
        let mut render = Self::from_config(cfg.common_voice_config());
        render.source = VoiceSource::Fm {
            ratio: cfg.ratio.value(),
            index: cfg.index.min(100) as f32 * 0.04,
            index_env: cfg.index_env,
        };
        render
    }

    pub(super) fn from_pluck(cfg: PluckConfig) -> Self {
        let mut render = Self::from_config(cfg.common_voice_config());
        render.source = VoiceSource::Pluck {
            decay_ms: cfg.decay_ms,
            brightness_pct: cfg.brightness_pct,
            pick_position_pct: cfg.pick_position_pct,
        };
        render
    }

    pub(super) fn from_drum(cfg: &DrumConfig) -> Self {
        let mut render = Self::from_config(cfg.common_voice_config());
        render.source = VoiceSource::Drum;
        render
    }

    pub(super) fn disabled(mut self) -> Self {
        self.source = VoiceSource::Disabled;
        self
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
    let (osc1_inc, osc2_inc) = match cfg.source {
        VoiceSource::Fm { ratio, .. } => (
            (voice.freq_hz / sample_rate as f32).clamp(0.0, 0.5),
            (voice.freq_hz * ratio / sample_rate as f32).clamp(0.0, 0.5),
        ),
        VoiceSource::Pluck { .. } | VoiceSource::Drum => (0.0, 0.0),
        _ => cfg.osc_increments(voice.freq_hz, sample_rate),
    };
    voice.osc1_inc = osc1_inc;
    voice.osc2_inc = osc2_inc;
    voice.filter_key_scale = if cfg.filter_key_tracking == 0.0 {
        1.0
    } else {
        2.0_f32.powf((voice.midi_note as f32 - 60.0) * cfg.filter_key_tracking / 12.0)
    };
    voice.fm_index_limit = if osc2_inc > 0.0 {
        ((0.5 - osc1_inc) / osc2_inc - 1.0).clamp(0.0, 4.0)
    } else {
        0.0
    };
    if let VoiceSource::Pluck {
        decay_ms,
        brightness_pct,
        ..
    } = cfg.source
    {
        voice
            .pluck
            .update_coefficients(voice.freq_hz, decay_ms, brightness_pct);
    }
    voice.render_revision = render_revision;
}

pub(super) fn render_synth_voice_sample_precomputed(
    sample_rate: u32,
    mods: InstrumentMod,
    cfg: &SynthVoiceRenderConfig,
    v: &mut Voice,
    ring: &mut crate::synth::pluck_string::PluckRing,
    amp_env: f32,
    filt_env: f32,
) -> f32 {
    let vel_sens = cfg.amp_velocity_sensitivity;
    let vel_gain = (1.0 - vel_sens) + vel_sens * v.velocity_norm;
    let gain = cfg.amp_gain;
    let dry = source_sample(cfg, v, ring);
    let cutoff = synth_voice_cutoff(
        cfg,
        mods.cutoff_cc,
        filt_env,
        v.filter_key_scale,
        sample_rate,
    );
    let q = synth_voice_q(cfg, mods.resonance_cc);
    let filtered = v.filt.process(dry, cfg.filter_kind, cutoff, q, sample_rate);
    filtered * amp_env * vel_gain * gain * 0.35
}

pub(super) fn prepare_synth_voice_block(
    cfg: &SynthVoiceRenderConfig,
    mods: InstrumentMod,
    velocity_norm: f32,
    key_scale: f32,
    sample_rate: u32,
) -> SynthVoiceBlockRenderState {
    let vel_sens = cfg.amp_velocity_sensitivity;
    let velocity_gain = (1.0 - vel_sens) + vel_sens * velocity_norm;
    let q = synth_voice_q(cfg, mods.resonance_cc);
    let static_cutoff = if mods.cutoff_cc > 0.0 || cfg.filter_env_amount == 0.0 {
        Some(synth_voice_cutoff(
            cfg,
            mods.cutoff_cc,
            0.0,
            key_scale,
            sample_rate,
        ))
    } else {
        None
    };
    SynthVoiceBlockRenderState {
        velocity_gain,
        q,
        static_cutoff,
    }
}

pub(super) fn render_synth_voice_sample_block_precomputed(
    context: &synth_voice_render::SynthVoiceFrameContext,
    v: &mut Voice,
    ring: &mut crate::synth::pluck_string::PluckRing,
    amp_env: f32,
    filt_env: f32,
    block: SynthVoiceBlockRenderState,
) -> f32 {
    let cfg = &context.render_config;
    let dry = source_sample(cfg, v, ring);
    let cutoff = block.static_cutoff.unwrap_or_else(|| {
        synth_voice_cutoff(
            cfg,
            context.mods.cutoff_cc,
            filt_env,
            v.filter_key_scale,
            context.sample_rate,
        )
    });
    let filtered = if block.static_cutoff.is_some() {
        v.filt.process_prepared(dry)
    } else {
        v.filt
            .process(dry, cfg.filter_kind, cutoff, block.q, context.sample_rate)
    };
    filtered * amp_env * block.velocity_gain * cfg.amp_gain * 0.35
}

fn source_sample(
    cfg: &SynthVoiceRenderConfig,
    v: &mut Voice,
    ring: &mut crate::synth::pluck_string::PluckRing,
) -> f32 {
    match cfg.source {
        VoiceSource::Synth => {
            let osc1 = osc_sample_precomputed(cfg.osc1, v.osc1_inc, &mut v.phase1);
            let osc2 = osc_sample_precomputed(cfg.osc2, v.osc2_inc, &mut v.phase2);
            (osc1 + osc2) * 0.5
        }
        VoiceSource::Fm { index, .. } => {
            v.phase1 = (v.phase1 + v.osc1_inc).fract();
            v.phase2 = (v.phase2 + v.osc2_inc).fract();
            (TAU * v.phase1
                + index.min(v.fm_index_limit) * v.index_env.next() * (TAU * v.phase2).sin())
            .sin()
        }
        VoiceSource::Pluck { .. } => v.pluck.next(ring),
        VoiceSource::Drum => v.drum.next(),
        VoiceSource::Disabled => 0.0,
    }
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

fn synth_voice_cutoff(
    cfg: &SynthVoiceRenderConfig,
    cutoff_cc: f32,
    filt_env: f32,
    key_scale: f32,
    sample_rate: u32,
) -> f32 {
    let cutoff_base = cfg.filter_cutoff_hz;
    let env_amt = cfg.filter_env_amount;
    let cutoff_env = cutoff_base * (1.0 + env_amt * filt_env).max(0.0);
    let cutoff = if cutoff_cc > 0.0 {
        120.0 + cutoff_cc * 15_880.0
    } else {
        cutoff_env * key_scale
    };
    cutoff.clamp(20.0, (sample_rate as f32 * 0.49).clamp(20.0, 20_000.0))
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

#[cfg(test)]
#[path = "fm_tests.rs"]
mod fm_tests;

#[cfg(test)]
#[path = "pluck_tests.rs"]
mod pluck_tests;

#[cfg(test)]
#[path = "pluck_worker_tests.rs"]
mod pluck_worker_tests;

#[cfg(test)]
#[path = "drum_tests.rs"]
mod drum_tests;

#[cfg(test)]
#[path = "drum_worker_tests.rs"]
mod drum_worker_tests;
