use super::fft::Fft;
use crate::oled_frame::{OledSignalVisual, SIGNAL_BAND_HEIGHT, SIGNAL_BAND_WIDTH, SPECTRUM_BARS};

/// The lowest frequency the log bins show; the highest is the tapped signal's Nyquist.
const LOW_HZ: f32 = 40.0;
/// Level 0 sits this far below a full-scale sine, level 255 at it.
const FLOOR_DB: f32 = -54.0;
/// Per 66 ms step a bar falls this many levels (a full bar in about 0.7 s), a peak this many.
const BAR_FALL: u8 = 24;
const PEAK_FALL: u8 = 6;

/// The spectrum bars and waterfall columns of one live run; the FFT runs once per step.
#[derive(Clone)]
pub(super) struct SpectrumHistory {
    fft: Fft,
    size: usize,
    bins: Vec<u8>,
    bars: Vec<u8>,
    peaks: Vec<u8>,
    /// Band columns, oldest first, each a level per band row from the lowest bin up.
    waterfall: Vec<u8>,
}

impl SpectrumHistory {
    pub(super) fn new(size: usize) -> Self {
        Self {
            fft: Fft::new(size),
            size,
            bins: vec![0; SPECTRUM_BARS.max(SIGNAL_BAND_HEIGHT)],
            bars: vec![0; SPECTRUM_BARS],
            peaks: vec![0; SPECTRUM_BARS],
            waterfall: vec![0; SIGNAL_BAND_WIDTH * SIGNAL_BAND_HEIGHT],
        }
    }

    pub(super) fn clear(&mut self) {
        self.bars.fill(0);
        self.peaks.fill(0);
        self.waterfall.fill(0);
    }

    /// Log-frequency bars over `samples` taken at `rate`: they jump up and fall gently, and a
    /// peak dot falls slower above each. Flat once everything has fallen.
    pub(super) fn spectrum(&mut self, samples: &[f32], rate: u32) -> OledSignalVisual {
        let levels = &mut self.bins[..SPECTRUM_BARS];
        log_levels(self.fft.magnitudes(samples), rate, self.size, levels);
        for ((bar, peak), level) in self.bars.iter_mut().zip(&mut self.peaks).zip(levels) {
            *bar = (*level).max(bar.saturating_sub(BAR_FALL));
            *peak = (*bar).max(peak.saturating_sub(PEAK_FALL));
        }
        if self.peaks.iter().all(|peak| *peak == 0) {
            return OledSignalVisual::Flat;
        }
        OledSignalVisual::Spectrum {
            levels: self.bars.clone(),
            peaks: self.peaks.clone(),
        }
    }

    /// Scrolls the waterfall one column left and adds the newest on the same log bins, one
    /// per band row. Flat while the whole history is dark.
    pub(super) fn waterfall(&mut self, samples: &[f32], rate: u32) -> OledSignalVisual {
        let column = &mut self.bins[..SIGNAL_BAND_HEIGHT];
        log_levels(self.fft.magnitudes(samples), rate, self.size, column);
        self.waterfall.rotate_left(SIGNAL_BAND_HEIGHT);
        let newest = self.waterfall.len() - SIGNAL_BAND_HEIGHT;
        self.waterfall[newest..].copy_from_slice(column);
        if self.waterfall.iter().all(|level| *level == 0) {
            return OledSignalVisual::Flat;
        }
        OledSignalVisual::Waterfall {
            levels: self.waterfall.clone(),
        }
    }
}

/// Fills `out` with one level per log-spaced band from [`LOW_HZ`] to Nyquist: the loudest FFT
/// bin in the band, or the nearest one where a band is narrower than a bin.
fn log_levels(magnitudes: &[f32], rate: u32, size: usize, out: &mut [u8]) {
    if rate == 0 {
        out.fill(0);
        return;
    }
    let bin_hz = rate as f32 / size as f32;
    let span = (rate as f32 / 2.0 / LOW_HZ).ln();
    let count = out.len() as f32;
    let edge = |band: usize| LOW_HZ * (span * band as f32 / count).exp() / bin_hz;
    let last_bin = magnitudes.len() - 1;
    for (band, level) in out.iter_mut().enumerate() {
        let first = (edge(band).round() as usize).clamp(1, last_bin);
        let end = (edge(band + 1).round() as usize).clamp(first + 1, last_bin + 1);
        let loudest = magnitudes[first..end]
            .iter()
            .fold(0.0_f32, |a, b| a.max(*b));
        *level = level_of(loudest, size);
    }
}

fn level_of(magnitude: f32, size: usize) -> u8 {
    let full_scale = size as f32 / 4.0;
    let db = 20.0 * (magnitude / full_scale).max(1.0e-9).log10();
    ((db - FLOOR_DB) / -FLOOR_DB * 255.0)
        .round()
        .clamp(0.0, 255.0) as u8
}

#[cfg(test)]
mod tests;
