use super::model::{OledMenuRowKind, OledSignalBand, OledSignalVisual};
use super::pixels::{dim, rgb565};
use super::render::render_oled_frame;
use super::tests::{base, menu_row, pixel, AREA};
use super::{OledPresentationInput, SIGNAL_BAND_HEIGHT, SIGNAL_BAND_WIDTH, SPECTRUM_BARS};
use platform_core::palette;

/// The home page's rows start 6 px low; slot 4 is the blank line above System.
const BAND_TOP: usize = 11 + 6 + 4 * 13 - 1;
const BAND_X: usize = 2;

fn home(visual: Option<OledSignalVisual>) -> OledPresentationInput {
    let mut input = base();
    let names = ["Build", "Link", "Shape", "Play", "", "System"];
    input.display.title = "Octessera".into();
    input.display.lines = names.iter().map(|name| (*name).into()).collect();
    input.display.rows = names
        .iter()
        .map(|name| (!name.is_empty()).then(|| menu_row(OledMenuRowKind::Group, name, "", None))?)
        .collect();
    input.display.colors = vec![AREA; names.len()];
    input.display.bars = vec![None; names.len()];
    input.display.scroll = None;
    input.display.signal_band = visual.map(|visual| OledSignalBand { slot: 4, visual });
    input.selected_row = Some(5);
    input
}

fn bar_rows(input: &OledPresentationInput) -> Vec<usize> {
    let frame = render_oled_frame(input);
    (9..114).filter(|y| pixel(&frame, 0, *y) == AREA).collect()
}

fn band_pixels(frame: &[u8]) -> Vec<(usize, usize, u16)> {
    (BAND_TOP..BAND_TOP + SIGNAL_BAND_HEIGHT)
        .flat_map(|y| (0..128).map(move |x| (x, y)))
        .filter_map(|(x, y)| Some((x, y, pixel(frame, x, y))).filter(|(_, _, c)| *c != 0))
        .collect()
}

#[test]
fn the_band_drops_only_the_rows_after_it_by_less_than_two_rows() {
    let plain = bar_rows(&home(None));
    let banded = bar_rows(&home(Some(OledSignalVisual::Flat)));
    assert_eq!(plain, (81..=91).collect::<Vec<_>>());
    assert_eq!(banded, (97..=107).collect::<Vec<_>>());
    let mut first = home(Some(OledSignalVisual::Flat));
    first.selected_row = Some(0);
    assert_eq!(bar_rows(&first), (16..=26).collect::<Vec<_>>());
    assert!(banded[0] - plain[0] < 26);
    assert!(BAND_TOP + SIGNAL_BAND_HEIGHT < banded[0]);
    assert!(plain[0] - 2 * 13 + 10 < BAND_TOP);
}

#[test]
fn a_stopped_band_is_one_dim_line_across_the_band() {
    let frame = render_oled_frame(&home(Some(OledSignalVisual::Flat)));
    let line = rgb565(dim(palette::GRAY, 5));
    let mid = BAND_TOP + (SIGNAL_BAND_HEIGHT - 1) / 2;
    let expected: Vec<_> = (BAND_X..BAND_X + SIGNAL_BAND_WIDTH)
        .map(|x| (x, mid, line))
        .collect();
    assert_eq!(band_pixels(&frame), expected);
}

#[test]
fn scope_points_span_the_band_and_join_column_to_column() {
    let mut points = vec![0_i8; SIGNAL_BAND_WIDTH];
    points[10] = 127;
    points[11] = -127;
    let frame = render_oled_frame(&home(Some(OledSignalVisual::Scope { points })));
    let blue = rgb565(palette::BLUE);
    let mid = BAND_TOP + (SIGNAL_BAND_HEIGHT - 1) / 2;
    let bottom = BAND_TOP + SIGNAL_BAND_HEIGHT - 1;
    let column = |x: usize| -> Vec<usize> {
        (BAND_TOP..=bottom)
            .filter(|y| pixel(&frame, x, *y) == blue)
            .collect()
    };
    assert_eq!(column(BAND_X), vec![mid]);
    assert_eq!(column(BAND_X + 10), (BAND_TOP..mid).collect::<Vec<_>>());
    assert_eq!(
        column(BAND_X + 11),
        (BAND_TOP + 1..=bottom).collect::<Vec<_>>()
    );
    assert_eq!(column(BAND_X + 12), (mid..bottom).collect::<Vec<_>>());
    assert!(band_pixels(&frame)
        .iter()
        .all(|(x, _, _)| (BAND_X..BAND_X + SIGNAL_BAND_WIDTH).contains(x)));
}

#[test]
fn spectrum_bars_rise_from_the_band_floor_under_their_peaks() {
    let mut levels = vec![0_u8; SPECTRUM_BARS];
    let mut peaks = vec![0_u8; SPECTRUM_BARS];
    levels[3] = 255;
    levels[5] = 128;
    peaks[5] = 255;
    let frame = render_oled_frame(&home(Some(OledSignalVisual::Spectrum { levels, peaks })));
    let bottom = BAND_TOP + SIGNAL_BAND_HEIGHT - 1;
    let lit = |x: usize| -> Vec<usize> {
        (BAND_TOP..=bottom)
            .filter(|y| pixel(&frame, x, *y) != 0)
            .collect()
    };
    assert_eq!(lit(BAND_X + 6), (BAND_TOP..=bottom).collect::<Vec<_>>());
    assert!(lit(BAND_X + 7).is_empty());
    let half = (128 * SIGNAL_BAND_HEIGHT + 127) / 255;
    let mut expected = vec![BAND_TOP];
    expected.extend(bottom + 1 - half..=bottom);
    assert_eq!(lit(BAND_X + 10), expected);
    assert_eq!(pixel(&frame, BAND_X + 10, BAND_TOP), rgb565(palette::WHITE));
}

#[test]
fn waterfall_columns_run_oldest_to_newest_with_low_bins_at_the_bottom() {
    let mut levels = vec![0_u8; SIGNAL_BAND_WIDTH * SIGNAL_BAND_HEIGHT];
    let newest = SIGNAL_BAND_WIDTH - 1;
    levels[newest * SIGNAL_BAND_HEIGHT] = 255;
    levels[SIGNAL_BAND_HEIGHT - 1] = 150;
    let frame = render_oled_frame(&home(Some(OledSignalVisual::Waterfall { levels })));
    let bottom = BAND_TOP + SIGNAL_BAND_HEIGHT - 1;
    assert_eq!(
        band_pixels(&frame),
        vec![
            (BAND_X, BAND_TOP, rgb565(palette::BLUE)),
            (BAND_X + newest, bottom, rgb565(palette::YELLOW)),
        ]
    );
}
