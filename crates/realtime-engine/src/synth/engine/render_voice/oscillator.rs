//! Band-limited oscillator waveforms for synth voices.

use super::{OscRenderConfig, WaveformId};
use crate::sine::sin_turns;

#[inline(always)]
pub(super) fn osc_sample_precomputed(cfg: OscRenderConfig, inc: f32, phase: &mut f32) -> f32 {
    *phase = (*phase + inc).fract();

    let raw = match cfg.waveform {
        WaveformId::Sine => sin_turns(*phase),
        WaveformId::Triangle => 4.0 * (*phase - 0.5).abs() - 1.0,
        WaveformId::Saw => 2.0 * *phase - 1.0 - poly_blep(*phase, inc),
        WaveformId::Square => pulse_sample(*phase, 0.5, inc),
        WaveformId::Pulse => pulse_sample(*phase, cfg.pulse_duty, inc),
    };

    raw * cfg.level
}

/// A band-limited ±1 pulse with its DC removed, so a narrow pulse sits around zero like a square
/// does and sounds as loud as the full-swing pulse it is; its short side peaks above 1.
#[inline(always)]
pub(super) fn pulse_sample(phase: f32, duty: f32, inc: f32) -> f32 {
    let dc = 2.0 * duty - 1.0;
    let (naive, since_fall) = if phase < duty {
        (1.0 - dc, phase - duty + 1.0)
    } else {
        (-1.0 - dc, phase - duty)
    };
    naive + poly_blep(phase, inc) - poly_blep(since_fall, inc)
}

/// Band-limits a unit step at phase 0 over one sample on either side of it.
#[inline(always)]
pub(super) fn poly_blep(phase: f32, inc: f32) -> f32 {
    if phase < inc {
        let x = phase / inc;
        2.0 * x - x * x - 1.0
    } else if phase > 1.0 - inc {
        let x = (phase - 1.0) / inc;
        x * x + 2.0 * x + 1.0
    } else {
        0.0
    }
}
