use super::fft::Fft;
use crate::oled_frame::{OledSignalVisual, SIGNAL_BODY_HEIGHT, SIGNAL_WIDTH, SPECTRUM_BARS};

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
    /// Columns, oldest first, each a level per pixel row from the lowest bin up; room for the
    /// tallest view.
    waterfall: Vec<u8>,
    /// The FFT bin range of each log band, for the rate and band count in `ranges_for`.
    ranges: Vec<(usize, usize)>,
    ranges_for: (u32, usize),
}

impl SpectrumHistory {
    pub(super) fn new(size: usize) -> Self {
        Self {
            fft: Fft::new(size),
            size,
            bins: vec![0; SPECTRUM_BARS.max(SIGNAL_BODY_HEIGHT)],
            bars: vec![0; SPECTRUM_BARS],
            peaks: vec![0; SPECTRUM_BARS],
            waterfall: vec![0; SIGNAL_WIDTH * SIGNAL_BODY_HEIGHT],
            ranges: Vec::new(),
            ranges_for: (0, 0),
        }
    }

    pub(super) fn clear(&mut self) {
        self.bars.fill(0);
        self.peaks.fill(0);
        self.waterfall.fill(0);
    }

    /// `bands` log-frequency bars over `samples` taken at `rate`, `steps` live steps after the
    /// previous ones: they jump up and fall gently, and a peak dot falls slower above each. Flat
    /// once everything has fallen.
    pub(super) fn spectrum(
        &mut self,
        samples: &[f32],
        rate: u32,
        bands: usize,
        steps: usize,
    ) -> OledSignalVisual {
        let bands = bands.min(SPECTRUM_BARS);
        self.band_levels(samples, rate, bands);
        let levels = &self.bins[..bands];
        let (bars, peaks) = (&mut self.bars[..bands], &mut self.peaks[..bands]);
        let fall = |per_step: u8| u8::try_from(usize::from(per_step) * steps).unwrap_or(u8::MAX);
        let (bar_fall, peak_fall) = (fall(BAR_FALL), fall(PEAK_FALL));
        for ((bar, peak), level) in bars.iter_mut().zip(peaks.iter_mut()).zip(levels) {
            *bar = (*level).max(bar.saturating_sub(bar_fall));
            *peak = (*bar).max(peak.saturating_sub(peak_fall));
        }
        if peaks.iter().all(|peak| *peak == 0) {
            return OledSignalVisual::Flat;
        }
        OledSignalVisual::Spectrum {
            levels: bars.to_vec(),
            peaks: peaks.to_vec(),
        }
    }

    /// Scrolls a waterfall `rows` tall one column left per block, oldest block first, and adds
    /// each block's column on the same log bins, one per pixel row; then the newest column again
    /// for each of `repeats` missed columns. Flat while the whole history is dark.
    pub(super) fn waterfall(
        &mut self,
        blocks: &[&[f32]],
        rate: u32,
        rows: usize,
        repeats: usize,
    ) -> OledSignalVisual {
        for block in blocks {
            self.band_levels(block, rate, rows);
            self.add_column(rows);
        }
        for _ in 0..repeats.min(SIGNAL_WIDTH) {
            self.add_column(rows);
        }
        let history = &self.waterfall[..SIGNAL_WIDTH * rows];
        if history.iter().all(|level| *level == 0) {
            return OledSignalVisual::Flat;
        }
        OledSignalVisual::Waterfall {
            levels: history.to_vec(),
        }
    }

    /// Scrolls the history one column left and adds the bins as its newest column.
    fn add_column(&mut self, rows: usize) {
        let history = &mut self.waterfall[..SIGNAL_WIDTH * rows];
        history.rotate_left(rows);
        let newest = history.len() - rows;
        history[newest..].copy_from_slice(&self.bins[..rows]);
    }

    /// Fills the first `count` bins with one level per log band of `samples` taken at `rate`:
    /// the loudest FFT bin in the band.
    fn band_levels(&mut self, samples: &[f32], rate: u32, count: usize) {
        if self.ranges_for != (rate, count) {
            self.ranges_for = (rate, count);
            self.ranges = band_ranges(rate, count, self.size);
        }
        let powers = self.fft.powers(samples);
        for (level, &(first, end)) in self.bins.iter_mut().zip(&self.ranges) {
            let loudest = powers[first..end].iter().fold(0.0_f32, |a, b| a.max(*b));
            *level = level_of(loudest, self.size);
        }
    }
}

/// The FFT bins of `count` log-spaced bands from [`LOW_HZ`] to Nyquist, or the nearest bin
/// where a band is narrower than one; empty ranges before the tap knows its rate.
fn band_ranges(rate: u32, count: usize, size: usize) -> Vec<(usize, usize)> {
    if rate == 0 {
        return vec![(0, 0); count];
    }
    let bin_hz = rate as f32 / size as f32;
    let span = (rate as f32 / 2.0 / LOW_HZ).ln();
    let edge = |band: usize| LOW_HZ * (span * band as f32 / count as f32).exp() / bin_hz;
    let last_bin = size / 2 - 1;
    (0..count)
        .map(|band| {
            let first = (edge(band).round() as usize).clamp(1, last_bin);
            let end = (edge(band + 1).round() as usize).clamp(first + 1, last_bin + 1);
            (first, end)
        })
        .collect()
}

/// The level of a bin's power on the [`FLOOR_DB`] scale up to a full-scale sine.
fn level_of(power: f32, size: usize) -> u8 {
    let full_scale = size as f32 / 4.0;
    let db = 10.0 * (power / (full_scale * full_scale)).max(1.0e-18).log10();
    ((db - FLOOR_DB) / -FLOOR_DB * 255.0)
        .round()
        .clamp(0.0, 255.0) as u8
}

#[cfg(test)]
mod tests;
