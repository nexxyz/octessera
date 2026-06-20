use super::fx_params::{DuckSource, FilterLfoKind, FxBusParams};
use super::runtime_state::*;
use super::types::*;
use std::f32::consts::PI;

mod algorithms;

use algorithms::{
    compressor_gain, mix_sample, process_bitcrusher, process_compressor, process_distortion,
    process_duck, process_eq, process_eq_channel, process_filter_lfo, process_glitch,
    process_mod_delay, process_saturator, process_vinyl_stereo, read_delay, wrap_phase,
    CompressorParams, DuckParams, EqChannelState, EqParams, FilterLfoParams, ModDelayParams,
    VinylParams, VinylState,
};

#[derive(Clone, Debug)]
pub(super) enum FxBusState {
    None,
    Tremolo {
        phase: f32,
    },
    Delay {
        buf: Vec<f32>,
        idx: usize,
    },
    ModDelay {
        buf: Vec<f32>,
        idx: usize,
        phase: f32,
    },
    FilterLfo {
        filt: BiquadState,
        phase: f32,
    },
    Duck {
        env: f32,
    },
    Bitcrusher {
        hold: u32,
        count: u32,
        last: f32,
    },
    Reverb {
        bufs: [Vec<f32>; 4],
        idxs: [usize; 4],
        lp: [f32; 4],
    },
    Glitch {
        buf: Vec<f32>,
        idx: usize,
        read: usize,
        remain: usize,
        rng: u32,
    },
    AutoPan {
        phase: f32,
        pos: f32,
    },
    Compressor {
        env: f32,
    },
    Eq {
        channel: EqChannelState,
    },
    Vinyl(VinylState),
}

#[derive(Clone, Debug)]
pub(super) enum MasterFxState {
    None,
    Compressor {
        env: f32,
    },
    Eq {
        left: Box<EqChannelState>,
        right: Box<EqChannelState>,
    },
    Vinyl(VinylState),
}

pub(super) fn fx_bus_state_from_params(params: &FxBusParams, sample_rate: u32) -> FxBusState {
    match params {
        FxBusParams::Delay { time_ms, .. } => FxBusState::Delay {
            buf: vec![0.0; ((*time_ms / 1000.0) * sample_rate as f32).round().max(1.0) as usize],
            idx: 0,
        },
        FxBusParams::Tremolo { .. } => FxBusState::Tremolo { phase: 0.0 },
        FxBusParams::ModDelay {
            depth_ms, base_ms, ..
        } => FxBusState::ModDelay {
            buf: vec![
                0.0;
                (((*base_ms + *depth_ms + 5.0) / 1000.0) * sample_rate as f32)
                    .ceil()
                    .max(2.0) as usize
            ],
            idx: 0,
            phase: 0.0,
        },
        FxBusParams::FilterLfo { .. } => FxBusState::FilterLfo {
            filt: BiquadState::new(),
            phase: 0.0,
        },
        FxBusParams::Duck { .. } => FxBusState::Duck { env: 0.0 },
        FxBusParams::Bitcrusher { .. } => FxBusState::Bitcrusher {
            hold: 1,
            count: 0,
            last: 0.0,
        },
        FxBusParams::Reverb { .. } => FxBusState::Reverb {
            bufs: [1557, 1617, 1491, 1422]
                .map(|n| vec![0.0; (n * sample_rate as usize / 44_100).max(1)]),
            idxs: [0; 4],
            lp: [0.0; 4],
        },
        FxBusParams::Glitch { .. } => FxBusState::Glitch {
            buf: vec![0.0; ((sample_rate as f32) * 0.25) as usize],
            idx: 0,
            read: 0,
            remain: 0,
            rng: 0x1234_abcd,
        },
        FxBusParams::AutoPan { .. } => FxBusState::AutoPan {
            phase: 0.0,
            pos: 0.5,
        },
        FxBusParams::Compressor { .. } => FxBusState::Compressor { env: 0.0 },
        FxBusParams::Eq { .. } => FxBusState::Eq {
            channel: EqChannelState::new(),
        },
        FxBusParams::Vinyl { .. } => FxBusState::Vinyl(VinylState::new()),
        _ => FxBusState::None,
    }
}

pub(super) fn master_fx_state_from_params(params: &FxBusParams) -> MasterFxState {
    match params {
        FxBusParams::Compressor { .. } => MasterFxState::Compressor { env: 0.0 },
        FxBusParams::Eq { .. } => MasterFxState::Eq {
            left: Box::new(EqChannelState::new()),
            right: Box::new(EqChannelState::new()),
        },
        FxBusParams::Vinyl { .. } => MasterFxState::Vinyl(VinylState::new()),
        _ => MasterFxState::None,
    }
}

pub(super) fn fx_bus_state_matches_params(state: &FxBusState, params: &FxBusParams) -> bool {
    matches!(
        (state, params),
        (FxBusState::None, FxBusParams::None)
            | (FxBusState::None, FxBusParams::Saturator { .. })
            | (FxBusState::None, FxBusParams::Distortion { .. })
            | (FxBusState::Tremolo { .. }, FxBusParams::Tremolo { .. })
            | (FxBusState::Delay { .. }, FxBusParams::Delay { .. })
            | (FxBusState::ModDelay { .. }, FxBusParams::ModDelay { .. })
            | (FxBusState::FilterLfo { .. }, FxBusParams::FilterLfo { .. })
            | (FxBusState::Duck { .. }, FxBusParams::Duck { .. })
            | (
                FxBusState::Bitcrusher { .. },
                FxBusParams::Bitcrusher { .. }
            )
            | (FxBusState::Reverb { .. }, FxBusParams::Reverb { .. })
            | (FxBusState::Glitch { .. }, FxBusParams::Glitch { .. })
            | (FxBusState::AutoPan { .. }, FxBusParams::AutoPan { .. })
            | (
                FxBusState::Compressor { .. },
                FxBusParams::Compressor { .. }
            )
            | (FxBusState::Eq { .. }, FxBusParams::Eq { .. })
            | (FxBusState::Vinyl(..), FxBusParams::Vinyl { .. })
    )
}

pub(super) fn master_fx_state_matches_params(state: &MasterFxState, params: &FxBusParams) -> bool {
    matches!(
        (state, params),
        (MasterFxState::None, FxBusParams::None)
            | (MasterFxState::None, FxBusParams::Saturator { .. })
            | (MasterFxState::None, FxBusParams::Distortion { .. })
            | (
                MasterFxState::Compressor { .. },
                FxBusParams::Compressor { .. }
            )
            | (MasterFxState::Eq { .. }, FxBusParams::Eq { .. })
            | (MasterFxState::Vinyl(..), FxBusParams::Vinyl { .. })
    )
}

pub(super) fn process_fx_bus_slot(
    params: &FxBusParams,
    state: &mut FxBusState,
    input: f32,
    slot_out: &[f32; INSTRUMENT_SLOT_COUNT],
    bus_in: &[f32],
    sample_rate: u32,
) -> f32 {
    match *params {
        FxBusParams::None => input,
        FxBusParams::Tremolo { rate_hz, depth } => {
            let FxBusState::Tremolo { phase } = state else {
                *state = FxBusState::Tremolo { phase: 0.0 };
                return input;
            };
            let gain = (1.0 - depth) + depth * ((phase.sin() + 1.0) * 0.5);
            *phase = wrap_phase(*phase + 2.0 * PI * rate_hz / sample_rate as f32);
            input * gain
        }
        FxBusParams::Delay {
            time_ms,
            feedback,
            mix,
        } => process_delay(state, input, time_ms, feedback, mix, sample_rate),
        FxBusParams::ModDelay {
            rate_hz,
            depth_ms,
            base_ms,
            feedback,
            mix,
        } => process_mod_delay(
            state,
            input,
            ModDelayParams {
                rate_hz,
                depth_ms,
                base_ms,
                feedback,
                mix,
            },
            sample_rate,
        ),
        FxBusParams::FilterLfo {
            kind,
            rate_hz,
            depth,
            center_hz,
            q,
        } => process_filter_lfo(
            state,
            input,
            FilterLfoParams {
                kind,
                rate_hz,
                depth,
                center_hz,
                q,
            },
            sample_rate,
        ),
        FxBusParams::Reverb { mix, decay, damp } => {
            let FxBusState::Reverb { bufs, idxs, lp } = state else {
                *state = fx_bus_state_from_params(params, sample_rate);
                return input;
            };
            let mut wet = 0.0;
            for i in 0..4 {
                let delayed = bufs[i][idxs[i]];
                lp[i] = delayed * (1.0 - damp) + lp[i] * damp;
                bufs[i][idxs[i]] = input + lp[i] * decay;
                idxs[i] = (idxs[i] + 1) % bufs[i].len();
                wet += delayed;
            }
            (input * (1.0 - mix) + wet * 0.25 * mix).clamp(-1.5, 1.5)
        }
        FxBusParams::Glitch {
            chance,
            slice_ms,
            mix,
        } => process_glitch(state, input, chance, slice_ms, mix, sample_rate),
        FxBusParams::AutoPan { rate_hz, depth } => {
            let FxBusState::AutoPan { phase, pos } = state else {
                *state = FxBusState::AutoPan {
                    phase: 0.0,
                    pos: 0.5,
                };
                return input;
            };
            *pos = 0.5 + ((*phase).sin() * 0.5 * depth);
            *phase = wrap_phase(*phase + 2.0 * PI * rate_hz / sample_rate as f32);
            input
        }
        FxBusParams::Duck {
            source,
            threshold,
            amount,
            attack_ms,
            release_ms,
        } => process_duck(
            state,
            input,
            DuckParams {
                source,
                threshold,
                amount,
                attack_ms,
                release_ms,
            },
            slot_out,
            bus_in,
            sample_rate,
        ),
        FxBusParams::Saturator { drive, mix } => process_saturator(input, drive, mix),
        FxBusParams::Distortion { drive, clip, mix } => process_distortion(input, drive, clip, mix),
        FxBusParams::Bitcrusher {
            rate_div,
            bits,
            mix,
        } => process_bitcrusher(state, input, rate_div, bits, mix),
        FxBusParams::Compressor {
            threshold_db,
            ratio,
            attack_ms,
            release_ms,
            makeup_db,
            mix,
        } => process_compressor(
            state,
            input,
            CompressorParams {
                threshold_db,
                ratio,
                attack_ms,
                release_ms,
                makeup_db,
                mix,
            },
            sample_rate,
        ),
        FxBusParams::Eq {
            low_gain_db,
            mid_gain_db,
            mid_freq_hz,
            mid_q,
            high_gain_db,
            mix,
        } => process_eq(
            state,
            input,
            EqParams {
                low_gain_db,
                mid_gain_db,
                mid_freq_hz,
                mid_q,
                high_gain_db,
                mix,
            },
            sample_rate,
        ),
        FxBusParams::Vinyl {
            saturation,
            crackle,
            warp_depth,
            mix,
        } => {
            let FxBusState::Vinyl(vinyl) = state else {
                *state = FxBusState::Vinyl(VinylState::new());
                return input;
            };
            let (left, _) = process_vinyl_stereo(
                vinyl,
                input,
                input,
                VinylParams {
                    saturation,
                    crackle,
                    warp_depth,
                    mix,
                },
                sample_rate,
            );
            left
        }
    }
}

pub(super) fn process_master_fx_slot(
    params: &FxBusParams,
    state: &mut MasterFxState,
    left: f32,
    right: f32,
    sample_rate: u32,
) -> (f32, f32) {
    match *params {
        FxBusParams::None => (left, right),
        FxBusParams::Saturator { drive, mix } => (
            process_saturator(left, drive, mix),
            process_saturator(right, drive, mix),
        ),
        FxBusParams::Distortion { drive, clip, mix } => (
            process_distortion(left, drive, clip, mix),
            process_distortion(right, drive, clip, mix),
        ),
        FxBusParams::Compressor {
            threshold_db,
            ratio,
            attack_ms,
            release_ms,
            makeup_db,
            mix,
        } => {
            let MasterFxState::Compressor { env } = state else {
                *state = MasterFxState::Compressor { env: 0.0 };
                return (left, right);
            };
            let params = CompressorParams {
                threshold_db,
                ratio,
                attack_ms,
                release_ms,
                makeup_db,
                mix,
            };
            let detector = left.abs().max(right.abs()).min(1.0);
            let gain = compressor_gain(env, detector, &params, sample_rate);
            (
                mix_sample(left, left * gain, mix),
                mix_sample(right, right * gain, mix),
            )
        }
        FxBusParams::Eq {
            low_gain_db,
            mid_gain_db,
            mid_freq_hz,
            mid_q,
            high_gain_db,
            mix,
        } => {
            let MasterFxState::Eq { left: l, right: r } = state else {
                *state = master_fx_state_from_params(params);
                return (left, right);
            };
            let eq_params = EqParams {
                low_gain_db,
                mid_gain_db,
                mid_freq_hz,
                mid_q,
                high_gain_db,
                mix,
            };
            let wet_l = process_eq_channel(l.as_mut(), left, &eq_params, sample_rate);
            let wet_r = process_eq_channel(r.as_mut(), right, &eq_params, sample_rate);
            (mix_sample(left, wet_l, mix), mix_sample(right, wet_r, mix))
        }
        FxBusParams::Vinyl {
            saturation,
            crackle,
            warp_depth,
            mix,
        } => {
            let MasterFxState::Vinyl(vinyl) = state else {
                *state = MasterFxState::Vinyl(VinylState::new());
                return (left, right);
            };
            process_vinyl_stereo(
                vinyl,
                left,
                right,
                VinylParams {
                    saturation,
                    crackle,
                    warp_depth,
                    mix,
                },
                sample_rate,
            )
        }
        _ => (left, right),
    }
}

fn process_delay(
    state: &mut FxBusState,
    input: f32,
    time_ms: f32,
    feedback: f32,
    mix: f32,
    sample_rate: u32,
) -> f32 {
    let delay_samples = (time_ms / 1000.0) * sample_rate as f32;
    let desired_len = delay_samples.ceil() as usize + 1;
    let FxBusState::Delay { buf, idx } = state else {
        *state = FxBusState::Delay {
            buf: vec![0.0; desired_len.max(2)],
            idx: 0,
        };
        return input;
    };
    if buf.len() < desired_len.max(2) {
        buf.resize(desired_len.max(2), 0.0);
    }
    let delayed = read_delay(buf, *idx, delay_samples);
    buf[*idx] = input + delayed * feedback;
    *idx = (*idx + 1) % buf.len();
    (input * (1.0 - mix) + delayed * mix).clamp(-1.5, 1.5)
}
