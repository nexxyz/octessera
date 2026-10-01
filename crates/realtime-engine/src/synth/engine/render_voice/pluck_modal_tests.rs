use super::pluck_pitch_tests::{
    pitch_candidate_band_window, render_dry_pluck, PluckPitchReference,
    DRY_MEASUREMENT_WINDOWS_SECONDS, MIN_PEAK_CONFIDENCE, MIN_Q_LITERAL,
};
use std::f64::consts::TAU;

fn loop_phase(
    frequency_hz: f64,
    sample_rate: u32,
    pluck: &super::pluck_pitch_tests::RenderedPluck,
) -> f64 {
    let w = TAU * frequency_hz / f64::from(sample_rate);
    let n = pluck.delay as f64;
    let mu = f64::from(pluck.fraction);
    let q = f64::from(pluck.loss) * (1.0 - f64::from(pluck.brightness));
    let a = f64::from(pluck.coefficient);
    let linear = (mu * w.sin()).atan2(1.0 - mu + mu * w.cos());
    let loss = (q * w.sin()).atan2(1.0 - q * w.cos());
    let allpass = if a == 0.0 {
        0.0
    } else {
        ((1.0 - a * a) * w.sin()).atan2((1.0 + a * a) * w.cos() + 2.0 * a)
    };
    n * w + linear + loss + allpass
}

fn ordered_phase_roots(
    sample_rate: u32,
    pluck: &super::pluck_pitch_tests::RenderedPluck,
) -> [f64; 7] {
    let mut roots = [0.0; 7];
    let mut lower = 0.0;
    for mode in 1..=7 {
        let target_phase = TAU * mode as f64;
        let mut upper = f64::from(sample_rate) * 0.5;
        assert!(
            loop_phase(upper, sample_rate, pluck) >= target_phase,
            "{sample_rate} Hz mode {mode}: Nyquist phase {} does not reach {target_phase}; N {} mu {} a {}",
            loop_phase(upper, sample_rate, pluck),
            pluck.delay,
            pluck.fraction,
            pluck.coefficient
        );
        for _ in 0..64 {
            let middle = (lower + upper) * 0.5;
            if loop_phase(middle, sample_rate, pluck) < target_phase {
                lower = middle;
            } else {
                upper = middle;
            }
        }
        roots[mode - 1] = (lower + upper) * 0.5;
        assert!(
            roots[mode - 1].is_finite()
                && roots[mode - 1] > 0.0
                && roots[mode - 1] < f64::from(sample_rate) * 0.5
                && (mode == 1 || roots[mode - 1] > roots[mode - 2])
        );
        assert!(
            (loop_phase(roots[mode - 1], sample_rate, pluck) - target_phase).abs() < 1.0e-7,
            "{sample_rate} Hz mode {mode} root {} does not close phase: {} vs {target_phase}",
            roots[mode - 1],
            loop_phase(roots[mode - 1], sample_rate, pluck)
        );
        lower = roots[mode - 1];
    }
    roots
}

fn mode_band(roots: &[f64; 7], mode: usize) -> (f64, f64) {
    (
        0.5 * (roots[mode - 2] + roots[mode - 1]),
        0.5 * (roots[mode - 1] + roots[mode]),
    )
}

fn assert_ordered_bands(roots: &[f64; 7], context: &str) {
    for mode in 2..=6 {
        let (low, high) = mode_band(roots, mode);
        let root = roots[mode - 1];
        assert!(
            low < root && root < high,
            "{context} mode {mode}: root {root} is outside midpoint band ({low}, {high})"
        );
        if mode < 6 {
            assert!(
                high <= mode_band(roots, mode + 1).0,
                "{context}: mode {mode} band overlaps mode {}",
                mode + 1
            );
        }
    }
}

fn confident_mode_peak(
    samples: &[f32],
    sample_rate: u32,
    band: (f64, f64),
    window_start: f64,
    context: &str,
) -> super::pluck_pitch_tests::SpectralPitch {
    let peak = pitch_candidate_band_window(samples, sample_rate, band.0, band.1, window_start)
        .unwrap_or_else(|| panic!("{context}: no interior peak in phase band {band:?}"));
    assert!(
        peak.confidence >= MIN_PEAK_CONFIDENCE && peak.normalized_strength >= MIN_Q_LITERAL,
        "{context}: unresolved phase-band peak {:.3} Hz raw {} Q {} confidence {} band {band:?}",
        peak.frequency_hz,
        peak.strength,
        peak.normalized_strength,
        peak.confidence
    );
    peak
}

#[test]
fn midi60_dry_stiffness_modes_follow_ordered_phase_roots_at_both_rates() {
    for sample_rate in [44_100_u32, 48_000] {
        for window_start in DRY_MEASUREMENT_WINDOWS_SECONDS {
            let unshaped = render_dry_pluck(
                60,
                120,
                sample_rate,
                PluckPitchReference::CompensatedUnshaped,
            );
            let stiff =
                render_dry_pluck(60, 120, sample_rate, PluckPitchReference::ProductionStiff);
            let unshaped_roots = ordered_phase_roots(sample_rate, &unshaped);
            let stiff_roots = ordered_phase_roots(sample_rate, &stiff);
            assert_ordered_bands(
                &unshaped_roots,
                &format!("{sample_rate} Hz MIDI60 unshaped"),
            );
            assert_ordered_bands(&stiff_roots, &format!("{sample_rate} Hz MIDI60 stiff"));
            let target_f0 = 440.0 * 2.0_f64.powf((60.0 - 69.0) / 12.0);
            let f0_band = (target_f0 * 0.75, target_f0 * 1.25);
            let unshaped_f0 = confident_mode_peak(
                &unshaped.samples,
                sample_rate,
                f0_band,
                window_start,
                &format!(
                    "{sample_rate} Hz MIDI60 unshaped f0 N {} mu {} a {}",
                    unshaped.delay, unshaped.fraction, unshaped.coefficient
                ),
            );
            let stiff_f0 = confident_mode_peak(
                &stiff.samples,
                sample_rate,
                f0_band,
                window_start,
                &format!(
                    "{sample_rate} Hz MIDI60 stiff f0 N {} mu {} a {}",
                    stiff.delay, stiff.fraction, stiff.coefficient
                ),
            );
            for mode in 2..=6 {
                let band_unshaped = mode_band(&unshaped_roots, mode);
                let band_stiff = mode_band(&stiff_roots, mode);
                let context = format!("{sample_rate} Hz MIDI60 mode {mode} window {window_start}");
                let peak_unshaped = confident_mode_peak(
                    &unshaped.samples,
                    sample_rate,
                    band_unshaped,
                    window_start,
                    &format!(
                        "{context} unshaped N {} mu {} a {}",
                        unshaped.delay, unshaped.fraction, unshaped.coefficient
                    ),
                );
                let peak_stiff = confident_mode_peak(
                    &stiff.samples,
                    sample_rate,
                    band_stiff,
                    window_start,
                    &format!(
                        "{context} stiff N {} mu {} a {}",
                        stiff.delay, stiff.fraction, stiff.coefficient
                    ),
                );
                let unshaped_ratio = peak_unshaped.frequency_hz / unshaped_f0.frequency_hz;
                let stiff_ratio = peak_stiff.frequency_hz / stiff_f0.frequency_hz;
                assert!(
                    stiff_ratio > unshaped_ratio,
                    "{context}: mode ratios did not stretch upward: unshaped {unshaped_ratio} ({} Hz, Q {}, confidence {}, root {} Hz), stiff {stiff_ratio} ({} Hz, Q {}, confidence {}, root {} Hz)",
                    peak_unshaped.frequency_hz,
                    peak_unshaped.normalized_strength,
                    peak_unshaped.confidence,
                    unshaped_roots[mode - 1],
                    peak_stiff.frequency_hz,
                    peak_stiff.normalized_strength,
                    peak_stiff.confidence,
                    stiff_roots[mode - 1]
                );
                println!("{context}: unshaped root/peak {}/{} Hz ratio {unshaped_ratio}, stiff root/peak {}/{} Hz ratio {stiff_ratio}", unshaped_roots[mode - 1], peak_unshaped.frequency_hz, stiff_roots[mode - 1], peak_stiff.frequency_hz);
            }
        }
    }
}

#[test]
fn midi84_stored_loop_phase_crossings_remain_ordered_at_both_rates() {
    for sample_rate in [44_100_u32, 48_000] {
        let unshaped = render_dry_pluck(
            84,
            120,
            sample_rate,
            PluckPitchReference::CompensatedUnshaped,
        );
        let stiff = render_dry_pluck(84, 120, sample_rate, PluckPitchReference::ProductionStiff);
        let unshaped_roots = ordered_phase_roots(sample_rate, &unshaped);
        let stiff_roots = ordered_phase_roots(sample_rate, &stiff);
        assert_ordered_bands(
            &unshaped_roots,
            &format!("{sample_rate} Hz MIDI84 unshaped"),
        );
        assert_ordered_bands(&stiff_roots, &format!("{sample_rate} Hz MIDI84 stiff"));
        for mode in 2..=5 {
            let unshaped_ratio = unshaped_roots[mode - 1] / unshaped_roots[0];
            let stiff_ratio = stiff_roots[mode - 1] / stiff_roots[0];
            assert!(
                stiff_ratio > unshaped_ratio,
                "{sample_rate} Hz MIDI84 analytical mode {mode}: unshaped ratio {unshaped_ratio} roots {:?}, stiff ratio {stiff_ratio} roots {:?}",
                unshaped_roots,
                stiff_roots
            );
            println!("{sample_rate} Hz MIDI84 analytical mode {mode}: unshaped root {} ratio {unshaped_ratio}, stiff root {} ratio {stiff_ratio}", unshaped_roots[mode - 1], stiff_roots[mode - 1]);
        }
    }
}
