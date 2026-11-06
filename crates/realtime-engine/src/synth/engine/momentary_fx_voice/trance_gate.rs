use super::{pct_param, synced_len, GainRamp, ENVELOPE_MS};
use serde_json::Value;
use std::collections::BTreeMap;

/// The shortest gate edge, even at Smooth 0, so a hard gate does not click.
const MIN_EDGE_MS: f32 = 1.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::synth::engine) struct TranceGateParams {
    pub(in crate::synth::engine) period: f32,
    pub(in crate::synth::engine) depth: f32,
    pub(in crate::synth::engine) smooth: f32,
}

impl TranceGateParams {
    pub(super) fn from_map(params: &BTreeMap<String, Value>, sample_rate: u32) -> Self {
        Self {
            period: synced_len(params, "ratePulses", 6.0, sample_rate),
            depth: pct_param(params, "depthPct", 100.0),
            smooth: pct_param(params, "smoothPct", 20.0),
        }
    }
}

/// A tempo-synced square gate, open for the first half of each period, with smoothstep edges.
/// It starts open at the press; Depth eases in and out so the press and release do not click.
#[derive(Clone)]
pub(in crate::synth::engine) struct TranceGateVoice {
    pub(super) params: TranceGateParams,
    pub(super) phase: f32,
    min_edge: f32,
    depth: GainRamp,
    releasing: bool,
}

impl TranceGateVoice {
    pub(super) fn new(params: TranceGateParams, sample_rate: u32) -> Self {
        Self {
            params,
            phase: 0.0,
            min_edge: MIN_EDGE_MS * sample_rate as f32 / 1_000.0,
            depth: GainRamp::new(0.0, ENVELOPE_MS, sample_rate),
            releasing: false,
        }
    }

    /// A new press takes over the releasing gate; it keeps its place in the period.
    pub(super) fn revive(&mut self, params: TranceGateParams) {
        self.retime(params);
        self.releasing = false;
    }

    pub(super) fn release(&mut self) {
        self.releasing = true;
    }

    pub(super) fn finished(&self) -> bool {
        self.releasing && self.depth.value() <= 0.0
    }

    /// Keeps the gate's place within the period when the tempo changes.
    pub(super) fn retime(&mut self, params: TranceGateParams) {
        self.phase = self.phase / self.params.period * params.period;
        self.params = params;
    }

    pub(super) fn process(&mut self, left: f32, right: f32) -> (f32, f32) {
        let period = self.params.period;
        let half = period * 0.5;
        let edge = (self.params.smooth * half * 0.5)
            .max(self.min_edge)
            .min(half * 0.5);
        let pos = self.phase;
        let open = if pos < half - edge {
            1.0
        } else if pos < half {
            1.0 - smoothstep((pos - (half - edge)) / edge)
        } else if pos < period - edge {
            0.0
        } else {
            smoothstep((pos - (period - edge)) / edge)
        };
        self.phase += 1.0;
        if self.phase >= period {
            self.phase -= period;
        }
        let depth = self.params.depth * self.depth.toward(if self.releasing { 0.0 } else { 1.0 });
        let gain = 1.0 - depth * (1.0 - open);
        (left * gain, right * gain)
    }
}

fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
