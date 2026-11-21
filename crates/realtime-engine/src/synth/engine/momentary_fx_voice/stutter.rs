use super::{param_f32, pct_param, GainRamp, ENVELOPE_MS};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::synth::engine) struct StutterParams {
    pub(in crate::synth::engine) depth: f32,
    pub(in crate::synth::engine) segment_len: usize,
}

impl StutterParams {
    pub(super) fn from_map(params: &BTreeMap<String, Value>, sample_rate: u32) -> Self {
        let rate = param_f32(params, "rateHz", 8.0).clamp(1.0, 32.0);
        Self {
            depth: pct_param(params, "depthPct", 100.0),
            segment_len: ((sample_rate as f32 / rate) as usize).clamp(48, sample_rate as usize),
        }
    }
}

/// Captures one segment of the target plus ~2 ms on press, then loops the segment with a ~2 ms
/// ease-in, crossfading that extra tail into the start of each repeat. On release the repeats fade
/// back to the dry sound over a few ms.
#[derive(Clone, Debug)]
pub(in crate::synth::engine) struct StutterVoice {
    pub(in crate::synth::engine) params: StutterParams,
    pub(in crate::synth::engine) left: Vec<f32>,
    pub(in crate::synth::engine) right: Vec<f32>,
    pub(in crate::synth::engine) write: usize,
    pub(in crate::synth::engine) ready: bool,
    ramp_len: usize,
    pub(in crate::synth::engine) ramp_pos: usize,
    releasing: bool,
    release_fade: GainRamp,
}

impl StutterVoice {
    pub(super) fn new(params: StutterParams, sample_rate: u32) -> Self {
        let ramp_len = ((sample_rate as f32 * 0.002) as usize).max(1);
        Self {
            params,
            left: vec![0.0; sample_rate as usize + ramp_len],
            right: vec![0.0; sample_rate as usize + ramp_len],
            write: 0,
            ready: false,
            ramp_len,
            ramp_pos: 0,
            releasing: false,
            release_fade: GainRamp::new(1.0, ENVELOPE_MS, sample_rate),
        }
    }

    /// New settings start a fresh capture.
    pub(super) fn retime(&mut self, params: StutterParams) {
        self.params = params;
        self.write = 0;
        self.ready = false;
        self.ramp_pos = 0;
    }

    pub(super) fn release(&mut self) {
        self.releasing = true;
    }

    pub(super) fn finished(&self) -> bool {
        self.releasing && (!self.ready || self.release_fade.value() <= 0.0)
    }

    pub(super) fn process(&mut self, left: f32, right: f32) -> (f32, f32) {
        let depth = if self.releasing {
            self.params.depth * self.release_fade.toward(0.0)
        } else {
            self.params.depth
        };
        let segment_len = self
            .params
            .segment_len
            .min(self.left.len() - self.ramp_len)
            .max(1);
        let ramp_len = self.ramp_len.min(segment_len / 4);
        let mut l = left;
        let mut r = right;

        if !self.ready {
            self.left[self.write] = l;
            self.right[self.write] = r;
            self.write += 1;
            if self.write >= segment_len + ramp_len {
                self.ready = true;
                self.write = 0;
                self.ramp_pos = 0;
            }
        } else {
            let read = self.write;
            let mut wet_l = self.left[read];
            let mut wet_r = self.right[read];
            let eff_wet = if self.ramp_pos < ramp_len {
                let ramp = self.ramp_pos as f32 / ramp_len as f32;
                self.ramp_pos += 1;
                depth * ramp
            } else {
                depth
            };

            if read < ramp_len {
                let fade_in = read as f32 / ramp_len as f32;
                let end_read = segment_len + read;
                wet_l = wet_l * fade_in + self.left[end_read] * (1.0 - fade_in);
                wet_r = wet_r * fade_in + self.right[end_read] * (1.0 - fade_in);
            }

            l = l * (1.0 - eff_wet) + wet_l * eff_wet;
            r = r * (1.0 - eff_wet) + wet_r * eff_wet;
            self.write += 1;
            if self.write >= segment_len {
                self.write = 0;
            }
        }

        (l, r)
    }
}
