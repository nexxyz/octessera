use super::super::super::scalar_param::ScalarMutation;
use super::super::momentary_fx_state::MomentaryFxKind;
use super::super::PreparedMomentaryFxUpdate;
use super::{
    CrushParams, DubThrowParams, FilterSweepParams, FreezeParams, HpRiserParams, MomentaryFxVoice,
    PitchShiftParams, ReverseParams, StutterParams, TapeStopParams, TranceGateParams, WashParams,
};
use serde_json::Value;
use std::collections::BTreeMap;

impl MomentaryFxVoice {
    pub(in crate::synth::engine) fn apply_update(
        &mut self,
        update: PreparedMomentaryFxUpdate,
    ) -> ScalarMutation {
        fn replace<T: PartialEq>(current: &mut T, next: T) -> ScalarMutation {
            if *current == next {
                return ScalarMutation::Unchanged;
            }
            *current = next;
            ScalarMutation::Changed
        }
        match (self, update) {
            (
                Self::Stutter(voice),
                PreparedMomentaryFxUpdate::Stutter {
                    depth, segment_len, ..
                },
            ) => {
                let next = StutterParams { depth, segment_len };
                if voice.params == next {
                    return ScalarMutation::Unchanged;
                }
                voice.retime(next);
                ScalarMutation::Changed
            }
            (
                Self::Freeze(voice),
                PreparedMomentaryFxUpdate::Freeze {
                    mix, release_len, ..
                },
            ) => replace(&mut voice.params, FreezeParams { mix, release_len }),
            (
                Self::FilterSweep(voice),
                PreparedMomentaryFxUpdate::FilterSweep {
                    target_cutoff,
                    q,
                    sweep_in_step,
                    sweep_out_step,
                    ..
                },
            ) => replace(
                &mut voice.params,
                FilterSweepParams {
                    target_cutoff,
                    q,
                    sweep_in_step,
                    sweep_out_step,
                },
            ),
            (
                Self::PitchShift(voice),
                PreparedMomentaryFxUpdate::PitchShift {
                    target_octaves,
                    mix,
                    slide_in_len,
                    slide_out_len,
                    ..
                },
            ) => voice.update(PitchShiftParams {
                target_octaves,
                mix,
                slide_in_len,
                slide_out_len,
            }),
            (
                Self::TapeStop(voice),
                PreparedMomentaryFxUpdate::TapeStop {
                    stop_len,
                    start_len,
                    ..
                },
            ) => replace(
                &mut voice.params,
                TapeStopParams {
                    stop_len,
                    start_len,
                },
            ),
            (
                Self::Reverse(voice),
                PreparedMomentaryFxUpdate::Reverse {
                    window_len, mix, ..
                },
            ) => replace(&mut voice.params, ReverseParams { window_len, mix }),
            (
                Self::TranceGate(voice),
                PreparedMomentaryFxUpdate::TranceGate {
                    period,
                    depth,
                    smooth,
                    ..
                },
            ) => {
                let next = TranceGateParams {
                    period,
                    depth,
                    smooth,
                };
                if voice.params == next {
                    return ScalarMutation::Unchanged;
                }
                voice.retime(next);
                ScalarMutation::Changed
            }
            (
                Self::DubThrow(voice),
                PreparedMomentaryFxUpdate::DubThrow {
                    delay_len,
                    feedback,
                    mix,
                    ..
                },
            ) => replace(
                &mut voice.params,
                DubThrowParams {
                    delay_len,
                    feedback,
                    mix,
                },
            ),
            (
                Self::Wash(voice),
                PreparedMomentaryFxUpdate::Wash {
                    feedback,
                    mix,
                    release_len,
                    ..
                },
            ) => replace(
                &mut voice.params,
                WashParams {
                    feedback,
                    mix,
                    release_len,
                },
            ),
            (
                Self::HpRiser(voice),
                PreparedMomentaryFxUpdate::HpRiser {
                    target_octaves,
                    q,
                    sweep_in_step,
                    sweep_out_step,
                    ..
                },
            ) => replace(
                &mut voice.params,
                HpRiserParams {
                    target_octaves,
                    q,
                    sweep_in_step,
                    sweep_out_step,
                },
            ),
            (
                Self::Crush(voice),
                PreparedMomentaryFxUpdate::Crush {
                    bits,
                    rate_div,
                    mix,
                    ..
                },
            ) => replace(
                &mut voice.params,
                CrushParams {
                    bits,
                    rate_div,
                    mix,
                },
            ),
            _ => ScalarMutation::Rejected,
        }
    }
}

/// The prepared live update of a Play FX type.
pub(in crate::synth::engine) fn prepare_voice_update(
    kind: MomentaryFxKind,
    epoch: u64,
    params: &BTreeMap<String, Value>,
    sample_rate: u32,
) -> PreparedMomentaryFxUpdate {
    match kind {
        MomentaryFxKind::Stutter => {
            let StutterParams { depth, segment_len } = StutterParams::from_map(params, sample_rate);
            PreparedMomentaryFxUpdate::Stutter {
                epoch,
                depth,
                segment_len,
            }
        }
        MomentaryFxKind::Freeze => {
            let FreezeParams { mix, release_len } = FreezeParams::from_map(params, sample_rate);
            PreparedMomentaryFxUpdate::Freeze {
                epoch,
                mix,
                release_len,
            }
        }
        MomentaryFxKind::FilterSweep => {
            let FilterSweepParams {
                target_cutoff,
                q,
                sweep_in_step,
                sweep_out_step,
            } = FilterSweepParams::from_map(params, sample_rate);
            PreparedMomentaryFxUpdate::FilterSweep {
                epoch,
                target_cutoff,
                q,
                sweep_in_step,
                sweep_out_step,
            }
        }
        MomentaryFxKind::PitchShift => {
            let PitchShiftParams {
                target_octaves,
                mix,
                slide_in_len,
                slide_out_len,
            } = PitchShiftParams::from_map(params, sample_rate);
            PreparedMomentaryFxUpdate::PitchShift {
                epoch,
                target_octaves,
                mix,
                slide_in_len,
                slide_out_len,
            }
        }
        MomentaryFxKind::TapeStop => {
            let TapeStopParams {
                stop_len,
                start_len,
            } = TapeStopParams::from_map(params, sample_rate);
            PreparedMomentaryFxUpdate::TapeStop {
                epoch,
                stop_len,
                start_len,
            }
        }
        MomentaryFxKind::Reverse => {
            let ReverseParams { window_len, mix } = ReverseParams::from_map(params, sample_rate);
            PreparedMomentaryFxUpdate::Reverse {
                epoch,
                window_len,
                mix,
            }
        }
        MomentaryFxKind::TranceGate => {
            let TranceGateParams {
                period,
                depth,
                smooth,
            } = TranceGateParams::from_map(params, sample_rate);
            PreparedMomentaryFxUpdate::TranceGate {
                epoch,
                period,
                depth,
                smooth,
            }
        }
        MomentaryFxKind::DubThrow => {
            let DubThrowParams {
                delay_len,
                feedback,
                mix,
            } = DubThrowParams::from_map(params, sample_rate);
            PreparedMomentaryFxUpdate::DubThrow {
                epoch,
                delay_len,
                feedback,
                mix,
            }
        }
        MomentaryFxKind::Wash => {
            let WashParams {
                feedback,
                mix,
                release_len,
            } = WashParams::from_map(params, sample_rate);
            PreparedMomentaryFxUpdate::Wash {
                epoch,
                feedback,
                mix,
                release_len,
            }
        }
        MomentaryFxKind::HpRiser => {
            let HpRiserParams {
                target_octaves,
                q,
                sweep_in_step,
                sweep_out_step,
            } = HpRiserParams::from_map(params, sample_rate);
            PreparedMomentaryFxUpdate::HpRiser {
                epoch,
                target_octaves,
                q,
                sweep_in_step,
                sweep_out_step,
            }
        }
        MomentaryFxKind::Crush => {
            let CrushParams {
                bits,
                rate_div,
                mix,
            } = CrushParams::from_map(params);
            PreparedMomentaryFxUpdate::Crush {
                epoch,
                bits,
                rate_div,
                mix,
            }
        }
    }
}

/// The parameter keys a Play FX type accepts.
pub(in crate::synth::engine) fn voice_param_keys(kind: MomentaryFxKind) -> &'static [&'static str] {
    match kind {
        MomentaryFxKind::Stutter => &["depthPct", "rateHz"],
        MomentaryFxKind::Freeze => &["mixPct", "releaseMs"],
        MomentaryFxKind::FilterSweep => &["cutoffPct", "resonancePct", "sweepInMs", "sweepOutMs"],
        MomentaryFxKind::PitchShift => &["semitones", "cents", "mixPct", "slideInMs", "slideOutMs"],
        MomentaryFxKind::TapeStop => &["stopMs", "startMs"],
        MomentaryFxKind::Reverse => &["lengthMs", "mixPct"],
        MomentaryFxKind::TranceGate => &["ratePulses", "depthPct", "smoothPct", "tempoBpm"],
        MomentaryFxKind::DubThrow => &["timePulses", "feedbackPct", "mixPct", "tempoBpm"],
        MomentaryFxKind::Wash => &["sizePct", "mixPct", "releaseMs"],
        MomentaryFxKind::HpRiser => &["cutoffPct", "resonancePct", "sweepInMs", "sweepOutMs"],
        MomentaryFxKind::Crush => &["bits", "rateDiv", "mixPct"],
    }
}
