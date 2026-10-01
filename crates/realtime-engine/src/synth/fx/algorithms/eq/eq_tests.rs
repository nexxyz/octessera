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
