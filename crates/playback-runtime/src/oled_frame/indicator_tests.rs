use super::footer::cpu_meter_bars;
use super::model::*;
use super::pixels::{dim, rgb565, scale};
use super::render::render_oled_frame;
use super::tests::{base, pixel};
use platform_core::palette;

fn faded(rgb: [u8; 3], level: u8) -> u16 {
    rgb565(scale(
        rgb,
        1.0 / f32::from(1_u8 << (INDICATOR_FADE_LEVELS - level)),
    ))
}

fn meter_pixels(frame: &[u8]) -> [u16; 3] {
    [
        pixel(frame, 116, 11),
        pixel(frame, 119, 11),
        pixel(frame, 122, 11),
    ]
}

#[test]
fn transport_icon_colours_follow_state() {
    for (icon, flash, color) in [
        (
            OledTransportIcon::Stop,
            OledTransportFlash::Beat,
            palette::MAGENTA_RGB565,
        ),
        (
            OledTransportIcon::Pause,
            OledTransportFlash::Measure,
            palette::BLUE_RGB565,
        ),
        (
            OledTransportIcon::Play,
            OledTransportFlash::Beat,
            palette::YELLOW_RGB565,
        ),
        (
            OledTransportIcon::Play,
            OledTransportFlash::Measure,
            palette::GREEN_RGB565,
        ),
        (
            OledTransportIcon::Play,
            OledTransportFlash::None,
            palette::WHITE_RGB565,
        ),
    ] {
        let mut input = base();
        input.transport = OledTransportInput { icon, flash };
        let frame = render_oled_frame(&input);
        assert_eq!(pixel(&frame, 107, 118), color);
    }
}

#[test]
fn event_dot_fades_in_halving_brightness_steps() {
    let mut input = base();
    let mut event_levels = Vec::new();
    for level in (0..=INDICATOR_FADE_LEVELS).rev() {
        input.event_dot_level = level;
        event_levels.push(pixel(&render_oled_frame(&input), 119, 119));
    }
    assert_eq!(
        event_levels,
        [
            palette::WHITE_RGB565,
            faded(palette::WHITE, 3),
            faded(palette::WHITE, 2),
            faded(palette::WHITE, 1),
            0,
        ]
    );

    input.display_brightness = 50;
    input.event_dot_level = 3;
    assert_eq!(
        pixel(&render_oled_frame(&input), 119, 119),
        rgb565(scale(palette::WHITE, 0.25))
    );
}

#[test]
fn long_toasts_stay_left_of_the_transport_icon_and_event_dot() {
    let mut input = base();
    input.display.toast = "A deliberately long toast message for the OLED".into();
    input.save_flash = OledSaveFlash::Flash;
    let frame = render_oled_frame(&input);
    let toast_pixels = (114..128)
        .flat_map(|y| (0..128).map(move |x| (x, y)))
        .filter(|(x, y)| pixel(&frame, *x, *y) == palette::WHITE_RGB565 && *x < 107)
        .collect::<Vec<_>>();
    assert!(toast_pixels.iter().any(|(x, _)| *x <= 10));
    assert!(toast_pixels.iter().any(|(x, _)| *x >= 100));
    assert!(toast_pixels
        .iter()
        .all(|(x, y)| (4..=104).contains(x) && (118..125).contains(y)));
    assert!((105..107).all(|x| (114..128).all(|y| pixel(&frame, x, y) == 0)));
    assert_eq!(pixel(&frame, 107, 118), palette::YELLOW_RGB565);
    assert_eq!(pixel(&frame, 119, 119), palette::WHITE_RGB565);
    assert_eq!(pixel(&frame, 97, 4), palette::YELLOW_RGB565);
}

#[test]
fn status_icons_stay_hidden_until_high_cpu_or_save_flash() {
    let frame = render_oled_frame(&base());
    assert!((94..114).all(|x| (0..14).all(|y| pixel(&frame, x, y) == 0)));
    let mut saving = base();
    saving.save_flash = OledSaveFlash::Flash;
    assert_eq!(
        pixel(&render_oled_frame(&saving), 97, 4),
        palette::YELLOW_RGB565
    );
}

#[test]
fn cpu_meter_lights_bars_by_peak_load_against_the_cpu_warn_setting() {
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
        (70, 70, [red, red, red]),
        (
            95,
            85,
            [palette::YELLOW_RGB565, palette::YELLOW_RGB565, unlit],
        ),
    ] {
        let mut input = base();
        input.cpu_warn_percent = warn;
        input.metrics.cpu_load_peak_percent = peak;
        let frame = render_oled_frame(&input);
        assert_eq!(meter_pixels(&frame), expected, "warn {warn} peak {peak}");
        assert_eq!(pixel(&frame, 116, 8), 0);
        assert_ne!(pixel(&frame, 119, 7), 0);
        assert_eq!(pixel(&frame, 119, 6), 0);
        assert_ne!(pixel(&frame, 122, 5), 0);
        assert!((124..128).all(|x| (0..14).all(|y| pixel(&frame, x, y) == 0)));
    }
}

#[test]
fn header_indicators_keep_four_clear_rows_around_the_rule() {
    let mut input = base();
    input.display.title = "/B/L1/Behavior/Options".into();
    input.save_flash = OledSaveFlash::Flash;
    input.metrics.cpu_load_peak_percent = 100;
    let frame = render_oled_frame(&input);
    for y in [12, 13, 14, 15, 17, 18, 19, 20] {
        assert!((0..128).all(|x| pixel(&frame, x, y) == 0), "y={y}");
    }
    for x in [93, 97, 116] {
        assert!((4..12).any(|y| pixel(&frame, x, y) != 0), "x={x}");
    }
    for x in [94, 95, 96, 105, 106, 114, 115] {
        assert!((0..14).all(|y| pixel(&frame, x, y) == 0), "x={x}");
    }
}
