use std::f64::consts::TAU;

pub(super) const WINDOW_SECONDS: f64 = 1.0;
pub(super) const DRY_MEASUREMENT_WINDOWS_SECONDS: [f64; 2] = [0.05, 0.55];
pub(super) const MIN_Q_LITERAL: f64 = 0.008_434_549_54;
pub(super) const MIN_PEAK_CONFIDENCE: f64 = 8.0;

#[derive(Clone, Copy, Debug)]
pub(super) struct SpectralPitch {
    pub(super) frequency_hz: f64,
    pub(super) strength: f64,
    pub(super) normalized_strength: f64,
    pub(super) confidence: f64,
    pub(super) hann_weighted_rms: f64,
}

fn estimate_pitch_window(
    samples: &[f32],
    sample_rate: u32,
    expected_hz: f64,
    start_seconds: f64,
) -> Option<SpectralPitch> {
    pitch_candidate_window(samples, sample_rate, expected_hz, start_seconds).filter(|candidate| {
        candidate.normalized_strength >= MIN_Q_LITERAL
            && candidate.confidence >= MIN_PEAK_CONFIDENCE
    })
}

struct PitchSpectrum {
    amplitudes: Vec<f64>,
    first_bin: usize,
    length: usize,
    hann_weighted_rms: f64,
}

fn pitch_spectrum_window(
    samples: &[f32],
    sample_rate: u32,
    low_hz: f64,
    high_hz: f64,
    start_seconds: f64,
) -> Option<PitchSpectrum> {
    let start = (start_seconds * f64::from(sample_rate)).round() as usize;
    let length = (WINDOW_SECONDS * f64::from(sample_rate)).round() as usize;
    let window = samples.get(start..start + length)?;
    let mean = window.iter().map(|sample| f64::from(*sample)).sum::<f64>() / length as f64;
    let hann_weights = (0..length)
        .map(|index| 0.5 - 0.5 * (TAU * index as f64 / (length - 1) as f64).cos())
        .collect::<Vec<_>>();
    let hann_sum = hann_weights.iter().sum::<f64>();
    let hann_squared_sum = hann_weights
        .iter()
        .map(|weight| weight * weight)
        .sum::<f64>();
    let hann_weighted_rms = (window
        .iter()
        .zip(&hann_weights)
        .map(|(sample, weight)| weight.powi(2) * (f64::from(*sample) - mean).powi(2))
        .sum::<f64>()
        / hann_squared_sum)
        .sqrt();
    if !hann_weighted_rms.is_finite() || hann_weighted_rms == 0.0 {
        return None;
    }
    let windowed = window
        .iter()
        .zip(&hann_weights)
        .map(|(sample, hann)| (f64::from(*sample) - mean) * hann / hann_weighted_rms)
        .collect::<Vec<_>>();
    let first_bin = (low_hz * length as f64 / f64::from(sample_rate)).ceil() as usize;
    let last_bin = (high_hz * length as f64 / f64::from(sample_rate)).floor() as usize;
    let amplitudes = (first_bin.saturating_sub(1)..=last_bin + 1)
        .map(|bin| {
            let phase_step = TAU * bin as f64 / length as f64;
            let (sin_step, cos_step) = phase_step.sin_cos();
            let (mut sin_phase, mut cos_phase) = (0.0, 1.0);
            let (mut real, mut imaginary) = (0.0, 0.0);
            for sample in &windowed {
                real += sample * cos_phase;
                imaginary -= sample * sin_phase;
                let next_sin = sin_phase * cos_step + cos_phase * sin_step;
                cos_phase = cos_phase * cos_step - sin_phase * sin_step;
                sin_phase = next_sin;
            }
            2.0 * real.hypot(imaginary) / hann_sum
        })
        .collect::<Vec<_>>();
    Some(PitchSpectrum {
        amplitudes,
        first_bin: first_bin.saturating_sub(1),
        length,
        hann_weighted_rms,
    })
}

pub(super) fn pitch_candidate_window(
    samples: &[f32],
    sample_rate: u32,
    expected_hz: f64,
    start_seconds: f64,
) -> Option<SpectralPitch> {
    pitch_candidate_band_window(
        samples,
        sample_rate,
        expected_hz * 0.75,
        expected_hz * 1.25,
        start_seconds,
    )
}

pub(super) fn pitch_candidate_band_window(
    samples: &[f32],
    sample_rate: u32,
    low_hz: f64,
    high_hz: f64,
    start_seconds: f64,
) -> Option<SpectralPitch> {
    let spectrum = pitch_spectrum_window(samples, sample_rate, low_hz, high_hz, start_seconds)?;
    let peak = (1..spectrum.amplitudes.len() - 1)
        .filter(|index| {
            spectrum.amplitudes[*index] >= spectrum.amplitudes[*index - 1]
                && spectrum.amplitudes[*index] >= spectrum.amplitudes[*index + 1]
        })
        .filter(|index| {
            let bin = spectrum.first_bin + index;
            let frequency = f64::from(sample_rate) * bin as f64 / spectrum.length as f64;
            (low_hz..=high_hz).contains(&frequency)
        })
        .max_by(|left, right| spectrum.amplitudes[*left].total_cmp(&spectrum.amplitudes[*right]))?;
    Some(pitch_estimate_at_bin(&spectrum, sample_rate, peak))
}

fn strongest_raw_pitch_bin_window(
    samples: &[f32],
    sample_rate: u32,
    expected_hz: f64,
    start_seconds: f64,
) -> Option<SpectralPitch> {
    let spectrum = pitch_spectrum_window(
        samples,
        sample_rate,
        expected_hz * 0.75,
        expected_hz * 1.25,
        start_seconds,
    )?;
    let peak = (1..spectrum.amplitudes.len() - 1)
        .max_by(|left, right| spectrum.amplitudes[*left].total_cmp(&spectrum.amplitudes[*right]))?;
    Some(pitch_estimate_at_bin(&spectrum, sample_rate, peak))
}

fn pitch_estimate_at_bin(spectrum: &PitchSpectrum, sample_rate: u32, peak: usize) -> SpectralPitch {
    let normalized_strength = spectrum.amplitudes[peak];
    let left = spectrum.amplitudes[peak - 1].ln();
    let center = normalized_strength.ln();
    let right = spectrum.amplitudes[peak + 1].ln();
    let denominator = left - 2.0 * center + right;
    let offset = if denominator.abs() > f64::EPSILON {
        (0.5 * (left - right) / denominator).clamp(-0.5, 0.5)
    } else {
        0.0
    };
    let noise_floor = spectrum
        .amplitudes
        .iter()
        .enumerate()
        .filter(|(index, _)| index.abs_diff(peak) > 2)
        .map(|(_, amplitude)| *amplitude)
        .collect::<Vec<_>>();
    let mut sorted_noise = noise_floor;
    sorted_noise.sort_by(f64::total_cmp);
    let confidence = normalized_strength / sorted_noise[sorted_noise.len() / 2].max(1.0e-12);
    SpectralPitch {
        frequency_hz: f64::from(sample_rate) * (spectrum.first_bin as f64 + peak as f64 + offset)
            / spectrum.length as f64,
        strength: normalized_strength * spectrum.hann_weighted_rms,
        normalized_strength,
        confidence,
        hann_weighted_rms: spectrum.hann_weighted_rms,
    }
}

fn window_rms_and_max(samples: &[f32], sample_rate: u32, start_seconds: f64) -> Option<(f64, f64)> {
    let start = (start_seconds * f64::from(sample_rate)).round() as usize;
    let length = (WINDOW_SECONDS * f64::from(sample_rate)).round() as usize;
    let window = samples.get(start..start + length)?;
    let rms = (window
        .iter()
        .map(|sample| f64::from(*sample).powi(2))
        .sum::<f64>()
        / length as f64)
        .sqrt();
    let maximum = window
        .iter()
        .map(|sample| f64::from(sample.abs()))
        .fold(0.0, f64::max);
    Some((rms, maximum))
}

fn note_frequency(note: u8) -> f64 {
    440.0 * 2.0_f64.powf((f64::from(note) - 69.0) / 12.0)
}

#[derive(Clone, Copy, Debug)]
pub(super) enum PluckPitchReference {
    ProductionZero,
    CompensatedUnshaped,
    ProductionStiff,
}

pub(super) struct RenderedPluck {
    pub(super) samples: Vec<f32>,
    pub(super) delay: usize,
    pub(super) fraction: f32,
    pub(super) coefficient: f32,
    pub(super) loss: f32,
    pub(super) brightness: f32,
}

pub(super) fn render_dry_pluck(
    note: u8,
    velocity: u8,
    sample_rate: u32,
    reference: PluckPitchReference,
) -> RenderedPluck {
    use crate::synth::pluck_string::{PluckRing, PluckState, PluckStringSettings, RING_LEN};
    use crate::synth::PluckConfig;

    let frequency = note_frequency(note);
    let config = PluckConfig::default();
    let dispersion_pct = match reference {
        PluckPitchReference::ProductionZero | PluckPitchReference::CompensatedUnshaped => 0.0,
        PluckPitchReference::ProductionStiff => 100.0,
    };
    let settings = PluckStringSettings {
        dispersion_pct,
        ..config.into()
    };
    let mut state = PluckState::note_on(frequency as f32, sample_rate, note, velocity, settings);
    if matches!(reference, PluckPitchReference::CompensatedUnshaped) {
        let tuning = crate::synth::pluck_dispersion::tune(
            frequency,
            f64::from(sample_rate),
            f64::from(state.loss),
            f64::from(state.brightness),
            0.0,
        )
        .unwrap();
        state.delay = tuning.delay;
        state.fraction = tuning.fraction;
        state.pick_offset = (((tuning.delay as f32 + tuning.fraction) * config.pick_position_pct
            / 100.0)
            .round() as usize)
            .clamp(1, tuning.delay.saturating_sub(1));
    }
    let frames = (0.05 + WINDOW_SECONDS * 2.0 + 0.05) * f64::from(sample_rate);
    let mut ring: PluckRing = [0.0; RING_LEN];
    let samples = (0..frames.round() as usize)
        .map(|_| state.next(&mut ring))
        .collect();
    RenderedPluck {
        samples,
        delay: state.delay,
        fraction: state.fraction,
        coefficient: state.dispersion_coefficient,
        loss: state.loss,
        brightness: state.brightness,
    }
}

#[test]
fn dry_pluck_fundamental_matches_requested_pitch_across_notes_rates_and_references() {
    let references = [
        PluckPitchReference::ProductionZero,
        PluckPitchReference::CompensatedUnshaped,
        PluckPitchReference::ProductionStiff,
    ];
    for sample_rate in [44_100_u32, 48_000] {
        for note in [48_u8, 49, 60] {
            let target = note_frequency(note);
            for reference in references {
                let mut measured = Vec::new();
                let mut diagnostics = Vec::new();
                for velocity in [96_u8, 120] {
                    let rendered = render_dry_pluck(note, velocity, sample_rate, reference);
                    for window_start in DRY_MEASUREMENT_WINDOWS_SECONDS {
                        let estimate = estimate_pitch_window(
                            &rendered.samples,
                            sample_rate,
                            target,
                            window_start,
                        )
                        .unwrap_or_else(|| {
                            let candidate = pitch_candidate_window(
                                &rendered.samples,
                                sample_rate,
                                target,
                                window_start,
                            );
                            let raw_bin = if candidate.is_none() {
                                strongest_raw_pitch_bin_window(
                                    &rendered.samples,
                                    sample_rate,
                                    target,
                                    window_start,
                                )
                            } else {
                                None
                            };
                            let (rms, maximum) = window_rms_and_max(
                                &rendered.samples,
                                sample_rate,
                                window_start,
                            )
                            .unwrap_or((f64::NAN, f64::NAN));
                            panic!(
                                "{sample_rate} Hz MIDI {note} {reference:?} velocity {velocity} window {window_start}: unresolved fundamental; local candidate {candidate:?}; strongest raw bin (only if no local peak) {raw_bin:?}; window RMS {rms}, max {maximum}; N {} mu {} a {} loss {} brightness {} target {target} Hz",
                                rendered.delay,
                                rendered.fraction,
                                rendered.coefficient,
                                rendered.loss,
                                rendered.brightness
                            )
                        });
                        measured.push(estimate.frequency_hz);
                        diagnostics.push(format!(
                            "vel {velocity} window {window_start}: {} Hz raw strength {} Q {} confidence {} R_H {} N {} mu {} a {} loss {} brightness {}",
                            estimate.frequency_hz,
                            estimate.strength,
                            estimate.normalized_strength,
                            estimate.confidence,
                            estimate.hann_weighted_rms,
                            rendered.delay,
                            rendered.fraction,
                            rendered.coefficient,
                            rendered.loss,
                            rendered.brightness
                        ));
                    }
                }
                let lowest = measured.iter().copied().reduce(f64::min).unwrap();
                let highest = measured.iter().copied().reduce(f64::max).unwrap();
                assert!(
                    highest - lowest < 0.75,
                    "{sample_rate} Hz MIDI {note} {reference:?}: fixed windows/seeds disagree ({lowest}..{highest} Hz), target {target} Hz; {}",
                    diagnostics.join("; ")
                );
                let actual = measured.iter().sum::<f64>() / measured.len() as f64;
                assert!(
                    (actual - target).abs() < 1.0,
                    "{sample_rate} Hz MIDI {note} {reference:?}: confident dry fundamental {actual} Hz differs from target {target} Hz; {}",
                    diagnostics.join("; ")
                );
                if note == 49 {
                    println!(
                        "dry MIDI49 {sample_rate} Hz {reference:?}: target {target} Hz, measured {actual} Hz; {}",
                        diagnostics.join("; ")
                    );
                }
            }
        }
    }
}
