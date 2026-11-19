use super::*;

const SAMPLE_RATE: u32 = 48_000;

fn params(low_gain_db: f32, high_gain_db: f32) -> EqParams {
    EqParams {
        low_gain_db,
        mid_gain_db: 0.0,
        mid_freq_hz: 1000.0,
        mid_q: 0.7,
        high_gain_db,
        mix: 1.0,
    }
}

fn sine(frequency_hz: f32, frame: usize) -> f32 {
    (2.0 * PI * frequency_hz * frame as f32 / SAMPLE_RATE as f32).sin() * 0.25
}

fn measured_gain_db(state: &mut EqChannelState, params: &EqParams, frequency_hz: f32) -> f32 {
    let settle = SAMPLE_RATE as usize / 2;
    let window = SAMPLE_RATE as usize / 5;
    let (mut input_energy, mut output_energy) = (0.0_f64, 0.0_f64);
    for frame in 0..settle + window {
        let input = sine(frequency_hz, frame);
        let output = process_eq_channel(state, input, params, SAMPLE_RATE);
        if frame >= settle {
            input_energy += f64::from(input * input);
            output_energy += f64::from(output * output);
        }
    }
    (10.0 * (output_energy / input_energy).log10()) as f32
}

fn assert_gain(params: &EqParams, frequency_hz: f32, expected_db: f32, tolerance_db: f32) {
    let gain = measured_gain_db(&mut EqChannelState::new(), params, frequency_hz);
    assert!(
        (gain - expected_db).abs() <= tolerance_db,
        "low {} dB / high {} dB at {frequency_hz} Hz: measured {gain} dB, expected {expected_db} dB",
        params.low_gain_db,
        params.high_gain_db,
    );
}

#[test]
fn low_band_shelves_the_lows_for_boost_and_cut() {
    for gain in [6.0, -6.0] {
        let params = params(gain, 0.0);
        assert_gain(&params, 50.0, gain, 0.5);
        assert_gain(&params, 10_000.0, 0.0, 0.25);
    }
}

#[test]
fn high_band_shelves_the_highs_for_boost_and_cut() {
    for gain in [6.0, -6.0] {
        let params = params(0.0, gain);
        assert_gain(&params, 10_000.0, gain, 1.0);
        assert_gain(&params, 50.0, 0.0, 0.25);
    }
}

fn mid_params(mid_gain_db: f32, mid_freq_hz: f32, mix: f32) -> EqParams {
    EqParams {
        low_gain_db: 0.0,
        mid_gain_db,
        mid_freq_hz,
        mid_q: 1.0,
        high_gain_db: 0.0,
        mix,
    }
}

#[test]
fn mid_band_reaches_its_gain_at_the_band_center_and_leaves_the_edges_flat() {
    for (gain, center) in [(9.0, 1000.0), (-9.0, 1000.0), (6.0, 300.0), (-12.0, 3000.0)] {
        let params = mid_params(gain, center, 1.0);
        assert_gain(&params, center, gain, 0.3);
        assert_gain(&params, center / 16.0, 0.0, 0.5);
        assert_gain(&params, (center * 16.0).min(20_000.0), 0.0, 0.5);
    }
}

#[test]
fn bus_eq_mix_blends_dry_and_boosted_signal_at_the_band_center() {
    let mut state = FxBusState::Eq {
        channel: EqChannelState::new(),
    };
    let settle = SAMPLE_RATE as usize / 2;
    let (mut input_energy, mut output_energy) = (0.0_f64, 0.0_f64);
    for frame in 0..settle * 2 {
        let input = sine(1000.0, frame);
        let output = process_eq(
            &mut state,
            input,
            mid_params(12.0, 1000.0, 0.5),
            SAMPLE_RATE,
        );
        if frame >= settle {
            input_energy += f64::from(input * input);
            output_energy += f64::from(output * output);
        }
    }
    let measured = 10.0 * (output_energy / input_energy).log10();
    let expected = 20.0 * (0.5 + 0.5 * 10.0_f64.powf(12.0 / 20.0)).log10();
    assert!(
        (measured - expected).abs() < 0.3,
        "measured {measured} dB, expected {expected} dB"
    );
}

#[test]
fn changed_band_gain_takes_effect_on_the_running_filter() {
    let mut state = EqChannelState::new();
    let boost = measured_gain_db(&mut state, &params(6.0, 0.0), 50.0);
    let cut = measured_gain_db(&mut state, &params(-6.0, 0.0), 50.0);
    assert!((boost - 6.0).abs() <= 0.5, "boost measured {boost} dB");
    assert!((cut + 6.0).abs() <= 0.5, "cut measured {cut} dB");
}

#[test]
fn re_engaged_band_starts_without_stale_history() {
    let mut state = EqChannelState::new();
    let cut = params(-12.0, -12.0);
    for frame in 0..4800 {
        process_eq_channel(&mut state, sine(40.0, frame) * 3.0, &cut, SAMPLE_RATE);
    }
    let bypassed = params(0.0, 0.0);
    for frame in 0..480 {
        process_eq_channel(&mut state, sine(440.0, frame), &bypassed, SAMPLE_RATE);
    }
    let nearly_flat = params(0.1, 0.1);
    for frame in 480..960 {
        let input = sine(440.0, frame);
        let output = process_eq_channel(&mut state, input, &nearly_flat, SAMPLE_RATE);
        assert!(
            (output - input).abs() < 0.01,
            "frame {frame}: re-engaged output {output} jumps away from input {input}"
        );
    }
}
