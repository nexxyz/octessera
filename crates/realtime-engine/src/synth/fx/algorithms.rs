use super::*;

mod delay;
mod eq;
mod filter_lfo;
mod vinyl;

pub(super) use delay::{
    process_delay, process_mod_delay, DelayCache, ModDelayCache, ModDelayParams,
};
pub(super) use eq::{process_eq, process_eq_channel, EqChannelState, EqParams};
pub(super) use filter_lfo::{process_filter_lfo, FilterLfoCache, FilterLfoParams};
pub(in crate::synth) use vinyl::VinylState;
pub(super) use vinyl::{process_vinyl_mono_bus, process_vinyl_stereo, VinylParams};

pub(super) struct DuckParams {
    pub(super) source: DuckSource,
    pub(super) threshold: f32,
    pub(super) amount: f32,
    pub(super) attack_ms: f32,
    pub(super) release_ms: f32,
}

pub(super) struct CompressorParams {
    pub(super) threshold_db: f32,
    pub(super) ratio: f32,
    pub(super) attack_ms: f32,
    pub(super) release_ms: f32,
    pub(super) makeup_db: f32,
    pub(super) mix: f32,
}

pub(super) fn mix_sample(dry: f32, wet: f32, mix: f32) -> f32 {
    dry * (1.0 - mix) + wet * mix
}

pub(super) fn process_saturator(input: f32, drive: f32, mix: f32) -> f32 {
    let y = (input * drive).tanh();
    mix_sample(input, y, mix)
}

pub(super) fn process_distortion(input: f32, drive: f32, clip: f32, mix: f32) -> f32 {
    let y = (input * drive).clamp(-clip, clip) / clip.max(1.0e-6);
    mix_sample(input, y, mix)
}

pub(super) fn process_glitch(
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

pub(super) fn process_duck(
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

pub(super) fn process_bitcrusher(
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

pub(super) fn process_compressor(
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

pub(super) fn compressor_gain(
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

pub(super) fn wrap_phase(mut phase: f32) -> f32 {
    while phase >= 2.0 * PI {
        phase -= 2.0 * PI;
    }
    while phase < 0.0 {
        phase += 2.0 * PI;
    }
    phase
}
