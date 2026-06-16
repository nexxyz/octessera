use super::fx_params::{DuckSource, FilterLfoKind, FxBusParams};
use super::runtime_state::*;
use super::types::*;
use std::f32::consts::PI;

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
        left: EqChannelState,
        right: EqChannelState,
    },
    Vinyl(VinylState),
}

#[derive(Clone, Debug)]
pub(super) struct EqChannelState {
    low: BiquadState,
    mid: BiquadState,
    high: BiquadState,
}

impl EqChannelState {
    fn new() -> Self {
        Self {
            low: BiquadState::new(),
            mid: BiquadState::new(),
            high: BiquadState::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct VinylState {
    wow_phase: f32,
    flutter_phase: f32,
    crackle_amp: f32,
    crackle_pan: f32,
    rng: u32,
    tone_l: f32,
    tone_r: f32,
}

impl VinylState {
    fn new() -> Self {
        Self {
            wow_phase: 0.0,
            flutter_phase: 0.0,
            crackle_amp: 0.0,
            crackle_pan: 0.0,
            rng: 0x4d59_5df4,
            tone_l: 0.0,
            tone_r: 0.0,
        }
    }
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
            left: EqChannelState::new(),
            right: EqChannelState::new(),
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
            let wet_l = process_eq_channel(l, left, &eq_params, sample_rate);
            let wet_r = process_eq_channel(r, right, &eq_params, sample_rate);
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

struct ModDelayParams {
    rate_hz: f32,
    depth_ms: f32,
    base_ms: f32,
    feedback: f32,
    mix: f32,
}

struct FilterLfoParams {
    kind: FilterLfoKind,
    rate_hz: f32,
    depth: f32,
    center_hz: f32,
    q: f32,
}

struct DuckParams {
    source: DuckSource,
    threshold: f32,
    amount: f32,
    attack_ms: f32,
    release_ms: f32,
}

struct CompressorParams {
    threshold_db: f32,
    ratio: f32,
    attack_ms: f32,
    release_ms: f32,
    makeup_db: f32,
    mix: f32,
}

struct EqParams {
    low_gain_db: f32,
    mid_gain_db: f32,
    mid_freq_hz: f32,
    mid_q: f32,
    high_gain_db: f32,
    mix: f32,
}

struct VinylParams {
    saturation: f32,
    crackle: f32,
    warp_depth: f32,
    mix: f32,
}

fn mix_sample(dry: f32, wet: f32, mix: f32) -> f32 {
    dry * (1.0 - mix) + wet * mix
}

fn process_saturator(input: f32, drive: f32, mix: f32) -> f32 {
    let y = (input * drive).tanh();
    mix_sample(input, y, mix)
}

fn process_distortion(input: f32, drive: f32, clip: f32, mix: f32) -> f32 {
    let y = (input * drive).clamp(-clip, clip) / clip.max(1.0e-6);
    mix_sample(input, y, mix)
}

fn process_mod_delay(
    state: &mut FxBusState,
    input: f32,
    params: ModDelayParams,
    sample_rate: u32,
) -> f32 {
    let FxBusState::ModDelay { buf, idx, phase } = state else {
        *state = FxBusState::ModDelay {
            buf: vec![0.0; ((sample_rate as f32) * 0.08) as usize],
            idx: 0,
            phase: 0.0,
        };
        return input;
    };
    let need =
        (((params.base_ms + params.depth_ms + 5.0) / 1000.0) * sample_rate as f32).ceil() as usize;
    if buf.len() < need.max(2) {
        buf.resize(need.max(2), 0.0);
    }
    let delay_ms =
        (params.base_ms + params.depth_ms * ((*phase).sin() + 1.0) * 0.5).clamp(0.1, 100.0);
    let delayed = read_delay(buf, *idx, delay_ms * sample_rate as f32 / 1000.0);
    buf[*idx] = (input + delayed * params.feedback).clamp(-2.0, 2.0);
    *idx = (*idx + 1) % buf.len();
    *phase = wrap_phase(*phase + 2.0 * PI * params.rate_hz / sample_rate as f32);
    (input * (1.0 - params.mix) + delayed * params.mix).clamp(-1.5, 1.5)
}

fn process_filter_lfo(
    state: &mut FxBusState,
    input: f32,
    params: FilterLfoParams,
    sample_rate: u32,
) -> f32 {
    let FxBusState::FilterLfo { filt, phase } = state else {
        *state = FxBusState::FilterLfo {
            filt: BiquadState::new(),
            phase: 0.0,
        };
        return input;
    };
    let sweep = ((*phase).sin() + 1.0) * 0.5;
    let semis = (sweep - 0.5) * 48.0 * params.depth;
    let cutoff = (params.center_hz * 2.0_f32.powf(semis / 12.0)).clamp(40.0, 18_000.0);
    *phase = wrap_phase(*phase + 2.0 * PI * params.rate_hz / sample_rate as f32);
    let mode = match params.kind {
        FilterLfoKind::Wah => FilterType::Bandpass,
        FilterLfoKind::FilterLfo => FilterType::Lowpass,
    };
    filt.process(input, mode, cutoff, params.q, sample_rate)
        .clamp(-1.5, 1.5)
}

fn process_glitch(
    state: &mut FxBusState,
    input: f32,
    chance: f32,
    slice_ms: f32,
    mix: f32,
    sample_rate: u32,
) -> f32 {
    let FxBusState::Glitch {
        buf,
        idx,
        read,
        remain,
        rng,
    } = state
    else {
        *state = FxBusState::Glitch {
            buf: vec![0.0; ((sample_rate as f32) * 0.25) as usize],
            idx: 0,
            read: 0,
            remain: 0,
            rng: 0x1234_abcd,
        };
        return input;
    };
    buf[*idx] = input;
    let block = (slice_ms * sample_rate as f32 / 1000.0).round().max(1.0) as usize;
    if *remain == 0 {
        *rng = rng.wrapping_mul(1664525).wrapping_add(1013904223);
        let roll = ((*rng >> 8) as f32) / ((u32::MAX >> 8) as f32);
        if roll < chance {
            *read = (*idx + buf.len()).saturating_sub(block.min(buf.len())) % buf.len();
            *remain = block;
        }
    }
    let wet = if *remain > 0 {
        let out = buf[*read];
        *read = (*read + 1) % buf.len();
        *remain -= 1;
        out
    } else {
        input
    };
    *idx = (*idx + 1) % buf.len();
    input * (1.0 - mix) + wet * mix
}

fn process_duck(
    state: &mut FxBusState,
    input: f32,
    params: DuckParams,
    slot_out: &[f32; INSTRUMENT_SLOT_COUNT],
    bus_in: &[f32],
    sample_rate: u32,
) -> f32 {
    let sc = match params.source {
        DuckSource::Instrument(idx) => slot_out.get(idx).copied().unwrap_or(0.0),
        DuckSource::Bus(idx) => bus_in.get(idx).copied().unwrap_or(0.0),
    };
    let FxBusState::Duck { env } = state else {
        *state = FxBusState::Duck { env: 0.0 };
        return input;
    };
    let x = sc.abs().min(1.0);
    let atk = (params.attack_ms / 1000.0 * sample_rate as f32).max(1.0);
    let rel = (params.release_ms / 1000.0 * sample_rate as f32).max(1.0);
    let coef = if x > *env { 1.0 / atk } else { 1.0 / rel };
    *env += (x - *env) * coef;
    let over = ((*env - params.threshold) / params.threshold.max(1.0e-6)).clamp(0.0, 1.0);
    input * (1.0 - params.amount * over)
}

fn process_bitcrusher(
    state: &mut FxBusState,
    input: f32,
    rate_div: u32,
    bits: u32,
    mix: f32,
) -> f32 {
    let FxBusState::Bitcrusher { hold, count, last } = state else {
        *state = FxBusState::Bitcrusher {
            hold: rate_div,
            count: 0,
            last: input,
        };
        return input;
    };
    *hold = rate_div;
    if *count == 0 {
        *last = input;
    }
    *count = (*count + 1) % (*hold).max(1);
    let levels = (1_u32 << bits.min(16)).max(2) as f32;
    let q = ((*last + 1.0) * 0.5 * (levels - 1.0)).round();
    let crushed = (q / (levels - 1.0)) * 2.0 - 1.0;
    input * (1.0 - mix) + crushed * mix
}

fn process_compressor(
    state: &mut FxBusState,
    input: f32,
    params: CompressorParams,
    sample_rate: u32,
) -> f32 {
    let FxBusState::Compressor { env } = state else {
        *state = FxBusState::Compressor { env: 0.0 };
        return input;
    };
    let gain = compressor_gain(env, input.abs().min(1.0), &params, sample_rate);
    mix_sample(input, input * gain, params.mix)
}

fn process_eq(state: &mut FxBusState, input: f32, params: EqParams, sample_rate: u32) -> f32 {
    let FxBusState::Eq { channel } = state else {
        *state = FxBusState::Eq {
            channel: EqChannelState::new(),
        };
        return input;
    };
    let wet = process_eq_channel(channel, input, &params, sample_rate);
    mix_sample(input, wet, params.mix)
}

fn compressor_gain(
    env: &mut f32,
    detector: f32,
    params: &CompressorParams,
    sample_rate: u32,
) -> f32 {
    let atk = (params.attack_ms / 1000.0 * sample_rate as f32).max(1.0);
    let rel = (params.release_ms / 1000.0 * sample_rate as f32).max(1.0);
    let coef = if detector > *env {
        1.0 / atk
    } else {
        1.0 / rel
    };
    *env += (detector - *env) * coef;

    let env_db = 20.0 * (*env).max(1.0e-10).log10();
    let reduction = if env_db > params.threshold_db {
        (env_db - params.threshold_db) * (1.0 - 1.0 / params.ratio.max(1.0))
    } else {
        0.0
    };
    10.0_f32.powf((-reduction + params.makeup_db) / 20.0)
}

fn process_eq_channel(
    state: &mut EqChannelState,
    input: f32,
    params: &EqParams,
    sample_rate: u32,
) -> f32 {
    let fs = sample_rate as f32;

    let y = input;
    let y = biquad_shelf(y, &mut state.low, params.low_gain_db, 250.0, 0.707, fs);
    let y = biquad_peak(
        y,
        &mut state.mid,
        params.mid_gain_db,
        params.mid_freq_hz,
        params.mid_q,
        fs,
    );
    biquad_shelf(y, &mut state.high, params.high_gain_db, 4000.0, 0.707, fs)
}

fn process_vinyl_stereo(
    state: &mut VinylState,
    left: f32,
    right: f32,
    params: VinylParams,
    sample_rate: u32,
) -> (f32, f32) {
    state.wow_phase = wrap_phase(state.wow_phase + 2.0 * PI * 0.33 / sample_rate as f32);
    state.flutter_phase = wrap_phase(state.flutter_phase + 2.0 * PI * 4.7 / sample_rate as f32);
    let warp = 1.0
        + ((state.wow_phase.sin() * 0.75 + state.flutter_phase.sin() * 0.25)
            * 0.08
            * params.warp_depth);

    let tone_mix = (0.08 + params.warp_depth * 0.24).clamp(0.0, 0.45);
    let drive = 1.0 + params.saturation * 4.0;
    let wet_l = process_saturator(left * warp, drive, 1.0);
    let wet_r = process_saturator(right * warp, drive, 1.0);
    state.tone_l += (wet_l - state.tone_l) * 0.08;
    state.tone_r += (wet_r - state.tone_r) * 0.08;
    let wet_l = wet_l * (1.0 - tone_mix) + state.tone_l * tone_mix;
    let wet_r = wet_r * (1.0 - tone_mix) + state.tone_r * tone_mix;

    state.rng = state.rng.wrapping_mul(1664525).wrapping_add(1013904223);
    let trigger = ((state.rng >> 8) as f32) / ((u32::MAX >> 8) as f32);
    if trigger < params.crackle * 0.0015 {
        state.rng = state.rng.wrapping_mul(1664525).wrapping_add(1013904223);
        let pan = ((state.rng >> 8) as f32) / ((u32::MAX >> 8) as f32);
        state.rng = state.rng.wrapping_mul(1664525).wrapping_add(1013904223);
        let amp = ((state.rng >> 8) as f32) / ((u32::MAX >> 8) as f32);
        state.crackle_pan = pan * 2.0 - 1.0;
        state.crackle_amp = params.crackle * (0.03 + amp * 0.18);
    }
    state.crackle_amp *= 0.94;
    state.rng = state.rng.wrapping_mul(1664525).wrapping_add(1013904223);
    let noise =
        ((((state.rng >> 8) as f32) / ((u32::MAX >> 8) as f32)) * 2.0 - 1.0) * state.crackle_amp;
    let crackle_l = noise * (1.0 - state.crackle_pan).clamp(0.0, 1.5);
    let crackle_r = noise * (1.0 + state.crackle_pan).clamp(0.0, 1.5);

    (
        mix_sample(left, wet_l + crackle_l, params.mix),
        mix_sample(right, wet_r + crackle_r, params.mix),
    )
}

fn biquad_shelf(x: f32, state: &mut BiquadState, gain_db: f32, fc: f32, q: f32, fs: f32) -> f32 {
    if gain_db.abs() < 0.05 {
        return x;
    }
    let a = 10.0_f32.powf(gain_db / 40.0);
    let sqrt_a = a.sqrt();
    let w0 = 2.0 * PI * fc / fs;
    let cos_w0 = w0.cos();
    let sin_w0 = w0.sin();
    let alpha = sin_w0 / (2.0 * q.max(0.001));
    let is_low = gain_db > 0.0;
    let (b0, b1, b2, a0, a1, a2) = if is_low {
        (
            a * ((a + 1.0) - (a - 1.0) * cos_w0 + 2.0 * sqrt_a * alpha),
            2.0 * a * ((a - 1.0) - (a + 1.0) * cos_w0),
            a * ((a + 1.0) - (a - 1.0) * cos_w0 - 2.0 * sqrt_a * alpha),
            (a + 1.0) + (a - 1.0) * cos_w0 + 2.0 * sqrt_a * alpha,
            -2.0 * ((a - 1.0) + (a + 1.0) * cos_w0),
            (a + 1.0) + (a - 1.0) * cos_w0 - 2.0 * sqrt_a * alpha,
        )
    } else {
        (
            a * ((a + 1.0) + (a - 1.0) * cos_w0 + 2.0 * sqrt_a * alpha),
            -2.0 * a * ((a - 1.0) + (a + 1.0) * cos_w0),
            a * ((a + 1.0) + (a - 1.0) * cos_w0 - 2.0 * sqrt_a * alpha),
            (a + 1.0) - (a - 1.0) * cos_w0 + 2.0 * sqrt_a * alpha,
            2.0 * ((a - 1.0) - (a + 1.0) * cos_w0),
            (a + 1.0) - (a - 1.0) * cos_w0 - 2.0 * sqrt_a * alpha,
        )
    };
    let nb0 = b0 / a0;
    let nb1 = b1 / a0;
    let nb2 = b2 / a0;
    let na1 = a1 / a0;
    let na2 = a2 / a0;
    let y = nb0 * x + nb1 * state.x1 + nb2 * state.x2 - na1 * state.y1 - na2 * state.y2;
    state.x2 = state.x1;
    state.x1 = x;
    state.y2 = state.y1;
    state.y1 = y;
    y
}

fn biquad_peak(x: f32, state: &mut BiquadState, gain_db: f32, fc: f32, q: f32, fs: f32) -> f32 {
    if gain_db.abs() < 0.05 {
        return x;
    }
    let a = 10.0_f32.powf(gain_db / 40.0);
    let w0 = 2.0 * PI * fc / fs;
    let cos_w0 = w0.cos();
    let sin_w0 = w0.sin();
    let alpha = sin_w0 / (2.0 * q.max(0.001));
    let b0 = 1.0 + alpha * a;
    let b1 = -2.0 * cos_w0;
    let b2 = 1.0 - alpha * a;
    let a0 = 1.0 + alpha / a;
    let a1 = -2.0 * cos_w0;
    let a2 = 1.0 - alpha / a;
    let nb0 = b0 / a0;
    let nb1 = b1 / a0;
    let nb2 = b2 / a0;
    let na1 = a1 / a0;
    let na2 = a2 / a0;
    let y = nb0 * x + nb1 * state.x1 + nb2 * state.x2 - na1 * state.y1 - na2 * state.y2;
    state.x2 = state.x1;
    state.x1 = x;
    state.y2 = state.y1;
    state.y1 = y;
    y
}

fn wrap_phase(mut phase: f32) -> f32 {
    while phase >= 2.0 * PI {
        phase -= 2.0 * PI;
    }
    while phase < 0.0 {
        phase += 2.0 * PI;
    }
    phase
}

fn read_delay(buf: &[f32], write_idx: usize, delay_samples: f32) -> f32 {
    let len = buf.len() as f32;
    let pos = (write_idx as f32 - delay_samples).rem_euclid(len);
    let i0 = pos.floor() as usize % buf.len();
    let i1 = (i0 + 1) % buf.len();
    let frac = pos - pos.floor();
    buf[i0] * (1.0 - frac) + buf[i1] * frac
}
