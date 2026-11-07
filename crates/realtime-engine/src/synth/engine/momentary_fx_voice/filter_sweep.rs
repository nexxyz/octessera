use super::pct_param;
use crate::synth::engine::support::param_f32;
use crate::synth::runtime_state::{ms_to_samples, BiquadState};
use crate::synth::types::FilterType;
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::synth::engine) struct FilterSweepParams {
    pub(in crate::synth::engine) target_cutoff: f32,
    pub(in crate::synth::engine) q: f32,
    pub(in crate::synth::engine) sweep_in_step: f32,
    pub(in crate::synth::engine) sweep_out_step: f32,
}

impl FilterSweepParams {
    /// Cutoff 0 targets about 120 Hz and 100 about 8 kHz.
    pub(super) fn from_map(params: &BTreeMap<String, Value>, sample_rate: u32) -> Self {
        let in_len =
            ms_to_samples(param_f32(params, "sweepInMs", 200.0), sample_rate).max(1) as f32;
        let out_len =
            ms_to_samples(param_f32(params, "sweepOutMs", 500.0), sample_rate).max(1) as f32;
        Self {
            target_cutoff: 120.0 + pct_param(params, "cutoffPct", 35.0) * 8_000.0,
            q: 0.5 + pct_param(params, "resonancePct", 70.0) * 11.5,
            sweep_in_step: 1.0 / in_len,
            sweep_out_step: 1.0 / out_len,
        }
    }
}

/// A lowpass that starts fully open and sweeps to the target cutoff while held; release sweeps
/// it back open, then the voice ends.
#[derive(Clone, Debug)]
pub(in crate::synth::engine) struct FilterSweepVoice {
    pub(super) params: FilterSweepParams,
    pub(in crate::synth::engine) sweep_pos: f32,
    pub(in crate::synth::engine) filt_l: BiquadState,
    pub(in crate::synth::engine) filt_r: BiquadState,
    releasing: bool,
    sample_rate: u32,
}

impl FilterSweepVoice {
    pub(super) fn new(params: FilterSweepParams, sample_rate: u32) -> Self {
        Self {
            params,
            sweep_pos: 0.0,
            filt_l: BiquadState::new(),
            filt_r: BiquadState::new(),
            releasing: false,
            sample_rate,
        }
    }

    pub(super) fn release(&mut self) {
        self.releasing = true;
    }

    pub(super) fn finished(&self) -> bool {
        self.releasing && self.sweep_pos <= 0.0
    }

    pub(super) fn process(&mut self, left: f32, right: f32) -> (f32, f32) {
        if self.releasing {
            self.sweep_pos -= self.params.sweep_out_step;
            if self.sweep_pos < 0.0 {
                self.sweep_pos = 0.0;
            }
        } else {
            self.sweep_pos += self.params.sweep_in_step;
            if self.sweep_pos > 1.0 {
                self.sweep_pos = 1.0;
            }
        }

        let cutoff = 20_000.0 + (self.params.target_cutoff - 20_000.0) * self.sweep_pos;
        let q = self.params.q;
        let rate = self.sample_rate;
        (
            self.filt_l
                .process(left, FilterType::Lowpass, cutoff, q, rate),
            self.filt_r
                .process(right, FilterType::Lowpass, cutoff, q, rate),
        )
    }
}
