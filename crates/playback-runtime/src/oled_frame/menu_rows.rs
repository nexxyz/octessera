use platform_core::palette;

use super::aux_cap::draw_aux_cap;
use super::font::{CLIP_MARKER, M5X7};
use super::footer::PLAY;
use super::menu_row_layout::{row_layout, RowValue, LIST_NAME_X, NAME_X};
use super::model::{OledBarInput, OledBarStyle, OledMenuRowInput, OledScrollInput};
use super::pixels::{dim, draw_mask, fill_rect, rgb565, rgb565_to_rgb, shade};
use super::{
    layout_card_body, OledDisplayInput, OledDisplayLayout, CARD_BODY_RECT, MENU_BODY_RECT,
};

pub(super) const BODY_ROWS: usize = MENU_BODY_RECT.rows();
const FIRST_SLOT_Y: usize = 11;
pub(super) const PITCH: usize = 13;
pub(super) const BAR_HEIGHT: usize = 11;
const CAP_X: usize = 0;
const BAR_X: usize = 9;
const LIST_BAR_X: usize = 0;
/// Text lines of overlays and cards, which have neither a gutter nor a pointer column.
const PLAIN_X: usize = 2;
const POINTER_X: usize = 1;
/// Values, markers and mini bars end here, whether or not the page scrolls.
pub(super) const VALUE_LAST: usize = 123;
/// The centre of the last digit cell, shared by the `>` and `!` markers.
pub(super) const MARKER_CENTRE_X: usize = 121;
pub(super) const SCROLLBAR_X: usize = 126;
const SCROLL_TOP: usize = FIRST_SLOT_Y - 1;
const SCROLL_HEIGHT: usize = (BODY_ROWS - 1) * PITCH + BAR_HEIGHT;
pub(super) const MINI_WIDTH: usize = 30;
pub(super) const PREVIEW_WIDTH: usize = 3;
const MINI_HEIGHT: usize = 5;

/// The top of body slot `index`; its selection bar spans `top - 1 ..= top + 9` and the
/// m5x7 capitals `top + 1 ..= top + 7`.
pub(super) fn slot_top(index: usize) -> usize {
    FIRST_SLOT_Y + index * PITCH
}

/// The selection bar's last column: the scrollbar keeps two columns and a gap.
pub(super) fn selection_bar_right(scrolls: bool) -> usize {
    if scrolls {
        124
    } else {
        127
    }
}

#[derive(Clone, Copy)]
struct RowStyle {
    area: u16,
    selected: bool,
    editing: bool,
    gutter: bool,
    bar_right: usize,
}

impl RowStyle {
    fn text(self) -> u16 {
        if self.selected {
            0
        } else {
            self.area
        }
    }

    fn draw_selection(self, frame: &mut [u8], top: usize) {
        if self.selected {
            let bar_x = if self.gutter { BAR_X } else { LIST_BAR_X };
            fill_rect(
                frame,
                bar_x,
                top - 1,
                self.bar_right + 1 - bar_x,
                BAR_HEIGHT,
                self.area,
            );
        }
    }
}

/// Draws a body in its own layout with `selected_row` highlighted, `editing` showing the
/// selected row's E2 box.
pub(super) fn draw_body(
    frame: &mut [u8],
    display: &OledDisplayInput,
    selected_row: Option<usize>,
    editing: bool,
) {
    match display.body_layout {
        OledDisplayLayout::Rows | OledDisplayLayout::List => {
            draw_menu_body(frame, display, selected_row, editing)
        }
        OledDisplayLayout::Card => draw_card_body(frame, display, selected_row),
    }
}

/// Where a rows body draws its selection bar: its first and last column.
pub(super) fn selection_bar_span(display: &OledDisplayInput) -> (usize, usize) {
    let bar_x = if has_gutter(display) {
        BAR_X
    } else {
        LIST_BAR_X
    };
    (bar_x, selection_bar_right(page_scroll(display).is_some()))
}

/// The first column of `row`'s E2 box.
pub(super) fn edit_box_x(display: &OledDisplayInput, row: usize) -> Option<usize> {
    let parts = display.rows.get(row)?.as_ref()?;
    let bar = display.bars.get(row).and_then(Option::as_ref);
    Some(row_layout(parts, bar, has_gutter(display)).value_x - 2)
}

/// The area colour of body row `row`.
pub(super) fn row_area(display: &OledDisplayInput, row: usize) -> u16 {
    display
        .colors
        .get(row)
        .copied()
        .unwrap_or_else(|| rgb565(palette::WHITE))
}

pub(super) fn page_scroll(display: &OledDisplayInput) -> Option<&OledScrollInput> {
    display
        .scroll
        .as_ref()
        .filter(|scroll| scroll.total_rows > BODY_ROWS)
}

fn has_gutter(display: &OledDisplayInput) -> bool {
    display.body_layout == OledDisplayLayout::Rows && display.rows.iter().flatten().next().is_some()
}

fn draw_menu_body(
    frame: &mut [u8],
    display: &OledDisplayInput,
    selected_row: Option<usize>,
    editing: bool,
) {
    let scroll = page_scroll(display);
    let bar_right = selection_bar_right(scroll.is_some());
    let gutter = has_gutter(display);
    let plain_x = if gutter {
        NAME_X
    } else if display.body_layout == OledDisplayLayout::List {
        LIST_NAME_X
    } else {
        PLAIN_X
    };
    for (index, line) in display.lines.iter().enumerate().take(BODY_ROWS) {
        let top = slot_top(index);
        let selected = selected_row == Some(index);
        let style = RowStyle {
            area: row_area(display, index),
            selected,
            editing: selected && editing,
            gutter,
            bar_right,
        };
        match display.rows.get(index).and_then(Option::as_ref) {
            Some(row) => {
                let bar = display.bars.get(index).and_then(Option::as_ref);
                draw_menu_row(frame, top, row, bar, style);
            }
            None => draw_plain_row(frame, top, line, plain_x, style),
        }
    }
    if let Some(scroll) = scroll {
        let area = display.title_color;
        draw_scrollbar(frame, scroll, scroll.offset * PITCH, area);
    }
}

fn draw_card_body(frame: &mut [u8], display: &OledDisplayInput, selected_row: Option<usize>) {
    for (index, line) in layout_card_body(&display.lines, selected_row, CARD_BODY_RECT)
        .iter()
        .enumerate()
    {
        let style = RowStyle {
            area: row_area(display, line.source_index),
            selected: line.selected,
            editing: false,
            gutter: false,
            bar_right: selection_bar_right(false),
        };
        draw_plain_row(frame, slot_top(index), &line.text, PLAIN_X, style);
    }
}

fn draw_plain_row(frame: &mut [u8], top: usize, text: &str, x: usize, style: RowStyle) {
    style.draw_selection(frame, top);
    M5X7.draw_fitted(frame, text, VALUE_LAST + 1 - x, x, top + 1, style.text());
}

fn draw_menu_row(
    frame: &mut [u8],
    top: usize,
    row: &OledMenuRowInput,
    bar: Option<&OledBarInput>,
    style: RowStyle,
) {
    let layout = row_layout(row, bar, style.gutter);
    let glyph_top = top + 1;
    style.draw_selection(frame, top);
    M5X7.draw_fitted(
        frame,
        &layout.name,
        layout.name_room,
        layout.name_x,
        glyph_top,
        style.text(),
    );
    let mut value_color = if style.selected {
        0
    } else {
        shade(style.area, 0.7)
    };
    if style.editing {
        let box_x = layout.value_x - 2;
        fill_rect(
            frame,
            box_x,
            top - 1,
            style.bar_right + 1 - box_x,
            BAR_HEIGHT,
            0,
        );
        value_color = style.area;
    }
    match layout.value {
        RowValue::Empty => {}
        RowValue::Mini(bar) => {
            let (fg, track) = if style.editing {
                (style.area, 0)
            } else if style.selected {
                (0, style.area)
            } else {
                (shade(style.area, 0.7), 0)
            };
            draw_mini_bar(frame, layout.value_x, top + 2, bar, fg, track);
        }
        RowValue::Preview => {
            draw_mask(
                frame,
                &PLAY,
                PREVIEW_WIDTH,
                layout.value_x,
                top + 2,
                value_color,
            );
        }
        RowValue::Marker(marker) => {
            draw_marker(frame, marker, layout.value_x, glyph_top, value_color)
        }
        RowValue::Picked(text) => {
            let x = layout.name_x + M5X7.advance(&layout.name);
            let pen = M5X7.draw(frame, &text[..layout.value_end], x, glyph_top, value_color);
            if layout.value_clipped {
                M5X7.draw(frame, CLIP_MARKER, pen, glyph_top, value_color);
            }
            draw_marker(frame, '>', layout.value_x, glyph_top, value_color);
        }
        RowValue::Text(text) => {
            let pen = M5X7.draw(
                frame,
                &text[..layout.value_end],
                layout.value_x,
                glyph_top,
                value_color,
            );
            if layout.value_clipped {
                M5X7.draw(frame, CLIP_MARKER, pen, glyph_top, value_color);
            }
            if style.editing {
                if let Some(cursor) = row.text_cursor {
                    draw_text_cursor(frame, text, cursor, layout.value_x, top, style.area);
                }
            }
        }
    }
    if style.gutter {
        if let Some(cap) = row.aux_cap {
            draw_aux_cap(frame, CAP_X, top + 1, cap, style.area);
        }
    } else if row.active {
        draw_mask(
            frame,
            &PLAY,
            PREVIEW_WIDTH,
            POINTER_X,
            top + 2,
            style.text(),
        );
    }
}

fn draw_marker(frame: &mut [u8], marker: char, x: usize, glyph_top: usize, color: u16) {
    let text = if marker == '>' { ">" } else { "!" };
    let (first, _) = M5X7.ink_columns(marker);
    M5X7.draw(frame, text, x - first, glyph_top, color);
}

fn draw_text_cursor(
    frame: &mut [u8],
    text: &str,
    cursor: usize,
    value_x: usize,
    top: usize,
    area: u16,
) {
    let byte = (0..=cursor.min(text.len()))
        .rev()
        .find(|&index| text.is_char_boundary(index))
        .unwrap_or(0);
    let ch = text[byte..].chars().next().unwrap_or(' ');
    let x = value_x + M5X7.advance(&text[..byte]);
    let width = usize::from(M5X7.glyph(ch).advance);
    fill_rect(frame, x - 1, top, width + 1, BAR_HEIGHT - 2, area);
    let mut buffer = [0_u8; 4];
    M5X7.draw(frame, ch.encode_utf8(&mut buffer), x, top + 1, 0);
}

fn draw_mini_bar(frame: &mut [u8], x: usize, y: usize, bar: &OledBarInput, fg: u16, track: u16) {
    let fraction = bar.fraction.clamp(0.0, 1.0);
    let inner = MINI_WIDTH - 2;
    fill_rect(frame, x, y, MINI_WIDTH, MINI_HEIGHT, fg);
    fill_rect(frame, x + 1, y + 1, inner, MINI_HEIGHT - 2, track);
    match bar.style {
        OledBarStyle::Fill => {
            let filled = (inner as f32 * fraction).round() as usize;
            fill_rect(frame, x + 1, y + 1, filled, MINI_HEIGHT - 2, fg);
        }
        OledBarStyle::Marker => {
            fill_rect(frame, x + 1 + inner / 2, y + 2, 1, 1, fg);
            let position = x + 1 + ((inner - 1) as f32 * fraction).round() as usize;
            fill_rect(
                frame,
                (position - 1).max(x + 1),
                y + 1,
                2,
                MINI_HEIGHT - 2,
                fg,
            );
        }
    }
}

/// Draws the scrollbar with its thumb at `position`, the scroll offset in 13ths of a row.
pub(super) fn draw_scrollbar(
    frame: &mut [u8],
    scroll: &OledScrollInput,
    position: usize,
    area: u16,
) {
    let thumb = (BODY_ROWS * SCROLL_HEIGHT / scroll.total_rows).max(6);
    let max_offset = (scroll.total_rows - BODY_ROWS).max(1);
    let position = position.min(max_offset * PITCH);
    let y = SCROLL_TOP + position * (SCROLL_HEIGHT - thumb) / (max_offset * PITCH);
    let track = rgb565(dim(rgb565_to_rgb(area), 6));
    fill_rect(frame, SCROLLBAR_X, SCROLL_TOP, 2, SCROLL_HEIGHT, track);
    fill_rect(frame, SCROLLBAR_X, y, 2, thumb, area);
}
