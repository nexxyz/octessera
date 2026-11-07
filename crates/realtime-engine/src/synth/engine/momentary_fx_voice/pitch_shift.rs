use super::super::super::scalar_param::ScalarMutation;
use super::super::live_pitch_shift::{
    pitch_activation_ramp_len, LivePitchShift, PITCH_DEFAULT_SLIDE_IN_MS,
    PITCH_DEFAULT_SLIDE_OUT_MS, PITCH_FILL_FRAMES,
};
use super::{param_f32, pct_param};
use crate::synth::runtime_state::ms_to_samples;
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::synth::engine) struct PitchShiftParams {
    pub(in crate::synth::engine) target_octaves: f32,
    pub(in crate::synth::engine) mix: f32,
    pub(in crate::synth::engine) slide_in_len: u32,
    pub(in crate::synth::engine) slide_out_len: u32,
}

impl PitchShiftParams {
    pub(in crate::synth::engine) fn from_map(
        params: &BTreeMap<String, Value>,
        sample_rate: u32,
    ) -> Self {
        let semitones = param_f32(params, "semitones", 7.0).clamp(-24.0, 24.0);
        let cents = param_f32(params, "cents", 0.0).clamp(-100.0, 100.0);
        let slide_len = |key: &str, fallback: f32| {
            ms_to_samples(param_f32(params, key, fallback), sample_rate).max(1)
        };
        Self {
            target_octaves: (semitones + cents / 100.0) / 12.0,
            mix: pct_param(params, "mixPct", 100.0),
            slide_in_len: slide_len("slideInMs", PITCH_DEFAULT_SLIDE_IN_MS),
            slide_out_len: slide_len("slideOutMs", PITCH_DEFAULT_SLIDE_OUT_MS),
        }
    }
}

pub(in crate::synth::engine) enum PitchStopAction {
    Retire,
    Release,
    Ignore,
}

/// Shifts the live target, gliding in octaves to the target interval over Slide In and back
/// to unison over Slide Out. Its wet path waits until the shifter's buffer has filled, then
/// eases in; release fades it out at the end of the slide back.
#[derive(Clone, Debug)]
pub(in crate::synth::engine) struct PitchShiftVoice {
    pub(in crate::synth::engine) params: PitchShiftParams,
    pub(in crate::synth::engine) shifter: LivePitchShift,
    pub(in crate::synth::engine) fill_pos: u32,
    pub(in crate::synth::engine) ramp_pos: u32,
    pub(in crate::synth::engine) ramp_len: u32,
    pub(in crate::synth::engine) amount_octaves: f32,
    pub(in crate::synth::engine) slide_start_octaves: f32,
    pub(in crate::synth::engine) slide_target_octaves: f32,
    pub(in crate::synth::engine) slide_pos: u32,
    pub(in crate::synth::engine) slide_len: u32,
    pub(in crate::synth::engine) release_pos: u32,
    pub(in crate::synth::engine) release_len: u32,
    releasing: bool,
}

impl PitchShiftVoice {
    pub(super) fn new(params: PitchShiftParams, sample_rate: u32) -> Self {
        Self {
            params,
            shifter: LivePitchShift::new(sample_rate),
            fill_pos: 0,
            ramp_pos: 0,
            ramp_len: pitch_activation_ramp_len(sample_rate),
            amount_octaves: 0.0,
            slide_start_octaves: 0.0,
            slide_target_octaves: params.target_octaves,
            slide_pos: 0,
            slide_len: params.slide_in_len,
            release_pos: 0,
            release_len: 0,
            releasing: false,
        }
    }

    /// A shift that never became audible retires at once; otherwise it slides back to unison.
    pub(in crate::synth::engine) fn stop(&mut self) -> PitchStopAction {
        if self.releasing {
            return PitchStopAction::Ignore;
        }
        let activation = if self.fill_pos < PITCH_FILL_FRAMES {
            0.0
        } else {
            (self.ramp_pos as f32 / self.ramp_len.max(1) as f32).clamp(0.0, 1.0)
        };
        if self.params.mix * activation == 0.0 {
            return PitchStopAction::Retire;
        }
        let slide_out_len = self.params.slide_out_len;
        self.releasing = true;
        self.release_pos = 0;
        self.release_len = self.ramp_len.max(slide_out_len);
        self.slide_start_octaves = self.amount_octaves;
        self.slide_target_octaves = 0.0;
        self.slide_pos = 0;
        self.slide_len = slide_out_len;
        PitchStopAction::Release
    }

    pub(super) fn finished(&self) -> bool {
        self.releasing && self.release_pos >= self.release_len
    }

    /// Live settings for a held shift; a changed interval or Slide In restarts the slide.
    pub(super) fn update(&mut self, next: PitchShiftParams) -> ScalarMutation {
        if self.releasing {
            return ScalarMutation::Rejected;
        }
        let current = self.params;
        if current.target_octaves.to_bits() == next.target_octaves.to_bits()
            && current.mix.to_bits() == next.mix.to_bits()
            && current.slide_in_len == next.slide_in_len
            && current.slide_out_len == next.slide_out_len
        {
            return ScalarMutation::Unchanged;
        }
        let target_changed = current.target_octaves.to_bits() != next.target_octaves.to_bits();
        let slide_in_changed = current.slide_in_len != next.slide_in_len;
        self.params = next;
        if target_changed || (slide_in_changed && self.slide_pos < self.slide_len) {
            self.slide_start_octaves = self.amount_octaves;
            self.slide_target_octaves = next.target_octaves;
            self.slide_pos = 0;
            self.slide_len = next.slide_in_len;
        } else if slide_in_changed {
            self.slide_start_octaves = next.target_octaves;
            self.slide_target_octaves = next.target_octaves;
            self.slide_pos = next.slide_in_len;
            self.slide_len = next.slide_in_len;
        }
        ScalarMutation::Changed
    }

    pub(super) fn process(&mut self, left: f32, right: f32) -> (f32, f32) {
        let mix = self.params.mix;
        if self.fill_pos < PITCH_FILL_FRAMES {
            let _ = self.shifter.process_frame(left, right, 1.0);
            self.fill_pos += 1;
            return (left, right);
        }
        let pitch_octaves = if self.slide_pos < self.slide_len {
            self.slide_pos = (self.slide_pos + 1).min(self.slide_len);
            let progress = self.slide_pos as f32 / self.slide_len.max(1) as f32;
            self.slide_start_octaves
                + (self.slide_target_octaves - self.slide_start_octaves) * progress
        } else {
            self.slide_target_octaves
        };
        self.amount_octaves = pitch_octaves;
        let ratio = 2.0_f32.powf(pitch_octaves);
        let (wet_l, wet_r) = self.shifter.process_frame(left, right, ratio);
        if self.releasing {
            let activation = (self.ramp_pos as f32 / self.ramp_len.max(1) as f32).clamp(0.0, 1.0);
            let fade_start = self.release_len.saturating_sub(self.ramp_len);
            let release_gain = if self.release_pos < fade_start {
                1.0
            } else {
                self.release_len.saturating_sub(self.release_pos) as f32
                    / self.ramp_len.max(1) as f32
            };
            let wet_gain = mix * activation * release_gain;
            self.release_pos += 1;
            return (
                left * (1.0 - wet_gain) + wet_l * wet_gain,
                right * (1.0 - wet_gain) + wet_r * wet_gain,
            );
        }
        let ramp = if self.ramp_pos < self.ramp_len {
            let ramp = self.ramp_pos as f32 / self.ramp_len as f32;
            self.ramp_pos += 1;
            ramp
        } else {
            1.0
        };
        let wet_mix = mix * ramp;
        (
            left * (1.0 - wet_mix) + wet_l * wet_mix,
            right * (1.0 - wet_mix) + wet_r * wet_mix,
        )
    }
}
