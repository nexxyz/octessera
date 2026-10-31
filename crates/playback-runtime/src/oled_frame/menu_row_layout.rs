//! Where a menu row's label, value and marker sit, and what clips.

use std::borrow::Cow;

use super::font::{CLIP_MARKER, M5X7};
use super::menu_rows::{MARKER_CENTRE_X, MINI_WIDTH, PREVIEW_WIDTH, VALUE_LAST};
use super::model::{OledBarInput, OledMenuRowInput, OledMenuRowKind};

pub(super) const NAME_X: usize = 12;
pub(super) const LIST_NAME_X: usize = 6;
/// A picked value ends here, three blank columns before the `>` marker.
const PICKED_LAST: usize = 116;
/// A label ends at least three blank columns before the value, mini bar or marker ink. A
/// text value clips to the label's end, but always keeps at least a mini bar's width.
const NAME_VALUE_GAP: usize = 4;

pub(super) enum RowValue<'a> {
    Empty,
    Text(&'a str),
    Mini(&'a OledBarInput),
    Marker(char),
    Preview,
    /// A picker's value, drawn after its label and clipped before its `>` marker.
    Picked(&'a str),
}

pub(super) struct RowLayout<'a> {
    pub(super) name: Cow<'a, str>,
    pub(super) name_x: usize,
    pub(super) name_room: usize,
    pub(super) value: RowValue<'a>,
    pub(super) value_end: usize,
    pub(super) value_clipped: bool,
    pub(super) value_x: usize,
}

pub(super) fn row_layout<'a>(
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
        OledMenuRowKind::Picker => (
            Cow::Owned(format!("{}: ", row.label)),
            RowValue::Picked(&row.value),
        ),
        OledMenuRowKind::Action => (Cow::Borrowed(&row.label), RowValue::Marker('!')),
        OledMenuRowKind::Entry if row.preview => (Cow::Borrowed(&row.label), RowValue::Preview),
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
    let marker_width = |marker| {
        let (first, last) = M5X7.ink_columns(marker);
        VALUE_LAST + 1 + (first + last) / 2 - first - MARKER_CENTRE_X
    };
    let (value_width, value_end, value_clipped) = match value {
        RowValue::Empty => (0, 0, false),
        RowValue::Mini(_) => (MINI_WIDTH, 0, false),
        RowValue::Preview => (PREVIEW_WIDTH, 0, false),
        RowValue::Marker(marker) => (marker_width(marker), 0, false),
        RowValue::Picked(text) => {
            let room = (PICKED_LAST + 1).saturating_sub(name_x + M5X7.advance(&name));
            let fit = M5X7.fit(text, room, CLIP_MARKER);
            (marker_width('>'), fit.end, fit.marker)
        }
        RowValue::Text(text) => {
            let room = (VALUE_LAST + 1)
                .saturating_sub(name_x + M5X7.width(&name) + NAME_VALUE_GAP)
                .max(MINI_WIDTH);
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
    let name_room = if matches!(value, RowValue::Picked(_)) {
        (PICKED_LAST + 1).saturating_sub(name_x + MINI_WIDTH)
    } else if value_width > 0 {
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

/// Whether a row's scrolling text, its label or a picker's value, is wider than its room, so
/// the selected row scrolls it.
pub(crate) fn menu_row_text_overflows(row: &OledMenuRowInput, has_bar: bool, gutter: bool) -> bool {
    let bar = has_bar.then(OledBarInput::default);
    let layout = row_layout(row, bar.as_ref(), gutter);
    match layout.value {
        RowValue::Picked(_) => layout.value_clipped,
        _ => M5X7.width(&layout.name) > layout.name_room,
    }
}

/// Whether a row clips its label or a value that does not scroll.
#[cfg(test)]
pub(crate) fn menu_row_clips(row: &OledMenuRowInput, has_bar: bool) -> bool {
    let bar = has_bar.then(OledBarInput::default);
    let layout = row_layout(row, bar.as_ref(), true);
    let picked = matches!(layout.value, RowValue::Picked(_));
    layout.value_clipped && !picked || M5X7.width(&layout.name) > layout.name_room
}
