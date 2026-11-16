use platform_core::palette;
use std::sync::LazyLock;

use super::font::PixelFont;
use super::model::{OledAuxCard, OledAuxView, OledBarStyle, OledDisplayInput};
use super::pixels::{fill_rect, rgb565, shade};
use super::OLED_WIDTH;

/// Each card's centre column, its first row and the width its path may use.
const CARDS: [(usize, usize, usize); 3] = [(64, 12, 120), (32, 64, 61), (96, 64, 61)];
/// A row of cards drops its arcs this far while any of them shows a path under its name.
const PATH_DROP: usize = 6;
const PATH_BELOW_TOP: usize = 9;
/// The arc's centre corner sits this far below the card's first row.
const ARC_BELOW_TOP: usize = 31;
const ARC_INNER: f32 = 17.5;
const ARC_OUTER: f32 = 19.5;
/// A marker lights this many degrees each side of its position.
const MARKER_HALF_DEGREES: f32 = 10.0;
const TRACK_LEVEL: f32 = 0.28;
const PATH_LEVEL: f32 = 0.7;

/// The pixels of a 2 px semicircle around a pixel corner, as offsets from that corner and the
/// angle from 12 o'clock in degrees, negative to the left.
static ARC: LazyLock<Vec<(i32, i32, f32)>> = LazyLock::new(|| {
    (-21..=0)
        .flat_map(|y| (-21..21).map(move |x| (x, y)))
        .filter_map(|(x, y)| {
            let (dx, dy) = (x as f32 + 0.5, y as f32 + 0.5);
            let distance = dx.hypot(dy);
            let angle = dx.atan2(-dy).to_degrees();
            (ARC_INNER < distance && distance <= ARC_OUTER && angle.abs() <= 90.0)
                .then_some((x, y, angle))
        })
        .collect()
});

pub(super) fn draw_aux_view(frame: &mut [u8], display: &OledDisplayInput, view: &OledAuxView) {
    let fonts = (display.main_font.main(), display.small_font.small());
    let has_path = |card: &OledAuxCard| card.turn.as_ref().is_some_and(|turn| turn.path.is_some());
    let bottom_drop = has_path(&view.cards[1]) || has_path(&view.cards[2]);
    let drops = [has_path(&view.cards[0]), bottom_drop, bottom_drop];
    for ((card, spot), drop) in view.cards.iter().zip(CARDS).zip(drops) {
        draw_card(frame, fonts, card, spot, if drop { PATH_DROP } else { 0 });
    }
}

fn draw_card(
    frame: &mut [u8],
    (main, small): (&PixelFont, &PixelFont),
    card: &OledAuxCard,
    (center, top, room): (usize, usize, usize),
    drop: usize,
) {
    let color = card.color;
    let centered = |font: &PixelFont, text: &str| center.saturating_sub(font.width(text) / 2);
    let head = match (&card.turn, &card.click) {
        (Some(turn), _) => turn.label.clone(),
        (None, Some(click)) => format!("{click}!"),
        (None, None) => "-".into(),
    };
    let unbound = card.turn.is_none() && card.click.is_none();
    let head_color = if unbound {
        shade(rgb565(palette::GRAY), 0.5)
    } else {
        color
    };
    main.draw(frame, &head, centered(main, &head), top, head_color);
    if let Some(path) = card.turn.as_ref().and_then(|turn| turn.path.as_deref()) {
        let width = small.width(path).min(room);
        let x = center.saturating_sub(width / 2);
        small.draw_fitted(
            frame,
            path,
            room,
            x,
            top + PATH_BELOW_TOP,
            shade(color, PATH_LEVEL),
        );
    }
    let arc_y = top + ARC_BELOW_TOP + drop;
    let gauge = card.turn.as_ref().and_then(|turn| turn.gauge.as_ref());
    match gauge {
        Some(gauge) => {
            let lit = if card.turning {
                rgb565(palette::WHITE)
            } else {
                color
            };
            draw_arc(frame, center, arc_y, |_, angle| {
                let position = -90.0 + 180.0 * gauge.fraction.clamp(0.0, 1.0);
                let on = match gauge.style {
                    OledBarStyle::Fill => gauge.fraction > 0.0 && angle <= position,
                    OledBarStyle::Marker => (angle - position).abs() <= MARKER_HALF_DEGREES,
                };
                Some(if on { lit } else { shade(color, TRACK_LEVEL) })
            });
            let value = &gauge.value;
            let (x, y) = (centered(main, value), arc_y - 7);
            if card.turning {
                fill_rect(frame, x - 2, y - 1, main.width(value) + 4, 9, color);
                main.draw(frame, value, x, y, rgb565(palette::BLACK));
            } else {
                main.draw(frame, value, x, y, rgb565(palette::WHITE));
            }
        }
        None => {
            let dots = shade(rgb565(palette::GRAY), 0.3);
            draw_arc(frame, center, arc_y, |(dx, dy), _| {
                ((dx + dy).rem_euclid(2) == 0).then_some(dots)
            });
        }
    }
    if let (Some(_), Some(click)) = (&card.turn, &card.click) {
        let click = format!("{click}!");
        main.draw(frame, &click, centered(main, &click), arc_y + 2, color);
    }
}

/// Paints the arc around the corner at (`center`, `corner_y`): each pixel in the colour `paint`
/// gives its offset and angle, or left alone.
fn draw_arc(
    frame: &mut [u8],
    center: usize,
    corner_y: usize,
    paint: impl Fn((i32, i32), f32) -> Option<u16>,
) {
    for &(dx, dy, angle) in ARC.iter() {
        let Some(color) = paint((dx, dy), angle) else {
            continue;
        };
        let (x, y) = (center as i32 + dx, corner_y as i32 + dy);
        if (0..OLED_WIDTH as i32).contains(&x) && y >= 0 {
            let index = (y as usize * OLED_WIDTH + x as usize) * 2;
            if let Some(pixel) = frame.get_mut(index..index + 2) {
                pixel.copy_from_slice(&color.to_be_bytes());
            }
        }
    }
}
