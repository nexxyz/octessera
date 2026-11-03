use std::sync::Arc;

use super::model::*;
use super::render::render_oled_frame;
use super::tests::{base, pixel, AREA};

const BODY: std::ops::RangeInclusive<usize> = 9..=113;

fn slot(index: usize) -> usize {
    11 + index * 13
}

fn first_lit(frame: &[u8], row: usize, xs: std::ops::Range<usize>) -> Option<usize> {
    xs.into_iter()
        .find(|x| (slot(row)..slot(row) + 9).any(|y| pixel(frame, *x, y) != 0))
}

fn capped() -> OledPresentationInput {
    let mut input = base();
    input.display.scroll = None;
    input.selected_row = Some(0);
    input
}

fn capless() -> OledPresentationInput {
    let mut input = capped();
    for row in input.display.rows.iter_mut().flatten() {
        row.aux_cap = None;
    }
    input
}

#[test]
fn a_page_without_caps_starts_its_bar_at_zero_and_its_names_at_six() {
    let frame = render_oled_frame(&capless());
    assert!((0..=127).all(|x| pixel(&frame, x, slot(0) - 1) == AREA));
    for row in 1..8 {
        assert_eq!(first_lit(&frame, row, 0..20), Some(6), "row {row}");
    }
}

#[test]
fn a_page_with_a_cap_keeps_the_gutter() {
    let frame = render_oled_frame(&capped());
    assert!((0..9).all(|x| pixel(&frame, x, slot(0) - 1) == 0));
    assert!((9..=127).all(|x| pixel(&frame, x, slot(0) - 1) == AREA));
    assert_eq!(first_lit(&frame, 2, 0..20), Some(0));
    for row in 1..8 {
        assert_eq!(first_lit(&frame, row, 7..20), Some(12), "row {row}");
    }
}

#[test]
fn a_slide_between_a_capped_and_a_capless_page_keeps_each_side_at_its_own_inset() {
    let (capped, capless) = (capped(), capless());
    let (capped_frame, capless_frame) = (render_oled_frame(&capped), render_oled_frame(&capless));
    let slide = |to: &OledPresentationInput, from: &OledPresentationInput, forward| {
        let mut input = to.clone();
        input.animation.body = Some(OledBodyAnimation::Slide {
            from: Arc::new(OledBodyFrom {
                display: from.display.clone(),
                selected_row: from.selected_row,
            }),
            forward,
            moved: 40,
        });
        render_oled_frame(&input)
    };
    let enter = slide(&capless, &capped, true);
    let back = slide(&capped, &capless, false);
    for y in BODY {
        for x in 0..128 {
            let expected_enter = if x < 88 {
                pixel(&capped_frame, x + 40, y)
            } else {
                pixel(&capless_frame, x - 88, y)
            };
            assert_eq!(pixel(&enter, x, y), expected_enter, "enter {x},{y}");
            let expected_back = if x < 40 {
                pixel(&capped_frame, x + 88, y)
            } else {
                pixel(&capless_frame, x - 40, y)
            };
            assert_eq!(pixel(&back, x, y), expected_back, "back {x},{y}");
        }
    }
    assert_ne!(capped_frame, capless_frame);
}
