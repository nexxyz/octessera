use platform_core::palette;

use super::aux_cap::draw_aux_cap;
use super::font::{M5X7, TOM_THUMB};
use super::model::{
    OledAuxCap, OledAuxHint, OledHintLabel, OledTransportFlash, OledTransportIcon,
    INDICATOR_FADE_LEVELS,
};
use super::pixels::{draw_mask, fill_rect, rgb565, scale, shade};
use super::OledPresentationInput;

const ICON_Y: usize = 119;
const RECORDING_X: usize = 109;
const TRANSPORT_X: usize = 116;
const EVENT_DOT_X: usize = 123;
/// Hints and the toast end at or before this column, left of the recording disc.
pub(super) const HINT_RIGHT: usize = 107;
pub(super) const HINT_SLOT_X: [usize; 3] = [0, 37, 74];
const HINT_CAP_Y: usize = 118;
const HINT_LABEL_OFFSET: usize = 9;
const HINT_SINGLE_Y: usize = 119;
const HINT_CLICK_Y: usize = 115;
const HINT_TURN_Y: usize = 123;
const TOAST_Y: usize = 115;
const TOAST_TEXT_X: usize = 4;
const TOAST_TEXT_Y: usize = 117;
const PLAY: [u8; 5] = [0b100, 0b110, 0b111, 0b110, 0b100];
const DISC: [u8; 5] = [0b01110, 0b11111, 0b11111, 0b11111, 0b01110];
const DIAMOND: [u8; 5] = [0b00100, 0b01110, 0b11111, 0b01110, 0b00100];

pub(super) fn draw_footer(frame: &mut [u8], input: &OledPresentationInput, brightness: f32) {
    draw_transport_icon(frame, input, brightness);
    let level = input.event_dot_level.min(INDICATOR_FADE_LEVELS);
    if level > 0 {
        let fade = 1.0 / f32::from(1_u8 << (INDICATOR_FADE_LEVELS - level));
        let color = rgb565(scale(palette::WHITE, brightness * fade));
        draw_mask(frame, &DIAMOND, 5, EVENT_DOT_X, ICON_Y, color);
    }
    let hints = if input.aux_shift_layer {
        &input.aux_hints.shift
    } else {
        &input.aux_hints.normal
    };
    draw_hints(frame, hints, input.aux_shift_layer, brightness);
    if input.recording_active {
        let level = if input.recording_pulse_dim { 0.5 } else { 1.0 };
        let red = rgb565(scale(palette::BEHAVIOR_PRIMARY_RED, brightness * level));
        draw_mask(frame, &DISC, 5, RECORDING_X, ICON_Y, red);
    }
    if !input.display.toast.is_empty() {
        fill_rect(frame, 0, TOAST_Y, HINT_RIGHT + 1, 128 - TOAST_Y, 0);
        let white = rgb565(scale(palette::WHITE, brightness));
        M5X7.draw_fitted(
            frame,
            &input.display.toast,
            HINT_RIGHT + 1 - TOAST_TEXT_X,
            TOAST_TEXT_X,
            TOAST_TEXT_Y,
            white,
        );
    }
}

fn draw_transport_icon(frame: &mut [u8], input: &OledPresentationInput, brightness: f32) {
    let rgb = match (&input.transport.icon, &input.transport.flash) {
        (OledTransportIcon::Play, OledTransportFlash::Measure) => palette::GREEN,
        (OledTransportIcon::Play, OledTransportFlash::Beat) => palette::YELLOW,
        (OledTransportIcon::Stop, _) => palette::MAGENTA,
        (OledTransportIcon::Pause, _) => palette::BLUE,
        _ => palette::WHITE,
    };
    let color = rgb565(scale(rgb, brightness));
    match input.transport.icon {
        OledTransportIcon::Play => draw_mask(frame, &PLAY, 3, TRANSPORT_X + 1, ICON_Y, color),
        OledTransportIcon::Pause => {
            fill_rect(frame, TRANSPORT_X, ICON_Y, 2, 5, color);
            fill_rect(frame, TRANSPORT_X + 3, ICON_Y, 2, 5, color);
        }
        _ => fill_rect(frame, TRANSPORT_X, ICON_Y, 5, 5, color),
    }
}

/// Where one hint's labels go, click above turn when both exist: `(label, is click, top row)`.
pub(super) fn hint_label_rows(hint: &OledAuxHint) -> [Option<(&OledHintLabel, bool, usize)>; 2] {
    match (&hint.turn, &hint.click) {
        (Some(turn), Some(click)) => [
            Some((click, true, HINT_CLICK_Y)),
            Some((turn, false, HINT_TURN_Y)),
        ],
        (Some(turn), None) => [Some((turn, false, HINT_SINGLE_Y)), None],
        (None, Some(click)) => [Some((click, true, HINT_SINGLE_Y)), None],
        (None, None) => [None, None],
    }
}

/// The last column a slot's labels may use: up to the hint area's end, or 2 px before the
/// next slot that shows a hint; empty neighbours lend their space.
pub(super) fn hint_label_right(hints: &[OledAuxHint; 3], slot: usize) -> usize {
    hints
        .iter()
        .enumerate()
        .skip(slot + 1)
        .find(|(_, hint)| hint.turn.is_some() || hint.click.is_some())
        .map_or(HINT_RIGHT, |(next, _)| HINT_SLOT_X[next] - 2)
}

fn draw_hints(frame: &mut [u8], hints: &[OledAuxHint; 3], shift: bool, brightness: f32) {
    for (slot, hint) in hints.iter().enumerate() {
        let Some(cap_label) = hint.turn.as_ref().or(hint.click.as_ref()) else {
            continue;
        };
        let x = HINT_SLOT_X[slot];
        let cap = OledAuxCap {
            encoder: slot as u8 + 1,
            shift,
        };
        draw_aux_cap(
            frame,
            x,
            HINT_CAP_Y,
            cap,
            shade(cap_label.color, brightness),
        );
        let label_x = x + HINT_LABEL_OFFSET;
        let room = hint_label_right(hints, slot) + 1 - label_x;
        for (label, click, top) in hint_label_rows(hint).into_iter().flatten() {
            let color = shade(label.color, brightness);
            let suffix = if click { "!" } else { "" };
            let pen = draw_hint_label(frame, &label.text, suffix, label_x, top, room, color);
            TOM_THUMB.draw(frame, suffix, pen, top, color);
        }
    }
}

/// Draws as much of a hint label as `room` allows with `suffix` still after it: whole when
/// it fits, else with vowels inside words dropped from the right, then cut. Returns the pen.
fn draw_hint_label(
    frame: &mut [u8],
    text: &str,
    suffix: &str,
    x: usize,
    top: usize,
    room: usize,
    color: u16,
) -> usize {
    let room = room.saturating_sub(TOM_THUMB.advance(suffix));
    let shortened = shortened_hint(text, room);
    let label = shortened.as_deref().unwrap_or(text);
    let fit = TOM_THUMB.fit(label, room, "");
    TOM_THUMB.draw(frame, &label[..fit.end], x, top, color)
}

/// `text` with lower-case vowels inside words dropped from the right until it fits `room`
/// pixels; `None` when it already fits.
pub(super) fn shortened_hint(text: &str, room: usize) -> Option<String> {
    let mut width = TOM_THUMB.width(text);
    if width <= room {
        return None;
    }
    let mut chars = text.chars().collect::<Vec<_>>();
    let mut index = chars.len();
    while width > room && index > 1 {
        index -= 1;
        let inside_word = chars[index - 1].is_alphanumeric();
        if inside_word && matches!(chars[index], 'a' | 'e' | 'i' | 'o' | 'u') {
            width -= usize::from(TOM_THUMB.glyph(chars[index]).advance);
            chars.remove(index);
        }
    }
    Some(chars.into_iter().collect())
}
