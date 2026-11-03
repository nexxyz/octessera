use platform_core::palette;

use super::menu_rows::{slot_top, PITCH};
use super::model::{OledSignalBand, OledSignalVisual};
use super::pixels::{dim, fill_rect, rgb565};

/// How far the rows after the band drop below their slots: less than two row pitches.
pub(super) const SIGNAL_BAND_DROP: usize = 16;
const BAND_X: usize = 2;
pub const SIGNAL_BAND_WIDTH: usize = 124;
/// From the band slot's bar top to two rows above the bar of the dropped row after it.
pub const SIGNAL_BAND_HEIGHT: usize = PITCH - 2 + SIGNAL_BAND_DROP;
/// 1 px bars with a 1 px gap.
pub const SPECTRUM_BARS: usize = SIGNAL_BAND_WIDTH / 2;
const MID: usize = (SIGNAL_BAND_HEIGHT - 1) / 2;
/// Waterfall levels climb through these colours.
const RAMP: [(u8, [u8; 3]); 5] = [
    (0, palette::BLACK),
    (80, [12, 52, 61]),
    (150, palette::BLUE),
    (210, palette::MAGENTA),
    (255, palette::YELLOW),
];

pub(super) fn draw_signal_band(frame: &mut [u8], band: &OledSignalBand, body_top_offset: usize) {
    let top = slot_top(body_top_offset, band.slot) - 1;
    match &band.visual {
        OledSignalVisual::Flat => fill_rect(
            frame,
            BAND_X,
            top + MID,
            SIGNAL_BAND_WIDTH,
            1,
            rgb565(dim(palette::GRAY, 5)),
        ),
        OledSignalVisual::Scope { points } => draw_scope(frame, top, points),
        OledSignalVisual::Spectrum { levels, peaks } => draw_spectrum(frame, top, levels, peaks),
        OledSignalVisual::Waterfall { levels } => draw_waterfall(frame, top, levels),
    }
}

fn draw_scope(frame: &mut [u8], top: usize, points: &[i8]) {
    let color = rgb565(palette::BLUE);
    let row = |point: i8| {
        let offset = (i32::from(point) * MID as i32 + 63).div_euclid(127);
        (top as i32 + MID as i32 - offset) as usize
    };
    let mut previous = None;
    for (column, point) in points.iter().take(SIGNAL_BAND_WIDTH).enumerate() {
        let y = row(*point);
        let (from, to) = match previous {
            Some(last) if y > last => (last + 1, y),
            Some(last) if y < last => (y, last - 1),
            _ => (y, y),
        };
        fill_rect(frame, BAND_X + column, from, 1, to + 1 - from, color);
        previous = Some(y);
    }
}

fn draw_spectrum(frame: &mut [u8], top: usize, levels: &[u8], peaks: &[u8]) {
    let bar = rgb565(palette::BLUE);
    let peak = rgb565(palette::WHITE);
    let bottom = top + SIGNAL_BAND_HEIGHT;
    for (index, (&level, &held)) in levels.iter().zip(peaks).take(SPECTRUM_BARS).enumerate() {
        let x = BAND_X + index * 2;
        let height = band_rows(level, SIGNAL_BAND_HEIGHT);
        fill_rect(frame, x, bottom - height, 1, height, bar);
        let held = band_rows(held, SIGNAL_BAND_HEIGHT);
        if held > height {
            fill_rect(frame, x, bottom - held, 1, 1, peak);
        }
    }
}

fn draw_waterfall(frame: &mut [u8], top: usize, levels: &[u8]) {
    for (column, bins) in levels
        .as_chunks::<SIGNAL_BAND_HEIGHT>()
        .0
        .iter()
        .take(SIGNAL_BAND_WIDTH)
        .enumerate()
    {
        for (bin, &level) in bins.iter().enumerate() {
            if level != 0 {
                let y = top + SIGNAL_BAND_HEIGHT - 1 - bin;
                fill_rect(frame, BAND_X + column, y, 1, 1, ramp(level));
            }
        }
    }
}

fn band_rows(level: u8, rows: usize) -> usize {
    (usize::from(level) * rows + 127) / 255
}

fn ramp(level: u8) -> u16 {
    let upper = RAMP
        .iter()
        .position(|(stop, _)| level <= *stop)
        .unwrap_or(RAMP.len() - 1)
        .max(1);
    let ((low, from), (high, to)) = (RAMP[upper - 1], RAMP[upper]);
    let t = f32::from(level - low) / f32::from(high - low);
    let mix = |channel: usize| {
        (f32::from(from[channel]) + (f32::from(to[channel]) - f32::from(from[channel])) * t).round()
            as u8
    };
    rgb565([mix(0), mix(1), mix(2)])
}
