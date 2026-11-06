use super::capture_ring::CaptureRing;
use super::{ms_param_len, GainRamp, HANDOFF_MS};
use serde_json::Value;
use std::collections::BTreeMap;

/// Below this speed the stopping tape fades to silence, so a full stop has no DC step.
const SILENT_SPEED: f32 = 0.15;
/// Lag grows by at most half of Stop plus half of Start, so two seconds plus slack covers both.
const RING_SECONDS: u32 = 2;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::synth::engine) struct TapeStopParams {
    pub(in crate::synth::engine) stop_len: u32,
    pub(in crate::synth::engine) start_len: u32,
}

impl TapeStopParams {
    pub(super) fn from_map(params: &BTreeMap<String, Value>, sample_rate: u32) -> Self {
        Self {
            stop_len: ms_param_len(params, "stopMs", 700.0, 50.0, 2_000.0, sample_rate),
            start_len: ms_param_len(params, "startMs", 300.0, 10.0, 2_000.0, sample_rate),
        }
    }
}

/// Plays the target from a capture ring whose read speed ramps to zero while held and back to
/// one after release; the read point falls behind by the integral of `1 - speed`.
#[derive(Clone)]
pub(in crate::synth::engine) struct TapeStopVoice {
    pub(super) params: TapeStopParams,
    ring: CaptureRing,
    pub(super) speed: f32,
    lag: f32,
    releasing: bool,
    handoff: GainRamp,
}

impl TapeStopVoice {
    pub(super) fn new(params: TapeStopParams, sample_rate: u32) -> Self {
        Self {
            params,
            ring: CaptureRing::new((sample_rate * RING_SECONDS) as usize + 64),
            speed: 1.0,
            lag: 0.0,
            releasing: false,
            handoff: GainRamp::new(1.0, HANDOFF_MS, sample_rate),
        }
    }

    pub(super) fn release(&mut self) {
        self.releasing = true;
        if self.speed <= 0.0 {
            self.lag = 0.0;
        }
    }

    pub(super) fn finished(&self) -> bool {
        self.releasing && self.speed >= 1.0 && self.handoff.value() <= 0.0
    }

    pub(super) fn process(&mut self, left: f32, right: f32) -> (f32, f32) {
        self.ring.push(left, right);
        if self.releasing {
            self.speed = (self.speed + 1.0 / self.params.start_len as f32).min(1.0);
        } else {
            self.speed = (self.speed - 1.0 / self.params.stop_len as f32).max(0.0);
        }
        if self.speed > 0.0 {
            let max_lag = (self.ring.len() - 3) as f32;
            self.lag = (self.lag + 1.0 - self.speed).min(max_lag);
        }
        let (tape_l, tape_r) = self.ring.read_back(self.lag);
        let level = (self.speed / SILENT_SPEED).min(1.0);
        let wet = if self.releasing && self.speed >= 1.0 {
            self.handoff.toward(0.0)
        } else {
            1.0
        };
        let tape_gain = level * wet;
        let dry_gain = 1.0 - wet;
        (
            tape_l * tape_gain + left * dry_gain,
            tape_r * tape_gain + right * dry_gain,
        )
    }
}
