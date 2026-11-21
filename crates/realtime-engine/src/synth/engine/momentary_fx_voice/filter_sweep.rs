use super::super::super::scalar_param::ScalarMutation;
use super::{ms_param_len, pct_param};
use crate::synth::runtime_state::BiquadState;
use crate::synth::types::FilterType;
use serde_json::Value;
use std::collections::BTreeMap;

const OPEN_CUTOFF_HZ: f32 = 20_000.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::synth::engine) struct FilterSweepParams {
    pub(in crate::synth::engine) target_octaves: f32,
    pub(in crate::synth::engine) q: f32,
    pub(in crate::synth::engine) sweep_in_step: f32,
    pub(in crate::synth::engine) sweep_out_step: f32,
}

impl FilterSweepParams {
    /// Cutoff 0 targets about 120 Hz and 100 about 8 kHz, in octaves below the open cutoff.
    pub(super) fn from_map(params: &BTreeMap<String, Value>, sample_rate: u32) -> Self {
        let in_len = ms_param_len(params, "sweepInMs", 200.0, 10.0, 3_000.0, sample_rate) as f32;
        let out_len = ms_param_len(params, "sweepOutMs", 500.0, 10.0, 3_000.0, sample_rate) as f32;
        Self {
            target_octaves: ((120.0 + pct_param(params, "cutoffPct", 35.0) * 8_000.0)
                / OPEN_CUTOFF_HZ)
                .log2(),
            q: 0.5 + pct_param(params, "resonancePct", 70.0) * 11.5,
            sweep_in_step: 1.0 / in_len,
            sweep_out_step: 1.0 / out_len,
        }
    }
}

/// A lowpass that starts fully open and sweeps to the target cutoff while held, evenly in
/// octaves; release sweeps it back open, then the voice ends. A new target glides there from the
/// current one over Sweep In, also in octaves.
#[derive(Clone, Debug)]
pub(in crate::synth::engine) struct FilterSweepVoice {
    pub(super) params: FilterSweepParams,
    pub(in crate::synth::engine) sweep_pos: f32,
    pub(in crate::synth::engine) filt_l: BiquadState,
    pub(in crate::synth::engine) filt_r: BiquadState,
    glide_from: (f32, f32),
    glide_pos: f32,
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
            glide_from: (params.target_octaves, params.q),
            glide_pos: 1.0,
            releasing: false,
            sample_rate,
        }
    }

    pub(super) fn update(&mut self, next: FilterSweepParams) -> ScalarMutation {
        if self.params == next {
            return ScalarMutation::Unchanged;
        }
        if next.target_octaves != self.params.target_octaves || next.q != self.params.q {
            self.glide_from = self.glided_target();
            self.glide_pos = 0.0;
        }
        self.params = next;
        ScalarMutation::Changed
    }

    /// The target octaves and resonance on the way from the previous target to the current one.
    fn glided_target(&self) -> (f32, f32) {
        let (from_octaves, from_q) = self.glide_from;
        (
            from_octaves + (self.params.target_octaves - from_octaves) * self.glide_pos,
            from_q + (self.params.q - from_q) * self.glide_pos,
        )
    }

    pub(super) fn cutoff(&self) -> f32 {
        OPEN_CUTOFF_HZ * (self.glided_target().0 * self.sweep_pos).exp2()
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

        if self.glide_pos < 1.0 {
            self.glide_pos = (self.glide_pos + self.params.sweep_in_step).min(1.0);
        }
        let (_, q) = self.glided_target();
        self.filt_l
            .prepare(FilterType::Lowpass, self.cutoff(), q, self.sample_rate);
        self.filt_r.prepare_like(&self.filt_l);
        (
            self.filt_l.process_prepared(left),
            self.filt_r.process_prepared(right),
        )
    }
}
