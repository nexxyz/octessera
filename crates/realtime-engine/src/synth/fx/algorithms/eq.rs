use super::mix_sample;
use crate::synth::fx::{BiquadState, FxBusState};
use std::f32::consts::PI;

#[derive(Clone, Debug)]
pub(in crate::synth) struct EqChannelState {
    low: EqBandState,
    mid: EqBandState,
    high: EqBandState,
}

impl EqChannelState {
    pub(in crate::synth) fn new() -> Self {
        Self {
            low: EqBandState::new(),
            mid: EqBandState::new(),
            high: EqBandState::new(),
        }
    }
}

#[derive(Clone, Debug)]
struct EqBandState {
    filter: BiquadState,
    coeffs: Option<CachedBiquad>,
}

impl EqBandState {
    fn new() -> Self {
        Self {
            filter: BiquadState::new(),
            coeffs: None,
        }
    }
}

#[derive(Clone, Debug)]
struct CachedBiquad {
    key: CoeffKey,
    coeffs: BiquadCoeffs,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CoeffKey {
    kind: EqFilterKind,
    gain_db: u32,
    fc: u32,
    q: u32,
    sample_rate: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EqFilterKind {
    Shelf,
    Peak,
}

#[derive(Clone, Copy, Debug)]
struct BiquadCoeffs {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
}

pub(in crate::synth) struct EqParams {
    pub(in crate::synth) low_gain_db: f32,
    pub(in crate::synth) mid_gain_db: f32,
    pub(in crate::synth) mid_freq_hz: f32,
    pub(in crate::synth) mid_q: f32,
    pub(in crate::synth) high_gain_db: f32,
    pub(in crate::synth) mix: f32,
}

pub(in crate::synth) fn process_eq(
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

pub(in crate::synth) fn process_eq_channel(
    state: &mut EqChannelState,
    input: f32,
    params: &EqParams,
    sample_rate: u32,
) -> f32 {
    let fs = sample_rate as f32;

    let y = input;
    let y = biquad_shelf(
        y,
        &mut state.low,
        params.low_gain_db,
        250.0,
        0.707,
        fs,
        sample_rate,
    );
    let y = biquad_peak(
        y,
        &mut state.mid,
        params.mid_gain_db,
        params.mid_freq_hz,
        params.mid_q,
        fs,
        sample_rate,
    );
    biquad_shelf(
        y,
        &mut state.high,
        params.high_gain_db,
        4000.0,
        0.707,
        fs,
        sample_rate,
    )
}

fn biquad_shelf(
    x: f32,
    state: &mut EqBandState,
    gain_db: f32,
    fc: f32,
    q: f32,
    fs: f32,
    sample_rate: u32,
) -> f32 {
    if gain_db.abs() < 0.05 {
        return x;
    }
    let key = coeff_key(EqFilterKind::Shelf, gain_db, fc, q, sample_rate);
    let coeffs = cached_coeffs(state, key, || shelf_coeffs(gain_db, fc, q, fs));
    apply_biquad(x, &mut state.filter, coeffs)
}

fn biquad_peak(
    x: f32,
    state: &mut EqBandState,
    gain_db: f32,
    fc: f32,
    q: f32,
    fs: f32,
    sample_rate: u32,
) -> f32 {
    if gain_db.abs() < 0.05 {
        return x;
    }
    let key = coeff_key(EqFilterKind::Peak, gain_db, fc, q, sample_rate);
    let coeffs = cached_coeffs(state, key, || peak_coeffs(gain_db, fc, q, fs));
    apply_biquad(x, &mut state.filter, coeffs)
}

fn coeff_key(kind: EqFilterKind, gain_db: f32, fc: f32, q: f32, sample_rate: u32) -> CoeffKey {
    CoeffKey {
        kind,
        gain_db: gain_db.to_bits(),
        fc: fc.to_bits(),
        q: q.to_bits(),
        sample_rate,
    }
}

fn cached_coeffs(
    state: &mut EqBandState,
    key: CoeffKey,
    build: impl FnOnce() -> BiquadCoeffs,
) -> BiquadCoeffs {
    if let Some(cached) = &state.coeffs {
        if cached.key == key {
            return cached.coeffs;
        }
    }

    let coeffs = build();
    state.coeffs = Some(CachedBiquad { key, coeffs });
    coeffs
}

fn shelf_coeffs(gain_db: f32, fc: f32, q: f32, fs: f32) -> BiquadCoeffs {
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
    normalize_coeffs(b0, b1, b2, a0, a1, a2)
}

fn peak_coeffs(gain_db: f32, fc: f32, q: f32, fs: f32) -> BiquadCoeffs {
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
    normalize_coeffs(b0, b1, b2, a0, a1, a2)
}

fn normalize_coeffs(b0: f32, b1: f32, b2: f32, a0: f32, a1: f32, a2: f32) -> BiquadCoeffs {
    BiquadCoeffs {
        b0: b0 / a0,
        b1: b1 / a0,
        b2: b2 / a0,
        a1: a1 / a0,
        a2: a2 / a0,
    }
}

fn apply_biquad(x: f32, state: &mut BiquadState, coeffs: BiquadCoeffs) -> f32 {
    let y = coeffs.b0 * x + coeffs.b1 * state.x1 + coeffs.b2 * state.x2
        - coeffs.a1 * state.y1
        - coeffs.a2 * state.y2;
    state.x2 = state.x1;
    state.x1 = x;
    state.y2 = state.y1;
    state.y1 = y;
    y
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cached_eq_matches_reference_with_static_params() {
        let params = EqParams {
            low_gain_db: 3.0,
            mid_gain_db: -2.5,
            mid_freq_hz: 900.0,
            mid_q: 0.8,
            high_gain_db: 1.75,
            mix: 1.0,
        };
        assert_matches_reference(&[params], 2048);
    }

    #[test]
    fn cached_eq_matches_reference_across_param_changes() {
        let params = [
            EqParams {
                low_gain_db: 3.0,
                mid_gain_db: -2.5,
                mid_freq_hz: 900.0,
                mid_q: 0.8,
                high_gain_db: 1.75,
                mix: 1.0,
            },
            EqParams {
                low_gain_db: -4.0,
                mid_gain_db: 2.25,
                mid_freq_hz: 1500.0,
                mid_q: 1.2,
                high_gain_db: -1.5,
                mix: 1.0,
            },
        ];
        assert_matches_reference(&params, 4096);
    }

    fn assert_matches_reference(params: &[EqParams], frames: usize) {
        let mut cached = EqChannelState::new();
        let mut reference = ReferenceEqChannelState::new();
        for frame in 0..frames {
            let input = ((frame as f32) * 0.013).sin() * 0.4 + ((frame as f32) * 0.031).cos() * 0.2;
            let params = &params[(frame * params.len()) / frames];
            let actual = process_eq_channel(&mut cached, input, params, 44_100);
            let expected = reference_process_eq_channel(&mut reference, input, params, 44_100);
            assert!(
                (actual - expected).abs() < 1.0e-6,
                "frame {frame}: {actual} != {expected}"
            );
        }
    }

    struct ReferenceEqChannelState {
        low: BiquadState,
        mid: BiquadState,
        high: BiquadState,
    }

    impl ReferenceEqChannelState {
        fn new() -> Self {
            Self {
                low: BiquadState::new(),
                mid: BiquadState::new(),
                high: BiquadState::new(),
            }
        }
    }

    fn reference_process_eq_channel(
        state: &mut ReferenceEqChannelState,
        input: f32,
        params: &EqParams,
        sample_rate: u32,
    ) -> f32 {
        let fs = sample_rate as f32;
        let y = reference_biquad_shelf(input, &mut state.low, params.low_gain_db, 250.0, 0.707, fs);
        let y = reference_biquad_peak(
            y,
            &mut state.mid,
            params.mid_gain_db,
            params.mid_freq_hz,
            params.mid_q,
            fs,
        );
        reference_biquad_shelf(y, &mut state.high, params.high_gain_db, 4000.0, 0.707, fs)
    }

    fn reference_biquad_shelf(
        x: f32,
        state: &mut BiquadState,
        gain_db: f32,
        fc: f32,
        q: f32,
        fs: f32,
    ) -> f32 {
        if gain_db.abs() < 0.05 {
            return x;
        }
        apply_biquad(x, state, shelf_coeffs(gain_db, fc, q, fs))
    }

    fn reference_biquad_peak(
        x: f32,
        state: &mut BiquadState,
        gain_db: f32,
        fc: f32,
        q: f32,
        fs: f32,
    ) -> f32 {
        if gain_db.abs() < 0.05 {
            return x;
        }
        apply_biquad(x, state, peak_coeffs(gain_db, fc, q, fs))
    }
}
