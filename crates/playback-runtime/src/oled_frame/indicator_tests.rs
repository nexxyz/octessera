use super::font::TOM_THUMB;
use super::footer::shortened_hint;
use super::header::cpu_meter_bars;
use super::model::*;
use super::pixels::{dim, rgb565, scale, shade};
use super::render::render_oled_frame;
use super::tests::{base, hint, pixel, AREA};
use platform_core::palette;

fn faded(rgb: [u8; 3], level: u8) -> u16 {
    rgb565(scale(
        rgb,
        1.0 / f32::from(1_u8 << (INDICATOR_FADE_LEVELS - level)),
    ))
}

fn lit(
    frame: &[u8],
    xs: std::ops::RangeInclusive<usize>,
    ys: std::ops::RangeInclusive<usize>,
) -> Vec<(usize, usize)> {
    ys.flat_map(|y| xs.clone().map(move |x| (x, y)))
        .filter(|(x, y)| pixel(frame, *x, *y) != 0)
        .collect()
}

fn with_hints(normal: [OledAuxHint; 3]) -> OledPresentationInput {
    let mut input = base();
    input.aux_hints = OledAuxHints {
        normal,
        ..Default::default()
    };
    input
}

fn turn(text: &str) -> OledAuxHint {
    OledAuxHint {
        turn: hint(text),
        click: None,
    }
}

#[test]
fn transport_icons_share_one_slot_and_follow_state_colours() {
    for (icon, flash, color, on, off) in [
        (
            OledTransportIcon::Stop,
            OledTransportFlash::Beat,
            palette::MAGENTA_RGB565,
            (116, 119),
            (115, 119),
        ),
        (
            OledTransportIcon::Pause,
            OledTransportFlash::Measure,
            palette::BLUE_RGB565,
            (120, 123),
            (118, 121),
        ),
        (
            OledTransportIcon::Play,
            OledTransportFlash::Beat,
            palette::YELLOW_RGB565,
            (119, 121),
            (119, 119),
        ),
        (
            OledTransportIcon::Play,
            OledTransportFlash::Measure,
            palette::GREEN_RGB565,
            (117, 123),
            (116, 121),
        ),
        (
            OledTransportIcon::Play,
            OledTransportFlash::None,
            palette::WHITE_RGB565,
            (118, 120),
            (120, 121),
        ),
    ] {
        let mut input = base();
        input.transport = OledTransportInput { icon, flash };
        let frame = render_oled_frame(&input);
        assert_eq!(pixel(&frame, on.0, on.1), color, "{on:?}");
        assert_eq!(pixel(&frame, off.0, off.1), 0, "{off:?}");
        assert!(lit(&frame, 114..=121, 113..=127)
            .iter()
            .all(|(x, y)| (116..=120).contains(x) && (119..=123).contains(y)));
    }
}

#[test]
fn event_diamond_fades_in_halving_brightness_steps() {
    let mut input = base();
    let mut levels = Vec::new();
    for level in (0..=INDICATOR_FADE_LEVELS).rev() {
        input.event_dot_level = level;
        let frame = render_oled_frame(&input);
        levels.push(pixel(&frame, 125, 121));
        assert_eq!(pixel(&frame, 123, 119), 0);
        assert_eq!(pixel(&frame, 125, 119), pixel(&frame, 123, 121));
    }
    assert_eq!(
        levels,
        [
            palette::WHITE_RGB565,
            faded(palette::WHITE, 3),
            faded(palette::WHITE, 2),
            faded(palette::WHITE, 1),
            0,
        ]
    );
}

#[test]
fn recording_disc_pulses_between_full_and_half_red() {
    let mut input = base();
    assert!(lit(&render_oled_frame(&input), 108..=114, 115..=127).is_empty());
    input.recording_active = true;
    let frame = render_oled_frame(&input);
    let red = rgb565(palette::BEHAVIOR_PRIMARY_RED);
    assert_eq!(pixel(&frame, 111, 121), red);
    assert_eq!(pixel(&frame, 109, 119), 0);
    assert_eq!(pixel(&frame, 109, 120), red);
    assert!(lit(&frame, 108..=114, 115..=127)
        .iter()
        .all(|(x, y)| (109..=113).contains(x) && (119..=123).contains(y)));
    input.recording_pulse_dim = true;
    assert_eq!(
        pixel(&render_oled_frame(&input), 111, 121),
        rgb565(scale(palette::BEHAVIOR_PRIMARY_RED, 0.5))
    );
}

#[test]
fn toast_is_a_black_box_over_the_hints_and_leaves_the_icons() {
    let mut input = base();
    input.recording_active = true;
    input.display.toast = "A deliberately long toast message for the OLED".into();
    let frame = render_oled_frame(&input);
    let toast = lit(&frame, 0..=107, 113..=127);
    assert!(toast.iter().any(|(x, _)| *x <= 6));
    assert!(toast.iter().any(|(x, _)| *x >= 100));
    assert!(toast.iter().all(|(x, y)| (4..=107).contains(x)
        && (117..=125).contains(y)
        && pixel(&frame, *x, *y) == palette::WHITE_RGB565));
    assert_eq!(
        pixel(&frame, 111, 121),
        rgb565(palette::BEHAVIOR_PRIMARY_RED)
    );
    assert_eq!(pixel(&frame, 118, 121), palette::YELLOW_RGB565);
    assert_eq!(pixel(&frame, 125, 121), palette::WHITE_RGB565);
}

#[test]
fn a_single_hint_label_is_centred_on_its_cap() {
    let frame = render_oled_frame(&with_hints([
        turn("Cutoff"),
        Default::default(),
        Default::default(),
    ]));
    let cap = lit(&frame, 0..=6, 113..=127);
    assert!(cap.iter().all(|(_, y)| (118..=124).contains(y)));
    assert_eq!(pixel(&frame, 3, 121), AREA);
    assert_eq!(pixel(&frame, 0, 121), shade(AREA, 0.28));
    let label = lit(&frame, 7..=107, 113..=127);
    assert_eq!(label.iter().map(|(x, _)| *x).min(), Some(9));
    assert!(label.iter().all(|(_, y)| (119..=124).contains(y)));
    assert!(label.iter().any(|(_, y)| *y == 119) && label.iter().any(|(_, y)| *y == 123));
}

#[test]
fn turn_and_click_stack_with_the_click_on_top() {
    let mut input = with_hints(Default::default());
    input.aux_hints.normal[0] = OledAuxHint {
        turn: hint("Cutoff"),
        click: Some(OledHintLabel {
            color: palette::GRAY_RGB565,
            ..hint("Map").unwrap()
        }),
    };
    let frame = render_oled_frame(&input);
    let click = lit(&frame, 9..=35, 113..=120);
    let turn = lit(&frame, 9..=35, 121..=127);
    assert!(click
        .iter()
        .all(|(x, y)| (115..=120).contains(y) && pixel(&frame, *x, *y) == palette::GRAY_RGB565));
    assert!(turn
        .iter()
        .all(|(x, y)| (123..=127).contains(y) && pixel(&frame, *x, *y) == AREA));
    let bang = 9 + TOM_THUMB.advance("Map");
    assert!((115..=119).any(|y| pixel(&frame, bang, y) == palette::GRAY_RGB565));
    assert!(!turn.is_empty());
}

#[test]
fn labels_borrow_only_from_an_empty_neighbour() {
    let long = "Interval Glider";
    let alone = render_oled_frame(&with_hints([
        turn(long),
        Default::default(),
        Default::default(),
    ]));
    let right = |frame: &[u8], from| {
        lit(frame, from..=107, 115..=127)
            .iter()
            .map(|(x, _)| *x)
            .max()
    };
    assert_eq!(right(&alone, 9), Some(8 + TOM_THUMB.width(long)));

    let crowded = render_oled_frame(&with_hints([turn(long), turn("Res"), Default::default()]));
    assert!(lit(&crowded, 9..=36, 115..=127)
        .iter()
        .all(|(x, _)| *x <= 35));
    assert!(lit(&crowded, 36..=36, 115..=127).is_empty());

    let gap = render_oled_frame(&with_hints([turn(long), Default::default(), turn("Env")]));
    assert!(right(&gap, 9).is_some_and(|x| x > 37));
    assert!(lit(&gap, 72..=73, 115..=127).is_empty());
}

#[test]
fn nothing_is_drawn_for_unmapped_slots() {
    let frame = render_oled_frame(&with_hints(Default::default()));
    assert!(lit(&frame, 0..=107, 113..=127).is_empty());
}

#[test]
fn shift_layer_shows_its_hints_under_a_barred_cap() {
    let mut input = with_hints([turn("Cutoff"), Default::default(), Default::default()]);
    input.aux_hints.shift[1] = turn("Key");
    input.aux_shift_layer = true;
    let frame = render_oled_frame(&input);
    assert!(lit(&frame, 0..=36, 113..=127).is_empty());
    assert!((38..=42).all(|x| pixel(&frame, x, 125) == shade(AREA, 0.55)));
    assert_eq!(pixel(&frame, 40, 117), shade(AREA, 0.28));
    assert!(!lit(&frame, 46..=73, 119..=123).is_empty());
}

#[test]
fn hint_labels_drop_inner_vowels_then_cut_to_their_room() {
    assert_eq!(shortened_hint("Cutoff", 40), None);
    let room = TOM_THUMB.width("Intrvl");
    assert_eq!(shortened_hint("Interval", room).as_deref(), Some("Intrvl"));
    assert_eq!(
        shortened_hint("Master Vol", TOM_THUMB.width("Mstr Vl")).as_deref(),
        Some("Mstr Vl")
    );
    let mut input = with_hints([turn("Res"), turn("Mix"), turn("Master Volume")]);
    input.aux_hints.normal[2].click = Some(OledHintLabel {
        text: "Record".into(),
        ..hint("").unwrap()
    });
    let frame = render_oled_frame(&input);
    let slot = lit(&frame, 83..=127, 113..=127);
    assert!(slot
        .iter()
        .filter(|(x, _)| *x < 108)
        .all(|(x, _)| *x <= 107));
    let bang = slot
        .iter()
        .filter(|(x, y)| *y <= 119 && *x < 108)
        .map(|(x, _)| *x)
        .max();
    assert!(bang.is_some_and(|x| (105..=107).contains(&x)));
}

#[test]
fn cpu_meter_lights_three_equal_bars_by_peak_load_against_the_cpu_warn_setting() {
    for (warn, peak, bars) in [
        (85, 0, 1),
        (85, 50, 1),
        (85, 55, 2),
        (85, 80, 2),
        (85, 85, 3),
        (85, 200, 3),
        (70, 40, 1),
        (70, 45, 2),
        (70, 70, 3),
        (95, 55, 1),
        (95, 60, 2),
        (95, 90, 2),
        (95, 95, 3),
    ] {
        assert_eq!(cpu_meter_bars(peak, warn), bars, "warn {warn} peak {peak}");
    }

    let unlit = rgb565(dim(palette::GRAY, 4));
    let red = rgb565(palette::BEHAVIOR_PRIMARY_RED);
    for (warn, peak, expected) in [
        (85, 20, [palette::GREEN_RGB565, unlit, unlit]),
        (
            85,
            60,
            [palette::YELLOW_RGB565, palette::YELLOW_RGB565, unlit],
        ),
        (85, 85, [red, red, red]),
    ] {
        let mut input = base();
        input.cpu_warn_percent = warn;
        input.metrics.cpu_load_peak_percent = peak;
        let frame = render_oled_frame(&input);
        for (bar, color) in expected.into_iter().enumerate() {
            let x = 120 + bar * 3;
            for y in 0..5 {
                assert_eq!(pixel(&frame, x, y), color, "warn {warn} peak {peak}");
                assert_eq!(pixel(&frame, x + 1, y), color, "warn {warn} peak {peak}");
            }
            assert_eq!(pixel(&frame, x, 5), 0);
        }
        assert!((0..5).all(|y| pixel(&frame, 122, y) == 0 && pixel(&frame, 125, y) == 0));
    }
}
