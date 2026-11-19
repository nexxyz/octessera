use super::render_voice::{SynthVoiceRenderConfig, VoiceSource};
use super::SynthEngine;
use crate::synth::scalar_param::ScalarMutation;
use crate::synth::types::{SampleBankConfig, SynthConfig, INSTRUMENT_SLOT_COUNT};

/// How long a live instrument tone change takes to land: about four 128-frame blocks, so a menu
/// step of cutoff or level no longer arrives as one jump, while a knob still feels immediate.
const INSTRUMENT_PARAM_GLIDE_MS: f32 = 10.0;

const PARAM_COUNT: usize = 20;

/// The continuous instrument parameters that glide when they change live. Envelope times,
/// waveforms, octaves, filter types and the parameters a voice only reads when it starts switch
/// at once.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum InstrumentGlideParam {
    Osc1Level,
    Osc1Detune,
    Osc1PulseWidth,
    Osc2Level,
    Osc2Detune,
    Osc2PulseWidth,
    AmpGain,
    AmpVelocitySensitivity,
    FilterCutoff,
    FilterResonance,
    FilterEnvAmount,
    FilterKeyTracking,
    FmIndex,
    FmVelocityToIndex,
    FmModShape,
    FmModMix,
    PluckDecay,
    PluckBrightness,
    SampleCutoff,
    SampleResonance,
}

impl InstrumentGlideParam {
    const ALL: [Self; PARAM_COUNT] = [
        Self::Osc1Level,
        Self::Osc1Detune,
        Self::Osc1PulseWidth,
        Self::Osc2Level,
        Self::Osc2Detune,
        Self::Osc2PulseWidth,
        Self::AmpGain,
        Self::AmpVelocitySensitivity,
        Self::FilterCutoff,
        Self::FilterResonance,
        Self::FilterEnvAmount,
        Self::FilterKeyTracking,
        Self::FmIndex,
        Self::FmVelocityToIndex,
        Self::FmModShape,
        Self::FmModMix,
        Self::PluckDecay,
        Self::PluckBrightness,
        Self::SampleCutoff,
        Self::SampleResonance,
    ];

    fn range(self) -> (f32, f32) {
        match self {
            Self::Osc1Detune | Self::Osc2Detune => (-50.0, 50.0),
            Self::Osc1PulseWidth | Self::Osc2PulseWidth => (5.0, 95.0),
            Self::FilterCutoff | Self::SampleCutoff => (20.0, 20_000.0),
            Self::FilterResonance | Self::SampleResonance => (0.0, 255.0),
            Self::FilterEnvAmount => (-100.0, 100.0),
            Self::FmIndex => (0.0, 4.0),
            Self::FmVelocityToIndex | Self::FmModShape | Self::FmModMix => (0.0, 1.0),
            Self::PluckDecay => (100.0, 5_000.0),
            _ => (0.0, 100.0),
        }
    }

    fn is_sample(self) -> bool {
        matches!(self, Self::SampleCutoff | Self::SampleResonance)
    }
}

/// Where a glide is heading; a settled parameter has no frames left and its engine field holds
/// the value.
#[derive(Clone, Copy, Debug)]
struct ParamGlide {
    target: f32,
    remaining: u32,
}

impl ParamGlide {
    const SETTLED: Self = Self {
        target: 0.0,
        remaining: 0,
    };

    /// Moves `current` `frames` along a straight line to the target; cutoffs move along a
    /// straight line in pitch instead.
    fn advance(&mut self, current: &mut f32, frames: u32, in_pitch: bool) {
        if frames >= self.remaining || !current.is_finite() || in_pitch && *current <= 0.0 {
            *current = self.target;
            self.remaining = 0;
            return;
        }
        let fraction = frames as f32 / self.remaining as f32;
        *current = if in_pitch {
            *current * (self.target / *current).powf(fraction)
        } else {
            *current + (self.target - *current) * fraction
        };
        self.remaining -= frames;
    }
}

/// The running glides of every instrument slot. Instruments advance them once per rendered
/// block, before the voices read the slot's values, so a settled engine pays one flag check.
#[derive(Clone, Copy, Debug)]
pub(super) struct InstrumentParamGlides {
    len: u32,
    active: bool,
    slots: [[ParamGlide; PARAM_COUNT]; INSTRUMENT_SLOT_COUNT],
}

impl InstrumentParamGlides {
    pub(super) fn new(sample_rate: u32) -> Self {
        Self {
            len: (sample_rate as f32 * INSTRUMENT_PARAM_GLIDE_MS / 1000.0)
                .round()
                .max(1.0) as u32,
            active: false,
            slots: [[ParamGlide::SETTLED; PARAM_COUNT]; INSTRUMENT_SLOT_COUNT],
        }
    }
}

fn glide_field<'a>(
    synth: &'a mut SynthConfig,
    source: &'a mut VoiceSource,
    bank: Option<&'a mut SampleBankConfig>,
    param: InstrumentGlideParam,
) -> Option<&'a mut f32> {
    use InstrumentGlideParam as P;
    Some(match (param, source) {
        (P::Osc1Level, _) => &mut synth.osc1.level_pct,
        (P::Osc1Detune, _) => &mut synth.osc1.detune_cents,
        (P::Osc1PulseWidth, _) => &mut synth.osc1.pulse_width_pct,
        (P::Osc2Level, _) => &mut synth.osc2.level_pct,
        (P::Osc2Detune, _) => &mut synth.osc2.detune_cents,
        (P::Osc2PulseWidth, _) => &mut synth.osc2.pulse_width_pct,
        (P::AmpGain, _) => &mut synth.amp.gain_pct,
        (P::AmpVelocitySensitivity, _) => &mut synth.amp.velocity_sensitivity_pct,
        (P::FilterCutoff, _) => &mut synth.filter.cutoff_hz,
        (P::FilterResonance, _) => &mut synth.filter.resonance,
        (P::FilterEnvAmount, _) => &mut synth.filter.env_amount_pct,
        (P::FilterKeyTracking, _) => &mut synth.filter.key_tracking_pct,
        (P::FmIndex, VoiceSource::Fm { index, .. }) => index,
        (
            P::FmVelocityToIndex,
            VoiceSource::Fm {
                velocity_to_index, ..
            },
        ) => velocity_to_index,
        (P::FmModShape, VoiceSource::Fm { mod_shape, .. }) => mod_shape,
        (P::FmModMix, VoiceSource::Fm { mod_mix, .. }) => mod_mix,
        (P::PluckDecay, VoiceSource::Pluck { settings }) => &mut settings.decay_ms,
        (P::PluckBrightness, VoiceSource::Pluck { settings }) => &mut settings.brightness_pct,
        (P::SampleCutoff, _) => &mut bank?.filter_cutoff_hz,
        (P::SampleResonance, _) => &mut bank?.filter_resonance,
        _ => return None,
    })
}

impl SynthEngine {
    /// Starts `param` of `slot` gliding towards `value` from where it is now, or from where a
    /// running glide is heading, the way the mixer ramps start.
    pub(super) fn glide_instrument_param(
        &mut self,
        slot: usize,
        param: InstrumentGlideParam,
        value: f32,
    ) -> ScalarMutation {
        if !value.is_finite() {
            return ScalarMutation::Rejected;
        }
        let (min, max) = param.range();
        let target = value.clamp(min, max);
        let glide = self.instrument_param_glides.slots[slot][param as usize];
        let Some(current) = glide_field(
            &mut self.instruments[slot],
            &mut self.synth_render_configs[slot].source,
            self.sample_banks.get_mut(slot),
            param,
        ) else {
            return ScalarMutation::Rejected;
        };
        let heading = if glide.remaining > 0 {
            glide.target
        } else {
            *current
        };
        if heading.to_bits() == target.to_bits() {
            return ScalarMutation::Unchanged;
        }
        let glides = &mut self.instrument_param_glides;
        glides.slots[slot][param as usize] = ParamGlide {
            target,
            remaining: glides.len,
        };
        glides.active = true;
        ScalarMutation::Changed
    }

    /// Moves every running glide one block along; the voices then read the moved values.
    #[inline]
    pub(super) fn advance_instrument_param_glides(&mut self, frames: usize) {
        if self.instrument_param_glides.active {
            self.advance_running_instrument_param_glides(frames);
        }
    }

    /// Kept out of line so the settled block render compiles exactly as it did without glides.
    #[inline(never)]
    fn advance_running_instrument_param_glides(&mut self, frames: usize) {
        let mut active = false;
        for slot in 0..INSTRUMENT_SLOT_COUNT {
            active |= self.advance_slot_param_glides(slot, frames as u32);
        }
        self.instrument_param_glides.active = active;
    }

    /// Lands `slot`'s running glides on their targets before a slot config writes its values.
    pub(super) fn finish_instrument_param_glides(&mut self, slot: usize) {
        if self.instrument_param_glides.active {
            self.advance_slot_param_glides(slot, u32::MAX);
        }
    }

    fn advance_slot_param_glides(&mut self, slot: usize, frames: u32) -> bool {
        let (mut synth_moved, mut sample_moved, mut active) = (false, false, false);
        for param in InstrumentGlideParam::ALL {
            let mut glide = self.instrument_param_glides.slots[slot][param as usize];
            if glide.remaining == 0 {
                continue;
            }
            match glide_field(
                &mut self.instruments[slot],
                &mut self.synth_render_configs[slot].source,
                self.sample_banks.get_mut(slot),
                param,
            ) {
                Some(current) => glide.advance(
                    current,
                    frames,
                    matches!(
                        param,
                        InstrumentGlideParam::FilterCutoff | InstrumentGlideParam::SampleCutoff
                    ),
                ),
                None => glide.remaining = 0,
            }
            self.instrument_param_glides.slots[slot][param as usize] = glide;
            active |= glide.remaining > 0;
            if param.is_sample() {
                sample_moved = true;
            } else {
                synth_moved = true;
            }
        }
        if synth_moved {
            self.refresh_synth_render_config(slot);
        }
        if let Some(bank) = self.sample_banks.get(slot).filter(|_| sample_moved) {
            let _ = self.sample_voice_pool.update_filter_for_slot(
                slot,
                bank.filter_cutoff_hz,
                bank.filter_resonance,
            );
        }
        active
    }

    /// Rebuilds the render config the voices read from the slot's values, keeping its source.
    pub(super) fn refresh_synth_render_config(&mut self, slot: usize) {
        let previous = self.synth_render_configs[slot];
        let mut render = SynthVoiceRenderConfig::from_config(self.instruments[slot]);
        render.source = previous.source;
        render.source_generation = previous.source_generation;
        self.synth_render_configs[slot] = render;
        self.synth_render_revisions[slot] = self.synth_render_revisions[slot].wrapping_add(1);
    }
}

#[cfg(test)]
mod tests;
