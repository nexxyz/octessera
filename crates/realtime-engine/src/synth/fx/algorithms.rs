use super::*;

mod comb_bank;
mod delay;
mod eq;
mod filter_lfo;
mod vinyl;

#[cfg(test)]
mod bitcrusher_tests;
#[cfg(test)]
mod compressor_tests;
#[cfg(test)]
mod reverb_tests;

pub use comb_bank::REVERB_COMB_LENGTHS;
pub(in crate::synth) use comb_bank::{
    comb_bank_frame, comb_bank_step, comb_buffers, ReverbInputGain,
};
pub(in crate::synth) use delay::{delay_line_state, process_delay, DELAY_MAX_MS};
pub(super) use delay::{process_mod_delay, DelayCache, ModDelayCache, ModDelayParams};
pub(super) use eq::{process_eq, process_eq_channel, EqChannelState, EqParams};
pub(super) use filter_lfo::{process_filter_lfo, FilterLfoCache, FilterLfoParams};
pub(in crate::synth) use vinyl::VinylState;
pub(super) use vinyl::{process_vinyl_mono_bus, process_vinyl_stereo, VinylParams};

/// Below this Drive the saturator's curve is a straight line to within 0.01 %.
const SATURATOR_CLEAN_DRIVE: f32 = 0.01;
/// The longest Glitch slice; the slot's buffer holds one.
const GLITCH_MAX_SLICE_MS: f32 = 500.0;
/// Each repeated glitch slice fades in and out over this long, or a quarter of the slice.
const GLITCH_FADE_MS: f32 = 2.0;
/// Duck Threshold 0 still waits for the source to reach -60 dBFS.
const DUCK_MIN_THRESHOLD: f32 = 0.001;
/// At 1 bit the crusher keeps only the sign; below -60 dBFS it is silent.
const ONE_BIT_GATE: f32 = 0.001;

pub(super) struct DuckParams {
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

/// `tanh(x * drive) / tanh(drive)`: full scale stays full scale, and a Drive near 0 passes the
/// sound clean.
pub(super) fn process_saturator(input: f32, drive: f32, mix: f32) -> f32 {
    let y = if drive < SATURATOR_CLEAN_DRIVE {
        input
    } else {
        (input * drive).tanh() / drive.tanh()
    };
    mix_sample(input, y, mix)
}

/// Drive is the gain above unity into a hard clip at Clip, so Drive 0 passes the sound clean up
/// to Clip.
pub(super) fn process_distortion(input: f32, drive: f32, clip: f32, mix: f32) -> f32 {
    let y = (input * (1.0 + drive)).clamp(-clip, clip);
    mix_sample(input, y, mix)
}

/// Chance is rolled once per slice. A hit replays the last slice the buffer heard, and the
/// buffer stops listening while it replays, so hits in a row keep repeating that slice; a miss
/// passes one slice through. Nothing repeats before a whole slice has been heard.
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
        slice,
        repeating,
        heard,
        rng,
    } = state
    else {
        *state = glitch_state(sample_rate);
        return input;
    };
    if *remain == 0 {
        *slice = ((slice_ms * sample_rate as f32 / 1000.0).round() as usize).clamp(1, buf.len());
        *rng = rng.wrapping_mul(1664525).wrapping_add(1013904223);
        let roll = ((*rng >> 8) as f32) / ((u32::MAX >> 8) as f32);
        *repeating = roll < chance && *heard >= *slice;
        *read = (*idx + buf.len() - *slice) % buf.len();
        *remain = *slice;
    }
    *remain -= 1;
    let wet = if *repeating {
        let played = *slice - *remain;
        let fade = (GLITCH_FADE_MS * sample_rate as f32 / 1000.0).min(*slice as f32 * 0.25);
        let edge = (played.min(*remain + 1) as f32 / fade.max(1.0)).min(1.0);
        let repeat = buf[*read];
        *read = (*read + 1) % buf.len();
        input + (repeat - input) * edge
    } else {
        buf[*idx] = input;
        *idx = (*idx + 1) % buf.len();
        *heard = heard.saturating_add(1);
        input
    };
    input * (1.0 - mix) + wet * mix
}

/// A zeroed buffer with every page written, so the audio thread never takes the page faults of
/// a lazily mapped allocation; the Pi locks memory only as it is first touched.
fn resident_zeros(len: usize) -> Vec<f32> {
    const PAGE_SAMPLES: usize = 4096 / std::mem::size_of::<f32>();
    let mut buf = vec![0.0; len];
    for sample in buf.iter_mut().step_by(PAGE_SAMPLES) {
        *sample = std::hint::black_box(0.0);
    }
    buf
}

pub(super) fn glitch_state(sample_rate: u32) -> FxBusState {
    FxBusState::Glitch {
        buf: resident_zeros((GLITCH_MAX_SLICE_MS * sample_rate as f32 / 1000.0).ceil() as usize),
        idx: 0,
        read: 0,
        remain: 0,
        slice: 1,
        repeating: false,
        heard: 0,
        rng: 0x1234_abcd,
    }
}

pub(super) fn process_duck_source(
    state: &mut FxBusState,
    input: f32,
    params: DuckParams,
    source: f32,
    sample_rate: u32,
) -> f32 {
    let FxBusState::Duck { env } = state else {
        *state = FxBusState::Duck { env: 0.0 };
        return input;
    };
    let x = source.abs().min(1.0);
    let atk = (params.attack_ms / 1000.0 * sample_rate as f32).max(1.0);
    let rel = (params.release_ms / 1000.0 * sample_rate as f32).max(1.0);
    let coef = if x > *env { 1.0 / atk } else { 1.0 / rel };
    *env += (x - *env) * coef;
    let threshold = params.threshold.max(DUCK_MIN_THRESHOLD);
    let over = (2.0 * *env / threshold - 1.0).clamp(0.0, 1.0);
    input * (1.0 - params.amount * over)
}

pub(in crate::synth) fn process_bitcrusher(
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
    crush_sample(count, last, input, rate_div, bits, mix)
}

/// One channel of the bitcrusher: holds a sample for `rate_div` samples and quantizes it.
pub(in crate::synth) fn crush_sample(
    count: &mut u32,
    last: &mut f32,
    input: f32,
    rate_div: u32,
    bits: u32,
    mix: f32,
) -> f32 {
    if *count == 0 {
        *last = input;
    }
    *count = (*count + 1) % rate_div.max(1);
    let crushed = if bits <= 1 {
        if last.abs() < ONE_BIT_GATE {
            0.0
        } else {
            0.5_f32.copysign(*last)
        }
    } else {
        let half_levels = (1_u32 << bits.min(16)) as f32 * 0.5;
        ((*last * half_levels).round() / half_levels).clamp(-1.0, 1.0)
    };
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
    let gain = compressor_gain(env, input.abs(), &params, sample_rate);
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
