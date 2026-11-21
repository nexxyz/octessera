use super::pluck_string::RING_LEN;
use std::f64::consts::TAU;

#[derive(Clone, Copy, Debug)]
pub(super) struct DispersionTuning {
    pub(super) delay: usize,
    pub(super) fraction: f32,
}

/// The allpass coefficient for `dispersion_pct`, held back so the loop can still close at the
/// fundamental; `None` when the note is out of range.
pub(super) fn coefficient(
    frequency_hz: f64,
    sample_rate: f64,
    loss: f64,
    brightness: f64,
    dispersion_pct: f64,
) -> Option<f32> {
    let w0 = TAU * frequency_hz / sample_rate;
    if !(w0.is_finite() && w0 > 0.0 && w0 < std::f64::consts::PI) {
        return None;
    }
    let psi_g = loop_filter_phase(w0, loss, brightness);
    let phase_budget = TAU - 3.0 * w0 - psi_g;
    let requested = -0.85 * dispersion_pct / 100.0;
    let coefficient = if phase_budget >= std::f64::consts::PI {
        requested
    } else if phase_budget > w0 {
        requested
            .max(((w0 - phase_budget) * 0.5).sin() / ((w0 + phase_budget) * 0.5).sin() + 1.0e-7)
    } else {
        0.0
    } as f32;
    (coefficient.is_finite() && coefficient.abs() < 1.0).then_some(coefficient)
}

fn loop_filter_phase(w0: f64, loss: f64, brightness: f64) -> f64 {
    let q = loss * (1.0 - brightness);
    (q * w0.sin()).atan2(1.0 - q * w0.cos())
}

/// The whole and fractional delay that close the loop at the fundamental with allpass
/// `coefficient`.
pub(super) fn delay(
    frequency_hz: f64,
    sample_rate: f64,
    loss: f64,
    brightness: f64,
    coefficient: f32,
) -> Option<DispersionTuning> {
    let w0 = TAU * frequency_hz / sample_rate;
    if !(w0.is_finite() && w0 > 0.0 && w0 < std::f64::consts::PI) {
        return None;
    }
    let psi_g = loop_filter_phase(w0, loss, brightness);
    let a = coefficient as f64;
    let psi_a = if a == 0.0 {
        0.0
    } else {
        ((1.0 - a * a) * w0.sin()).atan2((1.0 + a * a) * w0.cos() + 2.0 * a)
    };
    let total_phase = TAU - psi_g - psi_a;
    let integer = (total_phase / w0).floor();
    let theta = total_phase - integer * w0;
    let fraction = if theta <= f64::EPSILON {
        0.0
    } else {
        theta.sin() / (theta.sin() + (w0 - theta).sin())
    } as f32;
    if !integer.is_finite()
        || !fraction.is_finite()
        || !(3.0..=(RING_LEN - 2) as f64).contains(&integer)
        || !(0.0..1.0).contains(&fraction)
    {
        return None;
    }
    Some(DispersionTuning {
        delay: integer as usize,
        fraction,
    })
}
