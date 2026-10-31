use super::model::OledAuxCap;
use super::pixels::{draw_mask, fill_rect, shade};

const DISC: [u8; 7] = [
    0b0011100, 0b0111110, 0b1111111, 0b1111111, 0b1111111, 0b0111110, 0b0011100,
];

/// An Aux encoder cap: a dark disc with one pip per encoder number, and for the Shift layer
/// a bar below it. `y` is the top of the plain cap; the Shift cap starts one row higher so
/// cap, gap and bar stay centred on the same line.
pub(super) fn draw_aux_cap(frame: &mut [u8], x: usize, y: usize, cap: OledAuxCap, area: u16) {
    let y = if cap.shift { y - 1 } else { y };
    draw_mask(frame, &DISC, 7, x, y, shade(area, 0.28));
    let pips: &[usize] = match cap.encoder {
        1 => &[3],
        2 => &[2, 4],
        _ => &[1, 3, 5],
    };
    for column in pips {
        fill_rect(frame, x + column, y + 3, 1, 1, area);
    }
    if cap.shift {
        fill_rect(frame, x + 1, y + 8, 5, 1, shade(area, 0.55));
    }
}
