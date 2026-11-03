use super::render::render_oled_frame;
use super::tests::{base, pixel, AREA};
use super::OledPresentationInput;

fn page(rows: usize) -> OledPresentationInput {
    let mut input = base();
    input.display.scroll = None;
    input.display.lines.truncate(rows);
    input.display.rows.truncate(rows);
    input.display.bars.truncate(rows);
    input.display.colors.truncate(rows);
    input.selected_row = Some(0);
    input
}

fn bar_rows(input: &OledPresentationInput) -> Vec<usize> {
    let frame = render_oled_frame(input);
    (9..114).filter(|y| pixel(&frame, 9, *y) == AREA).collect()
}

#[test]
fn a_five_row_page_sits_six_pixels_lower() {
    assert_eq!(bar_rows(&page(5)), (16..=26).collect::<Vec<_>>());
}

#[test]
fn a_full_page_keeps_its_first_bar_at_the_top() {
    assert_eq!(bar_rows(&page(8)), (10..=20).collect::<Vec<_>>());
}

#[test]
fn a_scrolling_page_keeps_its_first_bar_at_the_top() {
    let mut input = base();
    input.selected_row = Some(0);
    assert_eq!(bar_rows(&input), (10..=20).collect::<Vec<_>>());
}
