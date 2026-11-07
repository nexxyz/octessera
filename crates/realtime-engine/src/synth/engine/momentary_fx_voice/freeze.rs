use super::{param_f32, pct_param};
use crate::synth::fx::{comb_bank_frame, comb_buffers};
use crate::synth::runtime_state::ms_to_samples;
use serde_json::Value;
use std::collections::BTreeMap;

pub(in crate::synth::engine) const FREEZE_INJECT_MS: u32 = 120;
const FEEDBACK: f32 = 0.997;
const DAMP: f32 = 0.35;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::synth::engine) struct FreezeParams {
    pub(in crate::synth::engine) mix: f32,
    pub(in crate::synth::engine) release_len: u32,
}

impl FreezeParams {
    pub(super) fn from_map(params: &BTreeMap<String, Value>, sample_rate: u32) -> Self {
        Self {
            mix: pct_param(params, "mixPct", 100.0),
            release_len: ms_to_samples(param_f32(params, "releaseMs", 500.0), sample_rate).max(1),
        }
    }
}

/// Injects the first moments after the press into an almost endless comb tail that holds with
/// no new input. Its wet mix eases in once every comb has filled; on release the tail fades
/// over Release.
#[derive(Clone, Debug)]
pub(in crate::synth::engine) struct FreezeVoice {
    pub(super) params: FreezeParams,
    pub(in crate::synth::engine) bufs: [Vec<f32>; 4],
    pub(in crate::synth::engine) idxs: [usize; 4],
    pub(in crate::synth::engine) lp: [f32; 4],
    pub(in crate::synth::engine) inject_pos: u32,
    inject_len: u32,
    pub(in crate::synth::engine) ready_len: u32,
    pub(in crate::synth::engine) activation_pos: u32,
    pub(in crate::synth::engine) activation_len: u32,
    pub(in crate::synth::engine) release_pos: u32,
    pub(in crate::synth::engine) release_len: u32,
    releasing: bool,
}

impl FreezeVoice {
    pub(super) fn new(params: FreezeParams, sample_rate: u32) -> Self {
        let bufs = comb_buffers(1, sample_rate);
        let ready_len = bufs.iter().map(Vec::len).max().unwrap_or(1) as u32;
        Self {
            params,
            bufs,
            idxs: [0; 4],
            lp: [0.0; 4],
            inject_pos: 0,
            inject_len: (sample_rate * FREEZE_INJECT_MS / 1000).max(1),
            ready_len,
            activation_pos: 0,
            activation_len: (sample_rate * 5 / 1000).max(1),
            release_pos: 0,
            release_len: 0,
            releasing: false,
        }
    }

    pub(super) fn release(&mut self) {
        self.releasing = true;
        self.release_pos = 0;
        self.release_len = self.params.release_len;
    }

    pub(super) fn finished(&self) -> bool {
        self.releasing && self.release_pos >= self.release_len
    }

    pub(super) fn process(&mut self, left: f32, right: f32) -> (f32, f32) {
        let mix = self.params.mix;
        if self.releasing {
            let total = self.release_len.max(1) as f32;
            let fade = 1.0 - (self.release_pos as f32 / total);
            self.release_pos += 1;
            let (wet_l, wet_r) = comb_bank_frame(
                &mut self.bufs,
                &mut self.idxs,
                &mut self.lp,
                (0.0, 0.0),
                FEEDBACK,
                DAMP,
            );
            let activation_gain = self.activation_gain();
            mix_output(left, right, wet_l, wet_r, mix * activation_gain * fade)
        } else {
            let activation_gain = self.activation_gain();
            let injecting = self.inject_pos < self.inject_len;
            let inject_gain = if injecting { 1.0 } else { 0.0 };
            if injecting {
                self.inject_pos += 1;
            }
            let (wet_l, wet_r) = comb_bank_frame(
                &mut self.bufs,
                &mut self.idxs,
                &mut self.lp,
                (left * inject_gain, right * inject_gain),
                FEEDBACK,
                DAMP,
            );
            mix_output(left, right, wet_l, wet_r, mix * activation_gain)
        }
    }

    fn activation_gain(&mut self) -> f32 {
        if self.inject_pos < self.ready_len {
            return 0.0;
        }
        if self.activation_pos >= self.activation_len {
            return 1.0;
        }
        let gain = self.activation_pos as f32 / self.activation_len as f32;
        if !self.releasing {
            self.activation_pos += 1;
        }
        gain
    }
}

fn mix_output(left: f32, right: f32, wet_l: f32, wet_r: f32, effective_mix: f32) -> (f32, f32) {
    if effective_mix == 0.0 {
        return (left, right);
    }
    (
        left * (1.0 - effective_mix) + wet_l * effective_mix,
        right * (1.0 - effective_mix) + wet_r * effective_mix,
    )
}
