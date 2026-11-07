use super::{ms_param_len, pct_param};
use crate::synth::runtime_state::BiquadState;
use crate::synth::types::FilterType;
use serde_json::Value;
use std::collections::BTreeMap;
use std::f32::consts::FRAC_1_SQRT_2;

/// The open end of the sweep: a resonance-free highpass here leaves the audio untouched.
const OPEN_HZ: f32 = 20.0;
/// The wet path blends in over the first tenth of the sweep, so the filter's phase shift near
/// its open end never shows up as a step when it engages or hands back to the dry signal.
const ENGAGE_SPAN: f32 = 0.1;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::synth::engine) struct HpRiserParams {
    pub(in crate::synth::engine) target_octaves: f32,
    pub(in crate::synth::engine) q: f32,
    pub(in crate::synth::engine) sweep_in_step: f32,
    pub(in crate::synth::engine) sweep_out_step: f32,
}

impl HpRiserParams {
    /// Cutoff 0 targets 80 Hz and 100 about 8 kHz, evenly in octaves.
    pub(super) fn from_map(params: &BTreeMap<String, Value>, sample_rate: u32) -> Self {
        let target_hz = 80.0 * 100.0_f32.powf(pct_param(params, "cutoffPct", 60.0));
        let sweep_in = ms_param_len(params, "sweepInMs", 2_000.0, 10.0, 8_000.0, sample_rate);
        let sweep_out = ms_param_len(params, "sweepOutMs", 150.0, 10.0, 3_000.0, sample_rate);
        Self {
            target_octaves: (target_hz / OPEN_HZ).log2(),
            q: FRAC_1_SQRT_2 + pct_param(params, "resonancePct", 30.0) * 7.3,
            sweep_in_step: 1.0 / sweep_in as f32,
            sweep_out_step: 1.0 / sweep_out as f32,
        }
    }
}

/// filter_sweep's highpass counterpart: from fully open it rises to the target cutoff over
/// Sweep In, evenly in octaves, with the resonance following; release sweeps back open.
#[derive(Clone, Debug)]
pub(in crate::synth::engine) struct HpRiserVoice {
    pub(super) params: HpRiserParams,
    pub(super) sweep_pos: f32,
    filt_l: BiquadState,
    filt_r: BiquadState,
    releasing: bool,
    sample_rate: u32,
}

impl HpRiserVoice {
    pub(super) fn new(params: HpRiserParams, sample_rate: u32) -> Self {
        Self {
            params,
            sweep_pos: 0.0,
            filt_l: BiquadState::new(),
            filt_r: BiquadState::new(),
            releasing: false,
            sample_rate,
        }
    }

    /// A new press of the same type takes over the releasing voice with its own settings.
    pub(super) fn revive(&mut self, params: HpRiserParams) {
        self.params = params;
        self.releasing = false;
    }

    pub(super) fn release(&mut self) {
        self.releasing = true;
    }

    pub(super) fn finished(&self) -> bool {
        self.releasing && self.sweep_pos <= 0.0
    }

    pub(super) fn process(&mut self, left: f32, right: f32) -> (f32, f32) {
        self.sweep_pos = if self.releasing {
            (self.sweep_pos - self.params.sweep_out_step).max(0.0)
        } else {
            (self.sweep_pos + self.params.sweep_in_step).min(1.0)
        };
        let cutoff = OPEN_HZ * (self.params.target_octaves * self.sweep_pos).exp2();
        let q = FRAC_1_SQRT_2 + (self.params.q - FRAC_1_SQRT_2) * self.sweep_pos;
        let rate = self.sample_rate;
        let wet_l = self
            .filt_l
            .process(left, FilterType::Highpass, cutoff, q, rate);
        let wet_r = self
            .filt_r
            .process(right, FilterType::Highpass, cutoff, q, rate);
        let blend = (self.sweep_pos / ENGAGE_SPAN).min(1.0);
        (
            left + (wet_l - left) * blend,
            right + (wet_r - right) * blend,
        )
    }
}
