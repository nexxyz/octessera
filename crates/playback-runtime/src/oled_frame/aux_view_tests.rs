use super::model::{
    OledAuxCard, OledAuxGauge, OledAuxHint, OledAuxTurn, OledAuxView, OledBarStyle, OledHintLabel,
    OledHintSource,
};
use super::pixels::{rgb565, shade};
use super::render::render_oled_frame;
use super::tests::{base, pixel};
use super::{OledFont, OledPresentationInput};
use platform_core::palette;

const BLUE: u16 = 0x2D7F;

fn card(label: &str, value: &str, fraction: f32, style: OledBarStyle) -> OledAuxCard {
    OledAuxCard {
        turn: Some(OledAuxTurn {
            label: label.into(),
            path: None,
            gauge: Some(OledAuxGauge {
                value: value.into(),
                fraction,
                style,
            }),
        }),
        click: None,
        color: BLUE,
        turning: false,
    }
}

fn view(cards: [OledAuxCard; 3]) -> OledPresentationInput {
    let mut input = base();
    input.display.title = "L1: life".into();
    input.display.lines = Vec::new();
    input.display.rows = Vec::new();
    input.display.colors = Vec::new();
    input.display.bars = Vec::new();
    input.display.scroll = None;
    input.selected_row = None;
    input.display.aux_view = Some(OledAuxView {
        bank: "Auto".into(),
        cards,
    });
    let hint = OledHintLabel {
        text: "Count".into(),
        color: BLUE,
        source: OledHintSource::Auto,
    };
    input.aux_hints.normal = std::array::from_fn(|_| OledAuxHint {
        turn: Some(hint.clone()),
        click: None,
    });
    input
}

/// The top card's arc row 1 px above its centre line: lit pixels left of x=64, then right.
fn top_arc_row(frame: &[u8], color: u16) -> (usize, usize) {
    let y = 12 + 31 - 1;
    let lit =
        |range: std::ops::Range<usize>| range.filter(|x| pixel(frame, *x, y) == color).count();
    (lit(40..64), lit(64..90))
}

#[test]
fn the_aux_view_draws_three_cards_and_hides_the_footer_hints() {
    let empty = OledAuxCard::default();
    let frame = render_oled_frame(&view([
        card("Step", "1/16", 0.5, OledBarStyle::Fill),
        empty.clone(),
        empty,
    ]));
    let footer_hints = (113..128)
        .flat_map(|y| (0..107).map(move |x| (x, y)))
        .filter(|(x, y)| pixel(&frame, *x, *y) != 0)
        .count();
    assert_eq!(footer_hints, 0);
    let unbound_dots = (70..112)
        .flat_map(|y| (0..64).map(move |x| (x, y)))
        .filter(|(x, y)| pixel(&frame, *x, *y) == shade(rgb565(palette::GRAY), 0.3))
        .count();
    assert!(unbound_dots > 20, "{unbound_dots}");
    let white = rgb565(palette::WHITE);
    assert!((36..44).any(|y| (50..80).any(|x| pixel(&frame, x, y) == white)));
}

#[test]
fn fill_arcs_light_up_to_the_value_and_markers_light_only_their_segment() {
    let track = shade(BLUE, 0.28);
    let full = render_oled_frame(&view([
        card("Mix", "100%", 1.0, OledBarStyle::Fill),
        OledAuxCard::default(),
        OledAuxCard::default(),
    ]));
    let (left, right) = top_arc_row(&full, BLUE);
    assert!(left > 0 && right > 0 && top_arc_row(&full, track) == (0, 0));

    let low = render_oled_frame(&view([
        card("Mix", "0%", 0.0, OledBarStyle::Fill),
        OledAuxCard::default(),
        OledAuxCard::default(),
    ]));
    assert_eq!(top_arc_row(&low, BLUE), (0, 0));

    let pan = render_oled_frame(&view([
        card("Pan Pos", "C", 0.5, OledBarStyle::Marker),
        OledAuxCard::default(),
        OledAuxCard::default(),
    ]));
    let lit: Vec<usize> = (40..90)
        .filter(|x| (20..30).any(|y| pixel(&pan, *x, y) == BLUE))
        .collect();
    assert!(lit.iter().all(|x| (56..72).contains(x)), "{lit:?}");
    assert!(!lit.is_empty());
}

#[test]
fn a_turning_card_lights_its_arc_white_and_boxes_its_value() {
    let mut turning = card("Cutoff", "156", 0.6, OledBarStyle::Fill);
    turning.turning = true;
    let frame = render_oled_frame(&view([
        turning,
        OledAuxCard::default(),
        OledAuxCard::default(),
    ]));
    assert!(top_arc_row(&frame, rgb565(palette::WHITE)).0 > 0);
    let boxed = (36..44)
        .flat_map(|y| (50..80).map(move |x| (x, y)))
        .filter(|(x, y)| pixel(&frame, *x, *y) == BLUE)
        .count();
    assert!(boxed > 40, "{boxed}");
}

#[test]
fn the_bank_sits_dim_in_the_upper_left_clear_of_the_longest_top_card_name() {
    let gray = shade(rgb565(palette::GRAY), 0.5);
    for font in [OledFont::Octessera, OledFont::Original] {
        for bank in ["Auto", "Man"] {
            let mut input = view([
                card("Spawn Interval", "4", 0.3, OledBarStyle::Fill),
                OledAuxCard::default(),
                OledAuxCard::default(),
            ]);
            input.display.main_font = font;
            input.display.small_font = font;
            input.display.aux_view.as_mut().unwrap().bank = bank.into();
            let frame = render_oled_frame(&input);
            let lit = |color: u16| -> Vec<(usize, usize)> {
                (9..30)
                    .flat_map(|y| (0..128).map(move |x| (x, y)))
                    .filter(|(x, y)| pixel(&frame, *x, *y) == color)
                    .collect()
            };
            let corner = lit(gray);
            let right = corner.iter().map(|(x, _)| *x).max().unwrap();
            assert_eq!(corner.iter().map(|(x, _)| *x).min(), Some(2), "{font:?}");
            assert!(corner.iter().all(|(_, y)| (12..18).contains(y)), "{font:?}");
            let left = lit(BLUE).iter().map(|(x, _)| *x).min().unwrap();
            assert!(right + 3 <= left, "{font:?} {bank}: {right} {left}");
        }
    }
}
