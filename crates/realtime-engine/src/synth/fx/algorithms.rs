use super::*;

mod delay;
mod eq;
mod filter_lfo;

pub(super) use delay::{
    process_delay, process_mod_delay, DelayCache, ModDelayCache, ModDelayParams,
};
pub(super) use eq::{process_eq, process_eq_channel, EqChannelState, EqParams};
pub(super) use filter_lfo::{process_filter_lfo, FilterLfoCache, FilterLfoParams};

#[derive(Clone, Debug)]
pub(in crate::synth) struct VinylState {
    wow_phase: f32,
    flutter_phase: f32,
    crackle_amp: f32,
    crackle_pan: f32,
    rng: u32,
    tone_l: f32,
    tone_r: f32,
}

impl VinylState {
    pub(in crate::synth) fn new() -> Self {
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

#[derive(Clone, Copy)]
pub(super) struct VinylParams {
    pub(super) saturation: f32,
    pub(super) crackle: f32,
    pub(super) warp_depth: f32,
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

pub(super) fn process_vinyl_mono_bus(
    state: &mut VinylState,
    input: f32,
    params: VinylParams,
    sample_rate: u32,
) -> f32 {
    state.wow_phase = wrap_phase(state.wow_phase + 2.0 * PI * 0.33 / sample_rate as f32);
    state.flutter_phase = wrap_phase(state.flutter_phase + 2.0 * PI * 4.7 / sample_rate as f32);
    let warp = 1.0
        + ((state.wow_phase.sin() * 0.75 + state.flutter_phase.sin() * 0.25)
            * 0.08
            * params.warp_depth);

    let tone_mix = (0.08 + params.warp_depth * 0.24).clamp(0.0, 0.45);
    let drive = 1.0 + params.saturation * 4.0;
    let wet_l = process_saturator(input * warp, drive, 1.0);
    state.tone_l += (wet_l - state.tone_l) * 0.08;
    state.tone_r += (wet_l - state.tone_r) * 0.08;
    let wet_l = wet_l * (1.0 - tone_mix) + state.tone_l * tone_mix;

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

    mix_sample(input, wet_l + crackle_l, params.mix)
}

pub(super) fn process_vinyl_stereo(
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

pub(super) fn wrap_phase(mut phase: f32) -> f32 {
    while phase >= 2.0 * PI {
        phase -= 2.0 * PI;
    }
    while phase < 0.0 {
        phase += 2.0 * PI;
    }
    phase
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mono_bus_vinyl_matches_duplicated_input_stereo_left_path() {
        let mut mono = VinylState::new();
        let mut stereo = VinylState::new();
        let sample_rate = 44_100;

        for frame in 0..4096 {
            let input =
                ((frame as f32) * 0.017).sin() * 0.35 + ((frame as f32) * 0.031).cos() * 0.12;
            let params = VinylParams {
                saturation: 0.15,
                crackle: 0.8,
                warp_depth: 0.5,
                mix: 1.0,
            };
            let mono_out = process_vinyl_mono_bus(&mut mono, input, params, sample_rate);
            let (stereo_left, _) =
                process_vinyl_stereo(&mut stereo, input, input, params, sample_rate);
            assert_eq!(mono_out.to_bits(), stereo_left.to_bits(), "frame {frame}");
            assert_vinyl_state_eq(&mono, &stereo, frame);
        }
    }

    fn assert_vinyl_state_eq(left: &VinylState, right: &VinylState, frame: usize) {
        assert_eq!(
            left.wow_phase.to_bits(),
            right.wow_phase.to_bits(),
            "wow {frame}"
        );
        assert_eq!(
            left.flutter_phase.to_bits(),
            right.flutter_phase.to_bits(),
            "flutter {frame}"
        );
        assert_eq!(
            left.crackle_amp.to_bits(),
            right.crackle_amp.to_bits(),
            "amp {frame}"
        );
        assert_eq!(
            left.crackle_pan.to_bits(),
            right.crackle_pan.to_bits(),
            "pan {frame}"
        );
        assert_eq!(left.rng, right.rng, "rng {frame}");
        assert_eq!(
            left.tone_l.to_bits(),
            right.tone_l.to_bits(),
            "tone_l {frame}"
        );
        assert_eq!(
            left.tone_r.to_bits(),
            right.tone_r.to_bits(),
            "tone_r {frame}"
        );
    }
}
