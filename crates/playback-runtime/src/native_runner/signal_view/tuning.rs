use super::{VisualKind, STEP};
use crate::oled_frame::SIGNAL_WIDTH;
use std::time::Duration;

/// Tapped samples per scope column: 124 columns show about 6, 11, 22, 45 or 90 ms at 44.1 kHz.
const SCOPE_STRIDES: [usize; 5] = [2, 4, 8, 16, 32];
/// Spectrum bands: bars 1, 3, 7 or 15 px wide with a 1 px gap.
const SPECTRUM_BANDS: [usize; 4] = [62, 31, 16, 8];
/// Waterfall history as live steps per column: two columns a step hold about 4 s, one about
/// 8 s; the longer ones take a column every second or fourth step.
const WATERFALL_STEPS: [(usize, u32); 4] = [(2, 1), (1, 1), (1, 2), (1, 4)];
const WATERFALL_SECONDS: [u32; 4] = [4, 8, 16, 32];
const NOMINAL_RATE: u32 = 44_100;

/// The session's adjustment of each visual, shared by the home band and the full-screen views.
/// It is never saved: a restart starts from the defaults.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::native_runner) struct VisualTuning {
    scope: usize,
    spectrum: usize,
    waterfall: usize,
}

impl Default for VisualTuning {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl VisualTuning {
    /// An 11 ms scope, 62 spectrum bands and 8 s of waterfall.
    pub(in crate::native_runner) const DEFAULT: Self = Self {
        scope: 1,
        spectrum: 0,
        waterfall: 1,
    };

    /// Moves `kind`'s adjustment `delta` steps within its range, right towards the wider window,
    /// fewer bands or longer history.
    pub(in crate::native_runner) fn adjust(&mut self, kind: VisualKind, delta: i32) {
        let (index, len) = match kind {
            VisualKind::Scope => (&mut self.scope, SCOPE_STRIDES.len()),
            VisualKind::Spectrum => (&mut self.spectrum, SPECTRUM_BANDS.len()),
            VisualKind::Waterfall => (&mut self.waterfall, WATERFALL_STEPS.len()),
        };
        *index = index.saturating_add_signed(delta as isize).min(len - 1);
    }

    /// The toast that names `kind`'s adjustment; the scope's window follows the tap's `rate`.
    pub(in crate::native_runner) fn label(self, kind: VisualKind, rate: u32) -> String {
        match kind {
            VisualKind::Scope => {
                let rate = if rate == 0 { NOMINAL_RATE } else { rate } as usize;
                let samples = SIGNAL_WIDTH * self.scope_stride() * 1000;
                format!("Window: {} ms", (samples + rate / 2) / rate)
            }
            VisualKind::Spectrum => format!("Bands: {}", self.spectrum_bands()),
            VisualKind::Waterfall => format!("History: {} s", WATERFALL_SECONDS[self.waterfall]),
        }
    }

    pub(super) fn scope_stride(self) -> usize {
        SCOPE_STRIDES[self.scope]
    }

    pub(super) fn spectrum_bands(self) -> usize {
        SPECTRUM_BANDS[self.spectrum]
    }

    /// The waterfall columns one step adds.
    pub(super) fn waterfall_columns(self) -> usize {
        WATERFALL_STEPS[self.waterfall].0
    }

    /// How long one live step of `kind` lasts.
    pub(super) fn step(self, kind: VisualKind) -> Duration {
        match kind {
            VisualKind::Waterfall => STEP * WATERFALL_STEPS[self.waterfall].1,
            VisualKind::Scope | VisualKind::Spectrum => STEP,
        }
    }
}
