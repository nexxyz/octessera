use std::ops::Range;

use super::menu_rows::{
    draw_body, draw_scrollbar, edit_box_x, row_area, selection_bar_span, slot_top, BAR_HEIGHT,
    PITCH, SCROLLBAR_X,
};
use super::model::{OledBodyAnimation, OledBodyFrom, OledDisplayInput, OledPresentationInput};
use super::pixels::{fill_rect, shade};
use super::{OLED_FRAME_BYTES, OLED_WIDTH};

/// Body animations only touch the rows between the area rule and the footer.
const BODY_TOP: usize = 9;
const BODY_BOTTOM: usize = 113;
const ROW_BYTES: usize = OLED_WIDTH * 2;

/// Turns the end-state body already drawn in `frame` into the animation's in-between frame.
pub(super) fn draw_body_animation(
    frame: &mut [u8],
    input: &OledPresentationInput,
    animation: &OledBodyAnimation,
    brightness: f32,
) {
    let mut scratch = [0_u8; OLED_FRAME_BYTES];
    let display = &input.display;
    match (animation, input.selected_row) {
        (OledBodyAnimation::Select { from_row, moved }, Some(to_row)) => {
            draw_body(&mut scratch, display, Some(*from_row), false, brightness);
            compose_select(
                frame,
                &scratch,
                display,
                [*from_row, to_row],
                *moved,
                brightness,
            );
        }
        (OledBodyAnimation::Scroll { from, down, moved }, Some(slot)) => {
            let shift = Shift {
                down: *down,
                moved: *moved,
            };
            compose_scroll(frame, &mut scratch, display, from, slot, shift, brightness);
        }
        (
            OledBodyAnimation::Slide {
                from,
                forward,
                moved,
            },
            _,
        ) => {
            draw_body(
                &mut scratch,
                &from.display,
                from.selected_row,
                from.display.editing,
                brightness,
            );
            compose_slide(frame, &scratch, *forward, *moved);
        }
        (OledBodyAnimation::Edit, Some(row)) => {
            let Some(box_x) = edit_box_x(display, row) else {
                return;
            };
            draw_body(
                &mut scratch,
                display,
                Some(row),
                !display.editing,
                brightness,
            );
            let cut = OLED_WIDTH - (OLED_WIDTH - box_x) / 2;
            let other = if display.editing {
                0..cut
            } else {
                cut..OLED_WIDTH
            };
            let top = slot_top(row) - 1;
            for y in top..top + BAR_HEIGHT {
                copy_span(frame, y, &scratch, y, other.clone());
            }
        }
        _ => {}
    }
}

/// The bar spans 11 of the 13 px between the two slot tops; `rows` is `[from, to]`.
fn compose_select(
    frame: &mut [u8],
    from_selected: &[u8],
    display: &OledDisplayInput,
    rows: [usize; 2],
    moved: usize,
    brightness: f32,
) {
    let (bar_x, bar_right) = selection_bar_span(display);
    let [from_top, to_top] = rows.map(|row| slot_top(row) - 1);
    let bar_top = if to_top > from_top {
        from_top + moved
    } else {
        from_top - moved
    };
    let area = row_area(display, rows[1], brightness);
    let bar = bar_top..bar_top + BAR_HEIGHT;
    for y in from_top.min(to_top)..from_top.max(to_top) + BAR_HEIGHT {
        let inside = bar.contains(&y);
        let in_from = (from_top..from_top + BAR_HEIGHT).contains(&y);
        let in_to = (to_top..to_top + BAR_HEIGHT).contains(&y);
        if (in_from && inside) || (in_to && !inside) {
            copy_span(frame, y, from_selected, y, bar_x..bar_right + 1);
        } else if inside && !in_from && !in_to {
            fill_rect(frame, bar_x, y, bar_right + 1 - bar_x, 1, area);
        }
    }
}

#[derive(Clone, Copy)]
struct Shift {
    down: bool,
    moved: usize,
}

impl Shift {
    /// The row of the list that has moved to `y`: in the old body, or in the new one.
    fn source(self, y: usize) -> (bool, usize) {
        if self.down {
            let src = y + self.moved;
            if src <= BODY_BOTTOM {
                (true, src)
            } else {
                (false, src - PITCH)
            }
        } else if y >= BODY_TOP + self.moved {
            (true, y - self.moved)
        } else {
            (false, y + PITCH - self.moved)
        }
    }

    /// `y` moved against the scroll by `extra` more pixels.
    fn behind(self, y: usize, extra: usize) -> Option<usize> {
        if self.down {
            (y + self.moved).checked_sub(extra)
        } else {
            (y + extra).checked_sub(self.moved)
        }
    }
}

/// The list moves under the fixed bar: inside the bar's band rows show selected, outside
/// unselected; the scrollbar thumb follows the list.
fn compose_scroll(
    frame: &mut [u8],
    scratch: &mut [u8; OLED_FRAME_BYTES],
    display: &OledDisplayInput,
    from: &OledBodyFrom,
    slot: usize,
    shift: Shift,
    brightness: f32,
) {
    let (bar_x, _) = selection_bar_span(display);
    let band_top = slot_top(slot) - 1;
    let band = band_top..band_top + BAR_HEIGHT;
    let mut new_band = [0_u8; BAR_HEIGHT * ROW_BYTES];
    new_band.copy_from_slice(&frame[band_top * ROW_BYTES..band.end * ROW_BYTES]);
    let outside = |y: usize| {
        if band.contains(&y) {
            0..bar_x
        } else {
            0..SCROLLBAR_X
        }
    };
    for (old, body) in [(false, display), (true, &from.display)] {
        scratch.fill(0);
        draw_body(scratch, body, None, false, brightness);
        for y in BODY_TOP..=BODY_BOTTOM {
            let (from_old, src) = shift.source(y);
            if from_old == old {
                copy_span(frame, y, scratch.as_slice(), src, outside(y));
            }
        }
    }
    scratch.fill(0);
    draw_body(scratch, &from.display, Some(slot), false, brightness);
    let area = row_area(display, slot, brightness);
    for y in band.clone() {
        let lens = bar_x..SCROLLBAR_X;
        if let Some(src) = shift.behind(y, 0).filter(|src| band.contains(src)) {
            copy_span(frame, y, scratch.as_slice(), src, lens);
        } else if let Some(src) = shift.behind(y, PITCH).filter(|src| band.contains(src)) {
            copy_span(frame, y, &new_band, src - band_top, lens);
        } else {
            fill_rect(frame, bar_x, y, SCROLLBAR_X - bar_x, 1, area);
        }
    }
    fill_rect(
        frame,
        SCROLLBAR_X,
        BODY_TOP,
        OLED_WIDTH - SCROLLBAR_X,
        BODY_BOTTOM + 1 - BODY_TOP,
        0,
    );
    if let (Some(scroll), Some(old)) = (&display.scroll, &from.display.scroll) {
        let start = old.offset * PITCH;
        let position = if shift.down {
            start + shift.moved
        } else {
            start.saturating_sub(shift.moved)
        };
        draw_scrollbar(
            frame,
            scroll,
            position,
            shade(display.title_color, brightness),
        );
    }
}

/// The new body (already in `frame`) slides in from the right when `forward`, else from the
/// left, pushing the old one out.
fn compose_slide(frame: &mut [u8], old: &[u8], forward: bool, moved: usize) {
    let keep = OLED_WIDTH - moved;
    for y in BODY_TOP..=BODY_BOTTOM {
        let row = &mut frame[y * ROW_BYTES..(y + 1) * ROW_BYTES];
        let old = &old[y * ROW_BYTES..(y + 1) * ROW_BYTES];
        if forward {
            row.copy_within(..moved * 2, keep * 2);
            row[..keep * 2].copy_from_slice(&old[moved * 2..]);
        } else {
            row.copy_within(keep * 2.., 0);
            row[moved * 2..].copy_from_slice(&old[..keep * 2]);
        }
    }
}

fn copy_span(frame: &mut [u8], y: usize, source: &[u8], source_y: usize, columns: Range<usize>) {
    let bytes = columns.start * 2..columns.end * 2;
    frame[y * ROW_BYTES..][bytes.clone()].copy_from_slice(&source[source_y * ROW_BYTES..][bytes]);
}
