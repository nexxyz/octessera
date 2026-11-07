use super::{pct_param, pulses_len, synced_len, synced_pulses, GainRamp, ENVELOPE_MS, SLOWEST_BPM};
use crate::synth::fx::{fx_bus_state_from_params, process_delay, FxBusState};
use crate::synth::fx_params::FxBusParams;
use serde_json::Value;
use std::collections::BTreeMap;

/// Echoes below this level (-80 dB) count as gone.
const SILENT_LEVEL: f32 = 1.0e-4;
const TIME_GLIDE_MS: f32 = 50.0;
/// A tail still ringing this long after release fades out over `TAIL_FADE_MS`, so a long, high
/// feedback throw does not hold one of the two Play FX slots for a minute.
const TAIL_LIMIT_SECONDS: u32 = 10;
const TAIL_FADE_MS: u32 = 1_000;
const DEFAULT_PULSES: f32 = 24.0;
/// The longest Time a held throw can be retimed to: a 1/2 note.
const LONGEST_PULSES: f32 = 48.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::synth::engine) struct DubThrowParams {
    pub(in crate::synth::engine) delay_len: f32,
    pub(in crate::synth::engine) feedback: f32,
    pub(in crate::synth::engine) mix: f32,
}

impl DubThrowParams {
    pub(super) fn from_map(params: &BTreeMap<String, Value>, sample_rate: u32) -> Self {
        Self {
            delay_len: synced_len(params, "timePulses", DEFAULT_PULSES, sample_rate),
            feedback: pct_param(params, "feedbackPct", 55.0).min(0.9),
            mix: pct_param(params, "mixPct", 70.0),
        }
    }
}

/// Sends the target into a pair of bus delay lines while held. After release the send closes and
/// the echoes keep trailing off; the voice ends once a full delay time has passed in silence, or
/// after the tail limit and its fade.
#[derive(Clone, Debug)]
pub(in crate::synth::engine) struct DubThrowVoice {
    pub(super) params: DubThrowParams,
    delay_l: FxBusState,
    delay_r: FxBusState,
    max_len: f32,
    pub(super) current_len: f32,
    glide: f32,
    send: GainRamp,
    releasing: bool,
    quiet_frames: u32,
    tail_frames: u32,
    tail_limit: u32,
    tail: GainRamp,
    sample_rate: u32,
}

impl DubThrowVoice {
    pub(super) fn new(
        params: DubThrowParams,
        params_map: &BTreeMap<String, Value>,
        sample_rate: u32,
    ) -> Self {
        let pulses = synced_pulses(params_map, "timePulses", DEFAULT_PULSES).max(LONGEST_PULSES);
        let max_len = pulses_len(pulses, SLOWEST_BPM, sample_rate);
        let line = FxBusParams::Delay {
            time_ms: max_len * 1_000.0 / sample_rate as f32,
            feedback: 0.0,
            mix: 1.0,
            spread: 0.0,
        };
        Self {
            params,
            delay_l: fx_bus_state_from_params(&line, sample_rate),
            delay_r: fx_bus_state_from_params(&line, sample_rate),
            max_len,
            current_len: params.delay_len.min(max_len),
            glide: 1.0 / (TIME_GLIDE_MS * sample_rate as f32 / 1_000.0),
            send: GainRamp::new(0.0, ENVELOPE_MS, sample_rate),
            releasing: false,
            quiet_frames: 0,
            tail_frames: 0,
            tail_limit: TAIL_LIMIT_SECONDS * sample_rate,
            tail: GainRamp::new(1.0, TAIL_FADE_MS, sample_rate),
            sample_rate,
        }
    }

    /// A new press of the same type takes over the releasing voice with its own settings.
    pub(super) fn revive(&mut self, params: DubThrowParams) {
        self.params = params;
        self.releasing = false;
        self.tail_frames = 0;
    }

    pub(super) fn release(&mut self) {
        self.releasing = true;
    }

    pub(super) fn finished(&self) -> bool {
        self.releasing
            && self.send.value() <= 0.0
            && (self.quiet_frames as f32 > self.current_len + 64.0 || self.tail.value() <= 0.0)
    }

    pub(super) fn process(&mut self, left: f32, right: f32) -> (f32, f32) {
        let target = self.params.delay_len.min(self.max_len);
        let diff = target - self.current_len;
        self.current_len = if diff.abs() < 0.05 {
            target
        } else {
            self.current_len + diff * self.glide
        };
        let send = self.send.toward(if self.releasing { 0.0 } else { 1.0 });
        if self.releasing {
            self.tail_frames = self.tail_frames.saturating_add(1);
        }
        let tail = self.tail.toward(if self.tail_frames > self.tail_limit {
            0.0
        } else {
            1.0
        });
        let time_ms = self.current_len * 1_000.0 / self.sample_rate as f32;
        let feedback = self.params.feedback;
        let rate = self.sample_rate;
        let wet_l = process_delay(&mut self.delay_l, left * send, time_ms, feedback, 1.0, rate);
        let wet_r = process_delay(
            &mut self.delay_r,
            right * send,
            time_ms,
            feedback,
            1.0,
            rate,
        );
        if self.releasing && send <= 0.0 && wet_l.abs().max(wet_r.abs()) < SILENT_LEVEL {
            self.quiet_frames = self.quiet_frames.saturating_add(1);
        } else {
            self.quiet_frames = 0;
        }
        let mix = self.params.mix * tail;
        (left + wet_l * mix, right + wet_r * mix)
    }
}
