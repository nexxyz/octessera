use super::font::M5X7;
use super::model::*;
use super::pixels::shade;
use super::render::render_oled_frame;
use super::tests::{base, menu_row, pixel, AREA};
use super::OledDisplayLayout;

const TOP: usize = 11;

fn page(layout: OledDisplayLayout, rows: Vec<Option<OledMenuRowInput>>) -> OledPresentationInput {
    let mut input = base();
    input.display.scroll = None;
    input.display.body_layout = layout;
    input.display.lines = rows
        .iter()
        .map(|row| row.as_ref().unwrap().label.clone())
        .collect();
    input.display.colors = vec![AREA; rows.len()];
    input.display.bars = vec![None; rows.len()];
    input.display.rows = rows;
    input.selected_row = None;
    input
}

fn picker(label: &str, value: &str) -> Option<OledMenuRowInput> {
    menu_row(OledMenuRowKind::Picker, label, value, None)
}

fn active(mut row: Option<OledMenuRowInput>) -> Option<OledMenuRowInput> {
    row.as_mut().unwrap().active = true;
    row
}

fn columns_in(frame: &[u8], color: u16, xs: std::ops::Range<usize>) -> Vec<usize> {
    xs.filter(|x| (TOP..TOP + 9).any(|y| pixel(frame, *x, y) == color))
        .collect()
}

#[test]
fn a_picker_row_draws_its_value_after_the_colon_in_the_value_shade() {
    let frame = render_oled_frame(&page(OledDisplayLayout::Rows, vec![picker("Beh.", "life")]));
    let value_x = 12 + M5X7.advance("Beh.: ");
    let label = columns_in(&frame, AREA, 0..value_x);
    assert_eq!(label.first(), Some(&12));
    let value = columns_in(&frame, shade(AREA, 0.7), value_x..128);
    assert_eq!(value.first(), Some(&value_x));
    assert_eq!(
        value
            .iter()
            .rev()
            .take(3)
            .rev()
            .copied()
            .collect::<Vec<_>>(),
        [120, 121, 122]
    );
    assert!(value
        .iter()
        .all(|x| *x < value_x + M5X7.width("life") || (120..=122).contains(x)));
}

#[test]
fn a_long_picked_value_clips_three_columns_before_its_marker() {
    let frame = render_oled_frame(&page(
        OledDisplayLayout::Rows,
        vec![picker("Beh.", "reaction diffusion and then some")],
    ));
    let value = columns_in(&frame, shade(AREA, 0.7), 0..128);
    let text_end = value.iter().copied().filter(|x| *x < 120).max().unwrap();
    assert!((110..=116).contains(&text_end), "{text_end}");
    assert!((117..120).all(|x| !value.contains(&x)));
}

#[test]
fn a_selected_picker_row_is_black_on_the_bar() {
    let mut input = page(OledDisplayLayout::Rows, vec![picker("Trn", "Cutoff")]);
    input.selected_row = Some(0);
    let frame = render_oled_frame(&input);
    let value_x = 12 + M5X7.advance("Trn: ");
    assert!((TOP..TOP + 9).any(|y| pixel(&frame, value_x, y) == 0));
    assert!((TOP..TOP + 9).any(|y| pixel(&frame, 121, y) == 0));
}

#[test]
fn the_active_list_entry_has_a_pointer_left_of_its_name() {
    use OledMenuRowKind::{Entry, Group};
    let rows = vec![
        menu_row(Group, "[Human]", "", None),
        active(menu_row(Group, "[Cellular]", "", None)),
        active(menu_row(Entry, "life", "", None)),
    ];
    let mut input = page(OledDisplayLayout::List, rows);
    input.selected_row = Some(2);
    let frame = render_oled_frame(&input);
    let ink = |slot: usize, x: usize, color: u16| {
        let top = TOP + slot * 13;
        (top..top + 9)
            .filter(|y| pixel(&frame, x, *y) == color)
            .map(|y| y - top)
            .collect::<Vec<_>>()
    };
    assert_eq!(ink(1, 1, AREA), [2, 3, 4, 5, 6]);
    assert_eq!(ink(1, 2, AREA), [3, 4, 5]);
    assert_eq!(ink(1, 3, AREA), [4]);
    assert!(ink(0, 1, AREA).is_empty());
    assert_eq!(ink(2, 1, 0), [2, 3, 4, 5, 6]);
    assert_eq!(ink(2, 3, 0), [4]);
    assert!((4..6).all(|x| ink(1, x, AREA).is_empty()));
    assert!(!ink(1, 6, AREA).is_empty());
}

#[test]
fn rows_with_a_gutter_draw_no_pointer() {
    let frame = render_oled_frame(&page(
        OledDisplayLayout::Rows,
        vec![active(menu_row(OledMenuRowKind::Group, "Synth", "", None))],
    ));
    assert!((TOP..TOP + 9).all(|y| (0..9).all(|x| pixel(&frame, x, y) == 0)));
}
