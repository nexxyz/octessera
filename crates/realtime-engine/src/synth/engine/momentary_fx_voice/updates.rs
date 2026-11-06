use super::super::super::scalar_param::ScalarMutation;
use super::super::momentary_fx_state::MomentaryFxKind;
use super::super::PreparedMomentaryFxUpdate;
use super::{
    CrushParams, DubThrowParams, HpRiserParams, MomentaryFxVoice, ReverseParams, TapeStopParams,
    TranceGateParams, WashParams,
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

/// The prepared live update for a voice type, or `None` for the other Play FX types.
pub(in crate::synth::engine) fn prepare_voice_update(
    kind: MomentaryFxKind,
    epoch: u64,
    params: &BTreeMap<String, Value>,
    sample_rate: u32,
) -> Option<PreparedMomentaryFxUpdate> {
    Some(match kind {
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
        MomentaryFxKind::Stutter
        | MomentaryFxKind::Freeze
        | MomentaryFxKind::FilterSweep
        | MomentaryFxKind::PitchShift => return None,
    })
}

/// The parameter keys a voice type accepts, or `None` for the other Play FX types.
pub(in crate::synth::engine) fn voice_param_keys(
    kind: MomentaryFxKind,
) -> Option<&'static [&'static str]> {
    Some(match kind {
        MomentaryFxKind::TapeStop => &["stopMs", "startMs"],
        MomentaryFxKind::Reverse => &["lengthMs", "mixPct"],
        MomentaryFxKind::TranceGate => &["ratePulses", "depthPct", "smoothPct", "tempoBpm"],
        MomentaryFxKind::DubThrow => &["timePulses", "feedbackPct", "mixPct", "tempoBpm"],
        MomentaryFxKind::Wash => &["sizePct", "mixPct", "releaseMs"],
        MomentaryFxKind::HpRiser => &["cutoffPct", "resonancePct", "sweepInMs", "sweepOutMs"],
        MomentaryFxKind::Crush => &["bits", "rateDiv", "mixPct"],
        MomentaryFxKind::Stutter
        | MomentaryFxKind::Freeze
        | MomentaryFxKind::FilterSweep
        | MomentaryFxKind::PitchShift => return None,
    })
}
