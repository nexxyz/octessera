use super::{ms_param_len, pct_param, GainRamp, ENVELOPE_MS};
use crate::synth::fx::{comb_bank_frame, comb_buffers};
use serde_json::Value;
use std::collections::BTreeMap;

/// The shared combs at twice their length, for a larger room than freeze's.
const COMB_SCALE: usize = 2;
const DIFFUSER_LENS: [usize; 4] = [556, 225, 579, 241];
const DIFFUSION: f32 = 0.5;
const DAMP: f32 = 0.3;
const SWELL_MS: u32 = 250;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::synth::engine) struct WashParams {
    pub(in crate::synth::engine) feedback: f32,
    pub(in crate::synth::engine) mix: f32,
    pub(in crate::synth::engine) release_len: u32,
    /// The send level that keeps the comb tail's loudness independent of its feedback.
    inject: f32,
}

impl WashParams {
    pub(super) fn new(feedback: f32, mix: f32, release_len: u32) -> Self {
        Self {
            feedback,
            mix,
            release_len,
            inject: (1.0 - feedback * feedback).sqrt(),
        }
    }

    pub(super) fn from_map(params: &BTreeMap<String, Value>, sample_rate: u32) -> Self {
        Self::new(
            0.75 + 0.22 * pct_param(params, "sizePct", 80.0),
            pct_param(params, "mixPct", 60.0),
            ms_param_len(params, "releaseMs", 2_500.0, 100.0, 8_000.0, sample_rate),
        )
    }
}

/// Diffuses the target into a comb reverb and swells its wet mix in while held. On release the
/// send closes and the tail fades out over Release, then the voice ends.
#[derive(Clone, Debug)]
pub(in crate::synth::engine) struct WashVoice {
    pub(super) params: WashParams,
    combs: [Vec<f32>; 4],
    comb_idxs: [usize; 4],
    comb_lp: [f32; 4],
    diffusers: [Vec<f32>; 4],
    diffuser_idxs: [usize; 4],
    send: GainRamp,
    swell: GainRamp,
    release_step: f32,
    releasing: bool,
}

impl WashVoice {
    pub(super) fn new(params: WashParams, sample_rate: u32) -> Self {
        let scaled = |n: usize| vec![0.0; (n * sample_rate as usize / 44_100).max(1)];
        Self {
            params,
            combs: comb_buffers(COMB_SCALE, sample_rate),
            comb_idxs: [0; 4],
            comb_lp: [0.0; 4],
            diffusers: DIFFUSER_LENS.map(scaled),
            diffuser_idxs: [0; 4],
            send: GainRamp::new(0.0, ENVELOPE_MS, sample_rate),
            swell: GainRamp::new(0.0, SWELL_MS, sample_rate),
            release_step: 0.0,
            releasing: false,
        }
    }

    pub(super) fn revive(&mut self, params: WashParams) {
        self.params = params;
        self.releasing = false;
    }

    pub(super) fn release(&mut self) {
        self.releasing = true;
        self.release_step = self.swell.value() / self.params.release_len.max(1) as f32;
    }

    pub(super) fn finished(&self) -> bool {
        self.releasing && self.swell.value() <= 0.0
    }

    pub(super) fn process(&mut self, left: f32, right: f32) -> (f32, f32) {
        let send = self.send.toward(if self.releasing { 0.0 } else { 1.0 });
        let swell = if self.releasing {
            self.swell.fall_by(self.release_step)
        } else {
            self.swell.toward(1.0)
        };
        let feedback = self.params.feedback;
        let inject = send * self.params.inject;
        let in_l = self.diffuse(0, left * inject);
        let in_l = self.diffuse(1, in_l);
        let in_r = self.diffuse(2, right * inject);
        let in_r = self.diffuse(3, in_r);
        let (wet_l, wet_r) = comb_bank_frame(
            &mut self.combs,
            &mut self.comb_idxs,
            &mut self.comb_lp,
            (in_l, in_r),
            feedback,
            DAMP,
        );
        let mix = self.params.mix * swell;
        (left + (wet_l - left) * mix, right + (wet_r - right) * mix)
    }

    fn diffuse(&mut self, index: usize, input: f32) -> f32 {
        let buf = &mut self.diffusers[index];
        let at = self.diffuser_idxs[index];
        let delayed = buf[at];
        let output = delayed - DIFFUSION * input;
        buf[at] = input + DIFFUSION * output;
        self.diffuser_idxs[index] = (at + 1) % buf.len();
        output
    }
}
