use super::*;

const SAMPLE_RATE: u32 = 48_000;

fn state(params: &FilterLfoParams) -> FxBusState {
    FxBusState::FilterLfo {
        filt: BiquadState::new(),
        phase: 0.0,
        cache: FilterLfoCache::new(params.kind, params.rate_hz, SAMPLE_RATE),
    }
}

fn params(
    kind: FilterLfoKind,
    rate_hz: f32,
    depth: f32,
    center_hz: f32,
    q: f32,
) -> FilterLfoParams {
    FilterLfoParams {
        kind,
        rate_hz,
        depth,
        center_hz,
        q,
    }
}

/// The level of a `tone_hz` sine through the effect over `window`, in dB against the input.
fn gain_db(params: &FilterLfoParams, tone_hz: f32, window: std::ops::Range<usize>) -> f32 {
    let mut state = state(params);
    let (mut input_energy, mut output_energy) = (0.0_f64, 0.0_f64);
    for frame in 0..window.end {
        let input = (2.0 * PI * tone_hz * frame as f32 / SAMPLE_RATE as f32).sin() * 0.25;
        let copy = FilterLfoParams { ..*params };
        let output = process_filter_lfo(&mut state, input, copy, SAMPLE_RATE);
        if window.contains(&frame) {
            input_energy += f64::from(input * input);
            output_energy += f64::from(output * output);
        }
    }
    (10.0 * (output_energy / input_energy).log10()) as f32
}

fn assert_near(measured: f32, expected: f32, tolerance: f32, what: &str) {
    assert!(
        (measured - expected).abs() <= tolerance,
        "{what}: {measured} dB, expected {expected} dB"
    );
}

const SETTLED: std::ops::Range<usize> = 24_000..33_600;

#[test]
fn filter_lfo_without_depth_is_a_lowpass_at_the_center() {
    let params = params(FilterLfoKind::FilterLfo, 0.5, 0.0, 1000.0, 0.707);
    assert_near(gain_db(&params, 100.0, SETTLED), 0.0, 0.2, "100 Hz");
    assert_near(gain_db(&params, 1000.0, SETTLED), -3.0, 0.3, "at cutoff");
    assert!(gain_db(&params, 8000.0, SETTLED) < -30.0);
}

#[test]
fn wah_without_depth_is_a_bandpass_peaking_at_the_center() {
    let params = params(FilterLfoKind::Wah, 1.2, 0.0, 1000.0, 2.0);
    assert_near(gain_db(&params, 1000.0, SETTLED), 0.0, 0.3, "at center");
    assert!(gain_db(&params, 250.0, SETTLED) < -12.0);
    assert!(gain_db(&params, 4000.0, SETTLED) < -12.0);
}

#[test]
fn full_depth_sweeps_the_cutoff_two_octaves_either_side_of_the_center() {
    let period = SAMPLE_RATE as usize * 4;
    let params = params(FilterLfoKind::FilterLfo, 0.25, 1.0, 1000.0, 0.707);
    let top = period / 4;
    let bottom = 3 * period / 4;
    let around = |frame: usize| frame - 1_000..frame + 1_000;
    assert_near(
        gain_db(&params, 4000.0, around(top)),
        -3.0,
        0.5,
        "4 kHz at top",
    );
    assert_near(
        gain_db(&params, 250.0, around(bottom)),
        -3.0,
        0.5,
        "250 Hz at bottom",
    );
    assert!(gain_db(&params, 4000.0, around(bottom)) < -40.0);
}
