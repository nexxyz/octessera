use super::*;

#[derive(Clone, Debug)]
pub(in crate::synth) struct EqChannelState {
    low: BiquadState,
    mid: BiquadState,
    high: BiquadState,
}

impl EqChannelState {
    pub(in crate::synth) fn new() -> Self {
        Self {
            low: BiquadState::new(),
            mid: BiquadState::new(),
            high: BiquadState::new(),
        }
    }
}

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

pub(super) struct ModDelayParams {
    pub(super) rate_hz: f32,
    pub(super) depth_ms: f32,
    pub(super) base_ms: f32,
    pub(super) feedback: f32,
    pub(super) mix: f32,
}

pub(super) struct FilterLfoParams {
    pub(super) kind: FilterLfoKind,
    pub(super) rate_hz: f32,
    pub(super) depth: f32,
    pub(super) center_hz: f32,
    pub(super) q: f32,
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

pub(super) struct EqParams {
    pub(super) low_gain_db: f32,
    pub(super) mid_gain_db: f32,
    pub(super) mid_freq_hz: f32,
    pub(super) mid_q: f32,
    pub(super) high_gain_db: f32,
    pub(super) mix: f32,
}

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

pub(super) fn process_mod_delay(
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

pub(super) fn process_filter_lfo(
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

pub(super) fn process_eq(
    state: &mut FxBusState,
    input: f32,
    params: EqParams,
    sample_rate: u32,
) -> f32 {
    let FxBusState::Eq { channel } = state else {
        *state = FxBusState::Eq {
            channel: EqChannelState::new(),
        };
        return input;
    };
    let wet = process_eq_channel(channel, input, &params, sample_rate);
    mix_sample(input, wet, params.mix)
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

pub(super) fn process_eq_channel(
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

pub(super) fn wrap_phase(mut phase: f32) -> f32 {
    while phase >= 2.0 * PI {
        phase -= 2.0 * PI;
    }
    while phase < 0.0 {
        phase += 2.0 * PI;
    }
    phase
}

pub(super) fn read_delay(buf: &[f32], write_idx: usize, delay_samples: f32) -> f32 {
    let len = buf.len() as f32;
    let pos = (write_idx as f32 - delay_samples).rem_euclid(len);
    let i0 = pos.floor() as usize % buf.len();
    let i1 = (i0 + 1) % buf.len();
    let frac = pos - pos.floor();
    buf[i0] * (1.0 - frac) + buf[i1] * frac
}
