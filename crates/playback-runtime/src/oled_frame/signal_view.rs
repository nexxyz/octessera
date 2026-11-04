use platform_core::palette;

use super::menu_rows::{slot_top, BAR_HEIGHT, PITCH};
use super::model::{OledSignalArea, OledSignalView, OledSignalVisual};
use super::pixels::{dim, fill_rect, rgb565};

/// How far the rows after the home band drop below their slots: less than two row pitches.
pub(super) const SIGNAL_BAND_DROP: usize = 18;
const LEFT_X: usize = 2;
pub const SIGNAL_WIDTH: usize = 124;
/// Odd, so a stopped band's flat line has a middle row.
pub const SIGNAL_BAND_HEIGHT: usize = 27;
/// The free rows from below the bar above the band slot to the bar of the dropped row after it.
const BAND_SPACE: usize = 2 * PITCH - BAR_HEIGHT + SIGNAL_BAND_DROP;
/// How far the band's first row sits below its slot top, centred in [`BAND_SPACE`].
const BAND_TOP_BELOW_SLOT: usize = (BAND_SPACE - SIGNAL_BAND_HEIGHT) / 2 - (PITCH - BAR_HEIGHT + 1);
/// A Shape signal view's picture fills the body between the header rule and the footer.
const BODY_TOP: usize = 12;
pub const SIGNAL_BODY_HEIGHT: usize = 96;
/// 1 px bars with a 1 px gap.
pub const SPECTRUM_BARS: usize = SIGNAL_WIDTH / 2;
/// Waterfall levels climb through these colours.
const RAMP: [(u8, [u8; 3]); 5] = [
    (0, palette::BLACK),
    (110, [10, 40, 48]),
    (175, palette::BLUE),
    (225, palette::MAGENTA),
    (255, palette::YELLOW),
];

/// The rows a signal picture covers.
#[derive(Clone, Copy)]
struct Rows {
    top: usize,
    height: usize,
}

impl Rows {
    fn mid(self) -> usize {
        self.top + (self.height - 1) / 2
    }

    fn bottom(self) -> usize {
        self.top + self.height
    }
}

pub(super) fn draw_signal_view(frame: &mut [u8], view: &OledSignalView, body_top_offset: usize) {
    let rows = match view.area {
        OledSignalArea::Band { slot } => Rows {
            top: slot_top(body_top_offset, slot) + BAND_TOP_BELOW_SLOT,
            height: SIGNAL_BAND_HEIGHT,
        },
        OledSignalArea::Body => Rows {
            top: BODY_TOP,
            height: SIGNAL_BODY_HEIGHT,
        },
    };
    match &view.visual {
        OledSignalVisual::Flat => fill_rect(
            frame,
            LEFT_X,
            rows.mid(),
            SIGNAL_WIDTH,
            1,
            rgb565(dim(palette::GRAY, 5)),
        ),
        OledSignalVisual::Scope { points } => draw_scope(frame, rows, points),
        OledSignalVisual::Spectrum { levels, peaks } => draw_spectrum(frame, rows, levels, peaks),
        OledSignalVisual::Waterfall { levels } => draw_waterfall(frame, rows, levels),
    }
}

fn draw_scope(frame: &mut [u8], rows: Rows, points: &[i8]) {
    let color = rgb565(palette::BLUE);
    let half = ((rows.height - 1) / 2) as i32;
    let row = |point: i8| {
        let offset = (i32::from(point) * half + 63).div_euclid(127);
        (rows.mid() as i32 - offset) as usize
    };
    let mut previous = None;
    for (column, point) in points.iter().take(SIGNAL_WIDTH).enumerate() {
        let y = row(*point);
        let (from, to) = match previous {
            Some(last) if y > last => (last + 1, y),
            Some(last) if y < last => (y, last - 1),
            _ => (y, y),
        };
        fill_rect(frame, LEFT_X + column, from, 1, to + 1 - from, color);
        previous = Some(y);
    }
}

fn draw_spectrum(frame: &mut [u8], rows: Rows, levels: &[u8], peaks: &[u8]) {
    let bar = rgb565(palette::BLUE);
    let peak = rgb565(palette::WHITE);
    for (index, (&level, &held)) in levels.iter().zip(peaks).take(SPECTRUM_BARS).enumerate() {
        let x = LEFT_X + index * 2;
        let height = level_rows(level, rows.height);
        fill_rect(frame, x, rows.bottom() - height, 1, height, bar);
        let held = level_rows(held, rows.height);
        if held > height {
            fill_rect(frame, x, rows.bottom() - held, 1, 1, peak);
        }
    }
}

fn draw_waterfall(frame: &mut [u8], rows: Rows, levels: &[u8]) {
    for (column, bins) in levels
        .chunks_exact(rows.height)
        .take(SIGNAL_WIDTH)
        .enumerate()
    {
        for (bin, &level) in bins.iter().enumerate() {
            if level != 0 {
                let y = rows.bottom() - 1 - bin;
                fill_rect(frame, LEFT_X + column, y, 1, 1, ramp(level));
            }
        }
    }
}

fn level_rows(level: u8, rows: usize) -> usize {
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
