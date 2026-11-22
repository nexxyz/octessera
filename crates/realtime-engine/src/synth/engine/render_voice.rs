use super::*;
use crate::sine::sin_turns;
use std::f32::consts::TAU;

mod oscillator;
pub(super) mod synth_voice_render;

use oscillator::osc_sample_precomputed;

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
        base_ratio: f32,
        effective_ratio: f32,
        ratio_fine_cents: i16,
        index: f32,
        velocity_to_index: f32,
        mod_shape: f32,
        mod_mix: f32,
        index_env: EnvConfig,
    },
    Pluck {
        settings: crate::synth::pluck_string::PluckStringSettings,
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
    /// How far each control-rate filter update moves the voice towards a new CC74 position.
    pub(super) cutoff_cc_step: f32,
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
            base_ratio: cfg.ratio.value(),
            effective_ratio: if cfg.ratio_fine_cents == 0 {
                cfg.ratio.value()
            } else {
                cfg.ratio.value() * 2.0_f32.powf(cfg.ratio_fine_cents as f32 / 1200.0)
            },
            ratio_fine_cents: cfg.ratio_fine_cents,
            index: cfg.index.min(100) as f32 * 0.04,
            velocity_to_index: cfg.velocity_to_index_pct as f32 * 0.01,
            mod_shape: cfg.mod_shape_pct as f32 * 0.01,
            mod_mix: cfg.mod_mix_pct as f32 * 0.01,
            index_env: cfg.index_env,
        };
        render
    }

    pub(super) fn from_pluck(cfg: PluckConfig) -> Self {
        let mut render = Self::from_config(cfg.common_voice_config());
        render.source = VoiceSource::Pluck {
            settings: cfg.into(),
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

    pub(super) fn levels(&self) -> [f32; 3] {
        [self.osc1.level, self.osc2.level, self.amp_gain]
    }

    #[inline(always)]
    pub(super) fn set_osc_levels(&mut self, osc1: f32, osc2: f32) {
        self.osc1.level = osc1;
        self.osc2.level = osc2;
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
        VoiceSource::Fm {
            effective_ratio, ..
        } => (
            (voice.freq_hz / sample_rate as f32).clamp(0.0, 0.5),
            (voice.freq_hz * effective_ratio / sample_rate as f32).clamp(0.0, 0.5),
        ),
        VoiceSource::Pluck { .. } | VoiceSource::Drum => (0.0, 0.0),
        VoiceSource::Synth | VoiceSource::Disabled => {
            cfg.osc_increments(voice.freq_hz, sample_rate)
        }
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
    if let VoiceSource::Fm {
        effective_ratio,
        index,
        velocity_to_index,
        mod_shape,
        mod_mix,
        ..
    } = cfg.source
    {
        let velocity_scale = if velocity_to_index == 0.0 {
            1.0
        } else {
            (1.0 - velocity_to_index) + velocity_to_index * voice.velocity_norm
        };
        let limited_index = (index * velocity_scale).min(voice.fm_index_limit);
        voice.fm_modulator_raw_inc = voice.freq_hz * effective_ratio / sample_rate as f32;
        let shape = mod_shape * 0.5;
        let upper = osc1_inc
            + 2.0 * voice.fm_modulator_raw_inc
            + limited_index * voice.fm_modulator_raw_inc * (1.0 + shape);
        let shaped = shape * crate::synth::engine::fm_render::fade(upper);
        voice.fm_index_fundamental = limited_index * (1.0 - shaped);
        voice.fm_index_second = limited_index * shaped;
        voice.fm_direct_mix =
            mod_mix * crate::synth::engine::fm_render::fade(voice.fm_modulator_raw_inc);
        voice.fm_normalization = 1.0 / (1.0 + voice.fm_direct_mix);
        voice.fm_neutral = velocity_to_index == 0.0 && mod_shape == 0.0 && mod_mix == 0.0;
    }
    if let VoiceSource::Pluck { settings } = cfg.source {
        voice
            .pluck
            .update_coefficients(voice.freq_hz, settings.decay_ms, settings.brightness_pct);
        voice.pluck.retune(voice.freq_hz, sample_rate);
    }
    voice.render_revision = render_revision;
}

#[cfg(test)]
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
    let q = synth_voice_q(cfg, mods.resonance_cc);
    let cutoff_cc = mods.cutoff_cc.map(|target| (target, 0.0));
    let filtered = control_rate_filter(cfg, cutoff_cc, v, filt_env, q, sample_rate, dry);
    filtered * amp_env * vel_gain * gain * 0.35
}

/// Filter cutoff follows its envelope at control rate: a new cutoff and coefficient set every
/// `FILTER_CONTROL_FRAMES` samples (about 0.3 ms) instead of a `sin`/`cos` pair every sample.
const FILTER_CONTROL_FRAMES: u8 = 16;

/// `cutoff_cc` is the CC74 target and the step each update glides towards it.
fn control_rate_filter(
    cfg: &SynthVoiceRenderConfig,
    cutoff_cc: Option<(f32, f32)>,
    v: &mut Voice,
    filt_env: f32,
    q: f32,
    sample_rate: u32,
    dry: f32,
) -> f32 {
    if v.filter_control_countdown == 0 {
        let cutoff_cc =
            cutoff_cc.map(|(target, step)| glide_cutoff_cc(&mut v.filter_cc, target, step));
        let cutoff = synth_voice_cutoff(cfg, cutoff_cc, filt_env, v.filter_key_scale, sample_rate);
        v.filt.prepare(cfg.filter_kind, cutoff, q, sample_rate);
        v.filter_control_countdown = FILTER_CONTROL_FRAMES - 1;
    } else {
        v.filter_control_countdown -= 1;
    }
    v.filt.process_prepared(dry)
}

/// Moves `current` one control update towards `target`; a zero step, or a voice that has no CC
/// position yet, lands on the target at once.
fn glide_cutoff_cc(current: &mut f32, target: f32, step: f32) -> f32 {
    let next = *current + step;
    *current = if step == 0.0 || !next.is_finite() || (step > 0.0) == (next >= target) {
        target
    } else {
        next
    };
    *current
}

/// Prepares one block of a voice. A CC74 move since its last block glides across the block's
/// control-rate filter updates; a settled CC keeps the one static cutoff per block.
pub(super) fn prepare_synth_voice_block(
    cfg: &SynthVoiceRenderConfig,
    mods: InstrumentMod,
    v: &mut Voice,
    frames: usize,
    sample_rate: u32,
) -> SynthVoiceBlockRenderState {
    let vel_sens = cfg.amp_velocity_sensitivity;
    let velocity_gain = (1.0 - vel_sens) + vel_sens * v.velocity_norm;
    let q = synth_voice_q(cfg, mods.resonance_cc);
    let cutoff_cc_step = match mods.cutoff_cc {
        Some(target) if v.filter_cc.is_finite() && v.filter_cc != target => {
            (target - v.filter_cc) * f32::from(FILTER_CONTROL_FRAMES) / frames.max(1) as f32
        }
        Some(target) => {
            v.filter_cc = target;
            0.0
        }
        None => 0.0,
    };
    let static_cutoff = (cutoff_cc_step == 0.0 && cfg.filter_env_amount == 0.0)
        .then(|| synth_voice_cutoff(cfg, mods.cutoff_cc, 0.0, v.filter_key_scale, sample_rate));
    SynthVoiceBlockRenderState {
        velocity_gain,
        q,
        static_cutoff,
        cutoff_cc_step,
    }
}

#[inline(always)]
pub(super) fn shape_synth_voice_block_sample(
    context: &synth_voice_render::SynthVoiceFrameContext,
    v: &mut Voice,
    dry: f32,
    amp_env: f32,
    filt_env: f32,
    block: SynthVoiceBlockRenderState,
    amp_gain: f32,
) -> f32 {
    let cfg = &context.render_config;
    let filtered = if block.static_cutoff.is_some() {
        v.filt.process_prepared(dry)
    } else {
        control_rate_filter(
            cfg,
            context
                .mods
                .cutoff_cc
                .map(|target| (target, block.cutoff_cc_step)),
            v,
            filt_env,
            block.q,
            context.sample_rate,
            dry,
        )
    };
    filtered * amp_env * block.velocity_gain * amp_gain * 0.35
}

/// Single-frame dispatch; block rendering picks one of these per voice instead (see
/// `synth_voice_render`), so each instrument gets its own compiled loop.
#[cfg(test)]
fn source_sample(
    cfg: &SynthVoiceRenderConfig,
    v: &mut Voice,
    ring: &mut crate::synth::pluck_string::PluckRing,
) -> f32 {
    match cfg.source {
        VoiceSource::Synth => synth_source_sample(cfg, v),
        VoiceSource::Fm { index, .. } => fm_source_sample(index, v),
        VoiceSource::Pluck { .. } => pluck_source_sample(v, ring),
        VoiceSource::Drum => v.drum.next(),
        VoiceSource::Disabled => 0.0,
    }
}

#[inline(always)]
pub(super) fn synth_source_sample(cfg: &SynthVoiceRenderConfig, v: &mut Voice) -> f32 {
    let osc1 = osc_sample_precomputed(cfg.osc1, v.osc1_inc, &mut v.phase1);
    let osc2 = osc_sample_precomputed(cfg.osc2, v.osc2_inc, &mut v.phase2);
    (osc1 + osc2) * 0.5
}

#[inline(always)]
pub(super) fn fm_source_sample(index: f32, v: &mut Voice) -> f32 {
    v.phase1 = (v.phase1 + v.osc1_inc).fract();
    v.phase2 = (v.phase2 + v.osc2_inc).fract();
    let modulator = sin_turns(v.phase2);
    if v.fm_neutral {
        sin_turns(
            v.phase1 + index.min(v.fm_index_limit) * v.index_env.next() * modulator * (1.0 / TAU),
        )
    } else {
        let env = v.index_env.next();
        crate::synth::engine::fm_render::sample(v.phase1, v.phase2, modulator, env, v)
    }
}

#[inline(always)]
pub(super) fn pluck_source_sample(
    v: &mut Voice,
    ring: &mut crate::synth::pluck_string::PluckRing,
) -> f32 {
    let dry = v.pluck.next(ring);
    if v.pluck.body_amount == 0.0 {
        dry
    } else {
        dry + 0.5 * v.pluck.body_amount * v.pluck.body.process_prepared(dry)
    }
}

fn synth_voice_cutoff(
    cfg: &SynthVoiceRenderConfig,
    cutoff_cc: Option<f32>,
    filt_env: f32,
    key_scale: f32,
    sample_rate: u32,
) -> f32 {
    let cutoff_env = filter_env_cutoff_hz(cfg.filter_cutoff_hz, cfg.filter_env_amount, filt_env);
    let cutoff = match cutoff_cc {
        Some(cutoff_cc) => cutoff_env * key_scale * cc_cutoff_ratio(cutoff_cc),
        None => cutoff_env * key_scale,
    };
    cutoff.clamp(20.0, (sample_rate as f32 * 0.49).clamp(20.0, 20_000.0))
}

/// How far a full Filter Env Amount moves the cutoff, up or down.
const FILTER_ENV_OCTAVES: f32 = 5.0;

/// The cutoff a filter envelope at `env` (0 to 1) moves `cutoff_hz` to: Env Amount sweeps in
/// octaves, so the same amount sounds the same at any cutoff.
pub(super) fn filter_env_cutoff_hz(cutoff_hz: f32, env_amount: f32, env: f32) -> f32 {
    cutoff_hz * (env_amount * env * FILTER_ENV_OCTAVES).exp2()
}

/// How far CC74 moves the cutoff from one end to the other.
const CC74_SPAN_OCTAVES: f32 = 7.0;

/// CC74 moves the patch's cutoff, its envelope and key tracking included, evenly in pitch around
/// it: 64 leaves it alone, 0 and 127 take it about three and a half octaves down or up.
fn cc_cutoff_ratio(cutoff_cc: f32) -> f32 {
    ((cutoff_cc - 64.0 / 127.0) * CC74_SPAN_OCTAVES).exp2()
}

fn synth_voice_q(cfg: &SynthVoiceRenderConfig, resonance_cc: Option<f32>) -> f32 {
    let resonance = resonance_cc.map_or(cfg.filter_resonance, |resonance_cc| resonance_cc * 100.0);
    0.5 + (resonance.clamp(0.0, 100.0) / 100.0) * 11.5
}

#[cfg(test)]
mod render_voice_tests;

#[cfg(test)]
mod oscillator_alias_tests;

#[cfg(test)]
mod fm_tests;

#[cfg(test)]
mod fm_core_tests;

#[cfg(test)]
mod fm_spectral_tests;

#[cfg(test)]
mod pluck_tests;

#[cfg(test)]
mod pluck_decay_tests;

#[cfg(test)]
mod pluck_dispersion_tests;

#[cfg(test)]
mod pluck_pitch_tests;

#[cfg(test)]
mod pluck_modal_tests;

#[cfg(test)]
mod pluck_pitch_calibration_tests;

#[cfg(all(test, feature = "routing-tree-executor"))]
mod pluck_worker_tests;

#[cfg(test)]
mod live_param_parity_tests;

#[cfg(test)]
mod drum_tests;

#[cfg(all(test, feature = "routing-tree-executor"))]
mod drum_worker_tests;
