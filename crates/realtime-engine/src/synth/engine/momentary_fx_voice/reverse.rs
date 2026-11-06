use super::capture_ring::CaptureRing;
use super::{ms_param_len, pct_param, GainRamp, HANDOFF_MS};
use serde_json::Value;
use std::collections::BTreeMap;

const MAX_LENGTH_MS: f32 = 1_000.0;
const LOOP_FADE_MS: u32 = 5;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::synth::engine) struct ReverseParams {
    pub(in crate::synth::engine) window_len: u32,
    pub(in crate::synth::engine) mix: f32,
}

impl ReverseParams {
    pub(super) fn from_map(params: &BTreeMap<String, Value>, sample_rate: u32) -> Self {
        Self {
            window_len: ms_param_len(params, "lengthMs", 250.0, 50.0, MAX_LENGTH_MS, sample_rate),
            mix: pct_param(params, "mixPct", 100.0),
        }
    }
}

/// One backwards read through the ring: it starts at the newest frame of `start` and walks back.
#[derive(Clone, Copy)]
struct ReverseHead {
    start: usize,
    pos: usize,
}

/// Captures one Length after the press, then plays each latest Length backwards. Consecutive
/// windows overlap by a short crossfade; the wet mix ramps in and out around them.
#[derive(Clone)]
pub(in crate::synth::engine) struct ReverseVoice {
    pub(super) params: ReverseParams,
    ring: CaptureRing,
    captured: u32,
    head: ReverseHead,
    fading: ReverseHead,
    fade_pos: u32,
    fade_len: u32,
    pub(super) wet: GainRamp,
    releasing: bool,
}

impl ReverseVoice {
    pub(super) fn new(params: ReverseParams, sample_rate: u32) -> Self {
        let max_window = (MAX_LENGTH_MS as u32 * sample_rate / 1_000) as usize;
        let fade_len = (sample_rate * LOOP_FADE_MS / 1_000).max(1);
        let idle = ReverseHead { start: 0, pos: 0 };
        Self {
            params,
            ring: CaptureRing::new(2 * max_window + fade_len as usize + 8),
            captured: 0,
            head: idle,
            fading: idle,
            fade_pos: fade_len,
            fade_len,
            wet: GainRamp::new(0.0, HANDOFF_MS, sample_rate),
            releasing: false,
        }
    }

    /// A new press of the same type takes over the releasing voice with its own settings.
    pub(super) fn revive(&mut self, params: ReverseParams) {
        self.params = params;
        self.releasing = false;
    }

    pub(super) fn release(&mut self) {
        self.releasing = true;
    }

    pub(super) fn finished(&self) -> bool {
        self.releasing && self.wet.value() <= 0.0
    }

    pub(super) fn process(&mut self, left: f32, right: f32) -> (f32, f32) {
        self.ring.push(left, right);
        let window = self.params.window_len.max(3 * self.fade_len);
        if self.captured < window {
            self.captured += 1;
            if self.captured < window || self.releasing {
                return (left, right);
            }
            self.head = self.newest_head();
        }
        let (mut wet_l, mut wet_r) = self.read(self.head);
        if self.fade_pos < self.fade_len {
            let (old_l, old_r) = self.read(self.fading);
            let t = self.fade_pos as f32 / self.fade_len as f32;
            wet_l = old_l + (wet_l - old_l) * t;
            wet_r = old_r + (wet_r - old_r) * t;
            self.fading.pos += 1;
            self.fade_pos += 1;
        }
        self.head.pos += 1;
        if self.head.pos >= (window - self.fade_len) as usize {
            self.fading = self.head;
            self.head = self.newest_head();
            self.fade_pos = 0;
        }
        let gain = self.params.mix * self.wet.toward(if self.releasing { 0.0 } else { 1.0 });
        (left + (wet_l - left) * gain, right + (wet_r - right) * gain)
    }

    fn newest_head(&self) -> ReverseHead {
        ReverseHead {
            start: self.ring.index_back(0),
            pos: 0,
        }
    }

    fn read(&self, head: ReverseHead) -> (f32, f32) {
        let len = self.ring.len();
        self.ring
            .frame_at((head.start + len - head.pos % len) % len)
    }
}
