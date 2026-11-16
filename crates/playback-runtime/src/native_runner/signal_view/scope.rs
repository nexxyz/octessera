use crate::oled_frame::{OledSignalVisual, SIGNAL_WIDTH};

/// Quieter than this reads as silence and draws the flat line.
const SILENCE: f32 = 1.0e-3;
/// Signals below this peak are not scaled up any further.
const GAIN_FLOOR: f32 = 0.25;
/// A trigger re-arms once the signal falls below this share of its peak.
const ARM_SHARE: f32 = 0.3;

/// A band-wide trace of the newest samples, `stride` samples a column. It starts at the latest
/// rising zero crossing that follows a dip below the arm level, so steady tones stand still;
/// flat when silent.
pub(super) fn scope_visual(samples: &[f32], stride: usize) -> OledSignalVisual {
    let span = SIGNAL_WIDTH * stride;
    let peak = samples
        .iter()
        .fold(0.0_f32, |peak, sample| peak.max(sample.abs()));
    if peak < SILENCE || samples.len() < span {
        return OledSignalVisual::Flat;
    }
    let latest = samples.len() - span;
    let start = trigger(&samples[..=latest], -peak * ARM_SHARE).unwrap_or(latest);
    let gain = 127.0 / peak.max(GAIN_FLOOR);
    let points = samples[start..start + span]
        .iter()
        .step_by(stride)
        .map(|sample| (sample * gain).round().clamp(-127.0, 127.0) as i8)
        .collect();
    OledSignalVisual::Scope { points }
}

fn trigger(samples: &[f32], arm_below: f32) -> Option<usize> {
    let mut armed = false;
    let mut found = None;
    for (index, pair) in samples.windows(2).enumerate() {
        armed |= pair[0] < arm_below;
        if armed && pair[0] < 0.0 && pair[1] >= 0.0 {
            found = Some(index + 1);
            armed = false;
        }
    }
    found
}
