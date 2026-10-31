use std::borrow::Cow;

use platform_core::palette;

use super::aux_cap::draw_aux_cap;
use super::font::{CLIP_MARKER, M5X7};
use super::model::{
    OledBarInput, OledBarStyle, OledMenuRowInput, OledMenuRowKind, OledScrollInput,
};
use super::pixels::{dim, fill_rect, rgb565, rgb565_to_rgb, scale, shade};
use super::{layout_card_body, layout_rows, OledPresentationInput, CARD_BODY_RECT, MENU_BODY_RECT};

pub(super) const BODY_ROWS: usize = MENU_BODY_RECT.rows();
const FIRST_SLOT_Y: usize = 11;
const PITCH: usize = 13;
const BAR_HEIGHT: usize = 11;
const CAP_X: usize = 0;
const BAR_X: usize = 9;
const NAME_X: usize = 12;
const LIST_BAR_X: usize = 0;
const LIST_NAME_X: usize = 2;
/// Values, markers and mini bars end here, whether or not the page scrolls.
pub(super) const VALUE_LAST: usize = 123;
/// The centre of the last digit cell, shared by the `>` and `!` markers.
pub(super) const MARKER_CENTRE_X: usize = 121;
const NAME_VALUE_GAP: usize = 4;
const SCROLLBAR_X: usize = 126;
const SCROLL_TOP: usize = FIRST_SLOT_Y - 1;
const SCROLL_HEIGHT: usize = (BODY_ROWS - 1) * PITCH + BAR_HEIGHT;
pub(super) const MINI_WIDTH: usize = 30;
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

enum RowValue<'a> {
    Empty,
    Text(&'a str),
    Mini(&'a OledBarInput),
    Marker(char),
}

struct RowLayout<'a> {
    name: Cow<'a, str>,
    name_x: usize,
    name_room: usize,
    value: RowValue<'a>,
    value_end: usize,
    value_clipped: bool,
    value_x: usize,
}

fn row_layout<'a>(
    row: &'a OledMenuRowInput,
    bar: Option<&'a OledBarInput>,
    gutter: bool,
) -> RowLayout<'a> {
    let name_x = if gutter { NAME_X } else { LIST_NAME_X };
    let (name, value): (Cow<str>, RowValue) = match row.kind {
        OledMenuRowKind::Group if row.value.is_empty() => {
            (Cow::Borrowed(&row.label), RowValue::Marker('>'))
        }
        OledMenuRowKind::Group => (
            Cow::Owned(format!("{}: {}", row.label, row.value)),
            RowValue::Marker('>'),
        ),
        OledMenuRowKind::Action => (Cow::Borrowed(&row.label), RowValue::Marker('!')),
        OledMenuRowKind::Entry => (Cow::Borrowed(&row.label), RowValue::Empty),
        OledMenuRowKind::Value => (
            Cow::Borrowed(&row.label),
            match bar {
                Some(bar) => RowValue::Mini(bar),
                None if row.value.is_empty() && row.text_cursor.is_none() => RowValue::Empty,
                None => RowValue::Text(&row.value),
            },
        ),
    };
    let (value_width, value_end, value_clipped) = match value {
        RowValue::Empty => (0, 0, false),
        RowValue::Mini(_) => (MINI_WIDTH, 0, false),
        RowValue::Marker(marker) => (M5X7.width(if marker == '>' { ">" } else { "!" }), 0, false),
        RowValue::Text(text) => {
            let room = (VALUE_LAST + 1).saturating_sub(name_x + M5X7.width(&name) + NAME_VALUE_GAP);
            let fit = M5X7.fit(text, room, CLIP_MARKER);
            let shown = M5X7.advance(&text[..fit.end])
                + if fit.marker {
                    M5X7.advance(CLIP_MARKER)
                } else {
                    0
                };
            let cursor = if row.text_cursor.is_some() {
                usize::from(M5X7.glyph(' ').advance)
            } else {
                0
            };
            (shown.saturating_sub(1) + cursor, fit.end, fit.marker)
        }
    };
    let value_x = VALUE_LAST + 1 - value_width;
    let name_room = if value_width > 0 {
        (value_x + 1).saturating_sub(NAME_VALUE_GAP + name_x)
    } else {
        VALUE_LAST + 1 - name_x
    };
    RowLayout {
        name,
        name_x,
        name_room,
        value,
        value_end,
        value_clipped,
        value_x,
    }
}

/// Whether a row's label is wider than its room, so the selected row scrolls it.
pub(crate) fn menu_row_label_overflows(row: &OledMenuRowInput, has_bar: bool) -> bool {
    let bar = has_bar.then(OledBarInput::default);
    let layout = row_layout(row, bar.as_ref(), row.kind != OledMenuRowKind::Entry);
    M5X7.width(&layout.name) > layout.name_room
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

pub(super) fn draw_menu_body(frame: &mut [u8], input: &OledPresentationInput, brightness: f32) {
    let display = &input.display;
    let white = rgb565(scale(palette::WHITE, brightness));
    let scroll = display
        .scroll
        .as_ref()
        .filter(|scroll| scroll.total_rows > BODY_ROWS);
    let bar_right = selection_bar_right(scroll.is_some());
    let mut menu_rows = display.rows.iter().flatten();
    let gutter = menu_rows.clone().next().is_some()
        && !menu_rows.any(|row| row.kind == OledMenuRowKind::Entry);
    for (index, line) in layout_rows(&display.lines, input.selected_row, MENU_BODY_RECT)
        .iter()
        .enumerate()
    {
        let top = slot_top(index);
        let style = RowStyle {
            area: display
                .colors
                .get(line.source_index)
                .map_or(white, |color| shade(*color, brightness)),
            selected: line.selected,
            editing: line.selected && display.editing,
            gutter,
            bar_right,
        };
        match display.rows.get(line.source_index).and_then(Option::as_ref) {
            Some(row) => {
                let bar = display.bars.get(line.source_index).and_then(Option::as_ref);
                draw_menu_row(frame, top, row, bar, style);
            }
            None => draw_plain_row(frame, top, &line.text, style),
        }
    }
    if let Some(scroll) = scroll {
        draw_scrollbar(frame, scroll, shade(display.title_color, brightness));
    }
}

pub(super) fn draw_card_body(frame: &mut [u8], input: &OledPresentationInput, brightness: f32) {
    let display = &input.display;
    let white = rgb565(scale(palette::WHITE, brightness));
    for (index, line) in layout_card_body(&display.lines, input.selected_row, CARD_BODY_RECT)
        .iter()
        .enumerate()
    {
        let style = RowStyle {
            area: display
                .colors
                .get(line.source_index)
                .map_or(white, |color| shade(*color, brightness)),
            selected: line.selected,
            editing: false,
            gutter: false,
            bar_right: selection_bar_right(false),
        };
        draw_plain_row(frame, slot_top(index), &line.text, style);
    }
}

fn draw_plain_row(frame: &mut [u8], top: usize, text: &str, style: RowStyle) {
    style.draw_selection(frame, top);
    let x = if style.gutter { NAME_X } else { LIST_NAME_X };
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
        RowValue::Marker(marker) => {
            let text = if marker == '>' { ">" } else { "!" };
            let (first, last) = M5X7.ink_columns(marker);
            let x = MARKER_CENTRE_X - (first + last) / 2;
            M5X7.draw(frame, text, x, glyph_top, value_color);
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
    }
}

fn draw_text_cursor(
    frame: &mut [u8],
    text: &str,
    cursor: usize,
    value_x: usize,
    top: usize,
    area: u16,
) {
    let (byte, ch) = text.char_indices().nth(cursor).unwrap_or((text.len(), ' '));
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

fn draw_scrollbar(frame: &mut [u8], scroll: &OledScrollInput, area: u16) {
    let thumb = (BODY_ROWS * SCROLL_HEIGHT / scroll.total_rows).max(6);
    let max_offset = (scroll.total_rows - BODY_ROWS).max(1);
    let y = SCROLL_TOP + scroll.offset.min(max_offset) * (SCROLL_HEIGHT - thumb) / max_offset;
    let track = rgb565(dim(rgb565_to_rgb(area), 6));
    fill_rect(frame, SCROLLBAR_X, SCROLL_TOP, 2, SCROLL_HEIGHT, track);
    fill_rect(frame, SCROLLBAR_X, y, 2, thumb, area);
}
