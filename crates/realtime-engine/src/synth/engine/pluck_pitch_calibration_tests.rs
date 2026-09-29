use super::pluck_pitch_tests::{
    pitch_candidate_window, SpectralPitch, DRY_MEASUREMENT_WINDOWS_SECONDS, MIN_PEAK_CONFIDENCE,
    MIN_Q_LITERAL, WINDOW_SECONDS,
};
use std::f64::consts::TAU;

#[derive(Clone)]
struct CalibrationObservation {
    label: String,
    positive: bool,
    candidate: Option<SpectralPitch>,
}

struct CalibrationCase<'a> {
    samples: &'a [f32],
    sample_rate: u32,
    expected_hz: f64,
    window_start: f64,
    label: &'a str,
    positive: bool,
    strong_positive: bool,
}

#[derive(Default)]
struct CalibrationVariation {
    max_frequency_error: f64,
    max_low_positive_q_variation: f64,
    max_low_positive_prominence_variation: f64,
    max_weak_q_variation: f64,
    max_weak_prominence_variation: f64,
}

fn synthetic_fixture(
    sample_rate: u32,
    frequency_hz: Option<f64>,
    fundamental_amplitude: f64,
    decay_seconds: f64,
    dc: f64,
    upper_modes: bool,
    mut noise_seed: Option<u32>,
) -> Vec<f32> {
    let samples =
        (DRY_MEASUREMENT_WINDOWS_SECONDS[1] + WINDOW_SECONDS + 0.05) * f64::from(sample_rate);
    let anchor_hz = frequency_hz.unwrap_or(138.591_323_85);
    (0..samples.round() as usize)
        .map(|index| {
            let time = index as f64 / f64::from(sample_rate);
            let envelope = (-time / decay_seconds).exp();
            let fundamental = frequency_hz
                .map(|frequency| fundamental_amplitude * (TAU * frequency * time).sin())
                .unwrap_or(0.0);
            let upper = if upper_modes {
                0.8 * (TAU * 2.07 * anchor_hz * time + 0.31).sin()
                    + 0.55 * (TAU * 3.13 * anchor_hz * time + 0.73).sin()
            } else {
                0.0
            };
            let noise = noise_seed.map_or(0.0, |seed| {
                let mut value = seed;
                value ^= value << 13;
                value ^= value >> 17;
                value ^= value << 5;
                noise_seed = Some(value);
                (value as f64 / u32::MAX as f64 * 2.0 - 1.0) * 0.2
            });
            (dc + envelope * (fundamental + upper + noise)) as f32
        })
        .collect()
}

fn scaled_observations(
    case: CalibrationCase<'_>,
    variation: &mut CalibrationVariation,
) -> Vec<CalibrationObservation> {
    let mut observations = Vec::new();
    let mut baseline: Option<Option<SpectralPitch>> = None;
    for scale in [1.0_f32, 0.01, 0.0001] {
        let scaled = case
            .samples
            .iter()
            .map(|sample| sample * scale)
            .collect::<Vec<_>>();
        let candidate = pitch_candidate_window(
            &scaled,
            case.sample_rate,
            case.expected_hz,
            case.window_start,
        );
        if let Some(Some(expected)) = baseline {
            if let Some(candidate) = candidate {
                if case.positive {
                    let frequency_error = (candidate.frequency_hz - case.expected_hz).abs();
                    variation.max_frequency_error =
                        variation.max_frequency_error.max(frequency_error);
                    assert!(
                        (candidate.frequency_hz - expected.frequency_hz).abs() < 0.5,
                        "{} scale {scale}: frequency changed from {} to {} Hz",
                        case.label,
                        expected.frequency_hz,
                        candidate.frequency_hz
                    );
                    if case.strong_positive {
                        for (name, actual, base) in [
                            (
                                "Q",
                                candidate.normalized_strength,
                                expected.normalized_strength,
                            ),
                            ("prominence", candidate.confidence, expected.confidence),
                        ] {
                            assert!(
                                (actual - base).abs() <= base.abs().max(1.0e-12) * 0.01,
                                "{} scale {scale}: strong-positive {name} changed from {base} to {actual}",
                                case.label
                            );
                        }
                    } else {
                        variation.max_low_positive_q_variation =
                            variation.max_low_positive_q_variation.max(
                                (candidate.normalized_strength - expected.normalized_strength)
                                    .abs(),
                            );
                        variation.max_low_positive_prominence_variation = variation
                            .max_low_positive_prominence_variation
                            .max((candidate.confidence - expected.confidence).abs());
                    }
                } else {
                    variation.max_weak_q_variation = variation
                        .max_weak_q_variation
                        .max((candidate.normalized_strength - expected.normalized_strength).abs());
                    variation.max_weak_prominence_variation = variation
                        .max_weak_prominence_variation
                        .max((candidate.confidence - expected.confidence).abs());
                }
            }
        } else if baseline.is_none() {
            baseline = Some(candidate);
        }
        if case.positive {
            let pitch = candidate.expect("positive synthetic tone must produce a raw candidate");
            assert!(
                pitch.confidence >= MIN_PEAK_CONFIDENCE,
                "{} scale {scale}: prominence {} below {}",
                case.label,
                pitch.confidence,
                MIN_PEAK_CONFIDENCE
            );
            let error = (pitch.frequency_hz - case.expected_hz).abs();
            variation.max_frequency_error = variation.max_frequency_error.max(error);
            assert!(
                error < 0.5,
                "{} scale {scale}: measured {} Hz, expected {} Hz",
                case.label,
                pitch.frequency_hz,
                case.expected_hz
            );
        }
        observations.push(CalibrationObservation {
            label: format!("{} scale {scale}", case.label),
            positive: case.positive,
            candidate,
        });
    }
    observations
}

#[test]
fn pluck_pitch_normalized_strength_calibrates_gain_invariant_positive_and_null_cases() {
    let pitches = [
        ("requested-midi49", 138.591_323_85),
        ("sharp-midi49", 141.942_290_95),
        ("midi48-second-mode", 261.625_565_3),
    ];
    let mut observations = Vec::new();
    let mut variation = CalibrationVariation::default();
    for sample_rate in [44_100_u32, 48_000] {
        for decay_seconds in [0.65, 0.20] {
            for (pitch_name, frequency) in pitches {
                for (amplitude, positive) in
                    [(0.32, true), (0.02, true), (0.002, false), (0.0, false)]
                {
                    for window_start in DRY_MEASUREMENT_WINDOWS_SECONDS {
                        let fixture = synthetic_fixture(
                            sample_rate,
                            Some(frequency),
                            amplitude,
                            decay_seconds,
                            0.37,
                            true,
                            None,
                        );
                        let label = format!("{sample_rate} {pitch_name} amp {amplitude} decay {decay_seconds} window {window_start}");
                        observations.extend(scaled_observations(
                            CalibrationCase {
                                samples: &fixture,
                                sample_rate,
                                expected_hz: frequency,
                                window_start,
                                label: &label,
                                positive,
                                strong_positive: amplitude == 0.32,
                            },
                            &mut variation,
                        ));
                    }
                }
            }
            for window_start in DRY_MEASUREMENT_WINDOWS_SECONDS {
                for seed in [0x1234_5678, 0x9abc_def0, 0x0bad_f00d, 0xfeed_beef] {
                    for upper_modes in [false, true] {
                        let fixture = synthetic_fixture(
                            sample_rate,
                            None,
                            0.0,
                            decay_seconds,
                            0.37,
                            upper_modes,
                            Some(seed),
                        );
                        let label = format!("{sample_rate} noise seed {seed:#x} uppers {upper_modes} decay {decay_seconds} window {window_start}");
                        observations.extend(scaled_observations(
                            CalibrationCase {
                                samples: &fixture,
                                sample_rate,
                                expected_hz: pitches[0].1,
                                window_start,
                                label: &label,
                                positive: false,
                                strong_positive: false,
                            },
                            &mut variation,
                        ));
                    }
                }
                for (label, dc) in [("silence", 0.0), ("dc-only", 0.37)] {
                    let fixture = synthetic_fixture(sample_rate, None, 0.0, 0.65, dc, false, None);
                    let label = format!("{sample_rate} {label} window {window_start}");
                    observations.extend(scaled_observations(
                        CalibrationCase {
                            samples: &fixture,
                            sample_rate,
                            expected_hz: pitches[0].1,
                            window_start,
                            label: &label,
                            positive: false,
                            strong_positive: false,
                        },
                        &mut variation,
                    ));
                }
            }
        }
    }

    let (positive_floor, positive_identity) = observations
        .iter()
        .filter(|row| row.positive)
        .filter_map(|row| {
            row.candidate
                .filter(|pitch| pitch.confidence >= MIN_PEAK_CONFIDENCE)
                .map(|pitch| (pitch.normalized_strength, row.label.as_str()))
        })
        .min_by(|left, right| left.0.total_cmp(&right.0))
        .expect("qualified positive calibration candidate must be available");
    let (negative_ceiling, negative_identity) = observations
        .iter()
        .filter(|row| !row.positive)
        .filter_map(|row| {
            row.candidate
                .filter(|pitch| pitch.confidence >= MIN_PEAK_CONFIDENCE)
                .map(|pitch| (pitch.normalized_strength, row.label.as_str()))
        })
        .max_by(|left, right| left.0.total_cmp(&right.0))
        .expect("prominent weak/null calibration candidate must be available");
    assert!(
        positive_floor >= 4.0 * negative_ceiling,
        "synthetic Q classes overlap: P={positive_floor} ({positive_identity}), N={negative_ceiling} ({negative_identity})"
    );
    assert!(
        MIN_Q_LITERAL >= 2.0 * negative_ceiling,
        "fixed Q threshold {MIN_Q_LITERAL} lacks 2x margin above N={negative_ceiling} ({negative_identity})"
    );
    assert!(
        MIN_Q_LITERAL <= positive_floor / 2.0,
        "fixed Q threshold {MIN_Q_LITERAL} lacks 2x margin below P={positive_floor} ({positive_identity})"
    );
    let proposed_threshold = (positive_floor * negative_ceiling).sqrt();
    let false_acceptances = observations
        .iter()
        .filter(|row| {
            row.candidate.is_some_and(|pitch| {
                pitch.confidence >= MIN_PEAK_CONFIDENCE
                    && pitch.normalized_strength >= MIN_Q_LITERAL
            }) != row.positive
        })
        .count();
    assert_eq!(
        false_acceptances, 0,
        "fixed Q threshold {MIN_Q_LITERAL} misclassified fixtures (geometric proposal {proposed_threshold})"
    );
    println!("synthetic calibration: P={positive_floor} ({positive_identity}), N={negative_ceiling} ({negative_identity}), proposed T={proposed_threshold}, max frequency error={} Hz, max lower-positive Q/prominence variation={}/{}, max weak/null Q/prominence variation={}/{}, false acceptances={false_acceptances}", variation.max_frequency_error, variation.max_low_positive_q_variation, variation.max_low_positive_prominence_variation, variation.max_weak_q_variation, variation.max_weak_prominence_variation);
}
