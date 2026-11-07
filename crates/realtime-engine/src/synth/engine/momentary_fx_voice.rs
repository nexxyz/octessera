use super::momentary_fx_state::MomentaryFxKind;
use super::support::param_f32;
use crate::synth::runtime_state::ms_to_samples;
use serde_json::Value;
use std::collections::BTreeMap;

mod capture_ring;
mod crush;
mod dub_throw;
mod filter_sweep;
mod freeze;
mod hp_riser;
mod pitch_shift;
mod reverse;
mod stutter;
mod tape_stop;
mod trance_gate;
mod updates;
mod wash;

use crush::{CrushParams, CrushVoice};
use dub_throw::{DubThrowParams, DubThrowVoice};
use filter_sweep::{FilterSweepParams, FilterSweepVoice};
#[cfg(test)]
pub(super) use freeze::FREEZE_INJECT_MS;
use freeze::{FreezeParams, FreezeVoice};
use hp_riser::{HpRiserParams, HpRiserVoice};
pub(super) use pitch_shift::PitchStopAction;
use pitch_shift::{PitchShiftParams, PitchShiftVoice};
use reverse::{ReverseParams, ReverseVoice};
use stutter::{StutterParams, StutterVoice};
use tape_stop::{TapeStopParams, TapeStopVoice};
use trance_gate::{TranceGateParams, TranceGateVoice};
pub(super) use updates::{prepare_voice_update, voice_param_keys};
use wash::{WashParams, WashVoice};

/// Fade length for the press and release envelopes of gate, crush, dub throw and wash sends, and
/// for a releasing tail a new press retires.
pub(super) const ENVELOPE_MS: u32 = 5;
/// Fade length where a voice hands its wet path back to the dry signal.
const HANDOFF_MS: u32 = 10;
/// Tempo-synced lengths follow `tempoBpm`, which the runtime resolves from the transport.
const DEFAULT_BPM: f32 = 120.0;
const SLOWEST_BPM: f32 = 40.0;
const FASTEST_BPM: f32 = 240.0;
const PULSES_PER_BEAT: f32 = 24.0;

/// The DSP state of a Play FX, with its parameters next to it. The type is chosen once per frame
/// for the active effect; each type has its own frame function.
#[derive(Clone, Debug)]
pub(super) enum MomentaryFxVoice {
    Stutter(Box<StutterVoice>),
    Freeze(Box<FreezeVoice>),
    FilterSweep(Box<FilterSweepVoice>),
    PitchShift(Box<PitchShiftVoice>),
    TapeStop(Box<TapeStopVoice>),
    Reverse(Box<ReverseVoice>),
    TranceGate(Box<TranceGateVoice>),
    DubThrow(Box<DubThrowVoice>),
    Wash(Box<WashVoice>),
    HpRiser(Box<HpRiserVoice>),
    Crush(Box<CrushVoice>),
}

impl MomentaryFxVoice {
    pub(super) fn new(
        kind: MomentaryFxKind,
        params: &BTreeMap<String, Value>,
        sample_rate: u32,
    ) -> Self {
        match kind {
            MomentaryFxKind::Stutter => Self::Stutter(Box::new(StutterVoice::new(
                StutterParams::from_map(params, sample_rate),
                sample_rate,
            ))),
            MomentaryFxKind::Freeze => Self::Freeze(Box::new(FreezeVoice::new(
                FreezeParams::from_map(params, sample_rate),
                sample_rate,
            ))),
            MomentaryFxKind::FilterSweep => Self::FilterSweep(Box::new(FilterSweepVoice::new(
                FilterSweepParams::from_map(params, sample_rate),
                sample_rate,
            ))),
            MomentaryFxKind::PitchShift => Self::PitchShift(Box::new(PitchShiftVoice::new(
                PitchShiftParams::from_map(params, sample_rate),
                sample_rate,
            ))),
            MomentaryFxKind::TapeStop => Self::TapeStop(Box::new(TapeStopVoice::new(
                TapeStopParams::from_map(params, sample_rate),
                sample_rate,
            ))),
            MomentaryFxKind::Reverse => Self::Reverse(Box::new(ReverseVoice::new(
                ReverseParams::from_map(params, sample_rate),
                sample_rate,
            ))),
            MomentaryFxKind::TranceGate => Self::TranceGate(Box::new(TranceGateVoice::new(
                TranceGateParams::from_map(params, sample_rate),
                sample_rate,
            ))),
            MomentaryFxKind::DubThrow => Self::DubThrow(Box::new(DubThrowVoice::new(
                DubThrowParams::from_map(params, sample_rate),
                params,
                sample_rate,
            ))),
            MomentaryFxKind::Wash => Self::Wash(Box::new(WashVoice::new(
                WashParams::from_map(params, sample_rate),
                sample_rate,
            ))),
            MomentaryFxKind::HpRiser => Self::HpRiser(Box::new(HpRiserVoice::new(
                HpRiserParams::from_map(params, sample_rate),
                sample_rate,
            ))),
            MomentaryFxKind::Crush => Self::Crush(Box::new(CrushVoice::new(
                CrushParams::from_map(params),
                sample_rate,
            ))),
        }
    }

    pub(super) fn process(&mut self, left: f32, right: f32) -> (f32, f32) {
        match self {
            Self::Stutter(voice) => voice.process(left, right),
            Self::Freeze(voice) => voice.process(left, right),
            Self::FilterSweep(voice) => voice.process(left, right),
            Self::PitchShift(voice) => voice.process(left, right),
            Self::TapeStop(voice) => voice.process(left, right),
            Self::Reverse(voice) => voice.process(left, right),
            Self::TranceGate(voice) => voice.process(left, right),
            Self::DubThrow(voice) => voice.process(left, right),
            Self::Wash(voice) => voice.process(left, right),
            Self::HpRiser(voice) => voice.process(left, right),
            Self::Crush(voice) => voice.process(left, right),
        }
    }

    pub(super) fn release(&mut self) {
        match self {
            Self::Stutter(voice) => voice.release(),
            Self::Freeze(voice) => voice.release(),
            Self::FilterSweep(voice) => voice.release(),
            Self::PitchShift(voice) => {
                let _ = voice.stop();
            }
            Self::TapeStop(voice) => voice.release(),
            Self::Reverse(voice) => voice.release(),
            Self::TranceGate(voice) => voice.release(),
            Self::DubThrow(voice) => voice.release(),
            Self::Wash(voice) => voice.release(),
            Self::HpRiser(voice) => voice.release(),
            Self::Crush(voice) => voice.release(),
        }
    }

    /// Hands a releasing voice the settings of a new press of its type; false for other pairs
    /// and for the types that end their release on their own schedule.
    pub(super) fn revive_with(&mut self, incoming: &Self) -> bool {
        match (self, incoming) {
            (Self::TapeStop(voice), Self::TapeStop(next)) => voice.revive(next.params),
            (Self::Reverse(voice), Self::Reverse(next)) => voice.revive(next.params),
            (Self::TranceGate(voice), Self::TranceGate(next)) => voice.revive(next.params),
            (Self::DubThrow(voice), Self::DubThrow(next)) => voice.revive(next.params),
            (Self::Wash(voice), Self::Wash(next)) => voice.revive(next.params),
            (Self::HpRiser(voice), Self::HpRiser(next)) => voice.revive(next.params),
            (Self::Crush(voice), Self::Crush(next)) => voice.revive(next.params),
            _ => return false,
        }
        true
    }

    pub(super) fn finished(&self) -> bool {
        match self {
            Self::Stutter(voice) => voice.finished(),
            Self::Freeze(voice) => voice.finished(),
            Self::FilterSweep(voice) => voice.finished(),
            Self::PitchShift(voice) => voice.finished(),
            Self::TapeStop(voice) => voice.finished(),
            Self::Reverse(voice) => voice.finished(),
            Self::TranceGate(voice) => voice.finished(),
            Self::DubThrow(voice) => voice.finished(),
            Self::Wash(voice) => voice.finished(),
            Self::HpRiser(voice) => voice.finished(),
            Self::Crush(voice) => voice.finished(),
        }
    }
}

#[cfg(test)]
impl MomentaryFxVoice {
    pub(super) fn stutter(&self) -> &StutterVoice {
        let Self::Stutter(voice) = self else {
            panic!("not a stutter voice");
        };
        voice
    }

    pub(super) fn freeze(&self) -> &FreezeVoice {
        let Self::Freeze(voice) = self else {
            panic!("not a freeze voice");
        };
        voice
    }

    pub(super) fn freeze_mut(&mut self) -> &mut FreezeVoice {
        let Self::Freeze(voice) = self else {
            panic!("not a freeze voice");
        };
        voice
    }

    pub(super) fn pitch(&self) -> &PitchShiftVoice {
        let Self::PitchShift(voice) = self else {
            panic!("not a pitch_shift voice");
        };
        voice
    }

    pub(super) fn pitch_mut(&mut self) -> &mut PitchShiftVoice {
        let Self::PitchShift(voice) = self else {
            panic!("not a pitch_shift voice");
        };
        voice
    }
}

/// A linear gain envelope moving toward a target by a fixed step per frame.
#[derive(Clone, Copy, Debug)]
pub(super) struct GainRamp {
    value: f32,
    step: f32,
}

impl GainRamp {
    pub(super) fn new(value: f32, ms: u32, sample_rate: u32) -> Self {
        Self {
            value,
            step: 1.0 / (sample_rate * ms / 1_000).max(1) as f32,
        }
    }

    pub(super) fn value(self) -> f32 {
        self.value
    }

    pub(super) fn toward(&mut self, target: f32) -> f32 {
        self.value = if self.value < target {
            (self.value + self.step).min(target)
        } else {
            (self.value - self.step).max(target)
        };
        self.value
    }

    fn fall_by(&mut self, step: f32) -> f32 {
        self.value = (self.value - step).max(0.0);
        self.value
    }
}

fn pct_param(params: &BTreeMap<String, Value>, key: &str, fallback: f32) -> f32 {
    (param_f32(params, key, fallback) / 100.0).clamp(0.0, 1.0)
}

fn ms_param_len(
    params: &BTreeMap<String, Value>,
    key: &str,
    fallback: f32,
    min: f32,
    max: f32,
    sample_rate: u32,
) -> u32 {
    ms_to_samples(
        param_f32(params, key, fallback).clamp(min, max),
        sample_rate,
    )
    .max(1)
}

fn synced_pulses(params: &BTreeMap<String, Value>, key: &str, fallback: f32) -> f32 {
    param_f32(params, key, fallback).round().clamp(1.0, 96.0)
}

/// Frames in `pulses` 24-PPQN clock pulses at `bpm`.
fn pulses_len(pulses: f32, bpm: f32, sample_rate: u32) -> f32 {
    pulses * 60.0 * sample_rate as f32 / (bpm * PULSES_PER_BEAT)
}

fn synced_len(
    params: &BTreeMap<String, Value>,
    key: &str,
    fallback_pulses: f32,
    sample_rate: u32,
) -> f32 {
    let bpm = param_f32(params, "tempoBpm", DEFAULT_BPM).clamp(SLOWEST_BPM, FASTEST_BPM);
    pulses_len(
        synced_pulses(params, key, fallback_pulses),
        bpm,
        sample_rate,
    )
}

#[cfg(test)]
mod tests;
