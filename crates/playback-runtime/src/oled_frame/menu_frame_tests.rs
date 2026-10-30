use super::font::glyph_rows;
use super::model::*;
use super::pixels::{dim, rgb565, rgb565_to_rgb, scale};
use super::render::{render_oled_frame, render_oled_frame_into};
use super::tests::{base, pixel, GLYPH_GOLDEN_TEXTS};
use super::text::title_text_for_oled;
use super::{layout_card_body, OledDisplayLayout, CARD_BODY_RECT, OLED_FRAME_BYTES};
use platform_core::palette;

const DEFINED_GLYPHS: &str =
    "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz:.-*+/()_#@><[]%!?,\'\"=▶■●|";

fn green_track() -> u16 {
    rgb565(dim(rgb565_to_rgb(palette::GREEN_RGB565), 6))
}

fn with_bar(fraction: f32, style: OledBarStyle, selected_row: usize) -> Vec<u8> {
    let mut input = base();
    input.display.bars[1] = Some(OledBarInput { fraction, style });
    input.selected_row = Some(selected_row);
    render_oled_frame(&input)
}

fn row_has_color(frame: &[u8], row: usize, color: u16) -> bool {
    let top = 18 + row * 13;
    (top..top + 7).any(|y| (4..120).any(|x| pixel(frame, x, y) == color))
}

fn modal(lines: &[&str], layout: OledDisplayLayout) -> OledPresentationInput {
    let mut input = base();
    input.display.body_layout = layout;
    input.display.lines = lines.iter().map(|line| (*line).to_owned()).collect();
    input.display.colors = vec![palette::WHITE_RGB565; lines.len()];
    input.display.bars = vec![None; lines.len()];
    input.display.scroll = None;
    input.selected_row = Some(lines.len() - 1);
    input
}

#[test]
fn header_rule_spans_the_width_in_the_area_colour_between_title_and_body() {
    for (title, color, brightness) in [
        ("/Play", palette::YELLOW_RGB565, 100),
        ("/Build", palette::GREEN_RGB565, 50),
        ("MENU", palette::GRAY_RGB565, 100),
    ] {
        for layout in [OledDisplayLayout::Rows, OledDisplayLayout::Card] {
            let mut input = base();
            input.display.title = title.into();
            input.display.title_color = color;
            input.display.body_layout = layout;
            input.selected_row = Some(0);
            input.display_brightness = brightness;
            let frame = render_oled_frame(&input);
            let expected = rgb565(scale(rgb565_to_rgb(color), f32::from(brightness) / 100.0));
            assert!(
                (0..128).all(|x| pixel(&frame, x, 14) == expected),
                "{title}"
            );
            for y in [12, 13, 15, 16] {
                assert!((0..128).all(|x| pixel(&frame, x, y) == 0), "{title} y={y}");
            }
            assert!(
                (0..128).any(|x| pixel(&frame, x, 11) != 0),
                "{title} glyphs"
            );
            if layout == OledDisplayLayout::Rows {
                assert_ne!(pixel(&frame, 4, 17), 0, "{title} selection");
            }
        }
    }
}

#[test]
fn menu_frame_draws_title_selection_bar_scrollbar_transport_and_event_dot() {
    let frame = render_oled_frame(&base());
    assert_ne!(pixel(&frame, 5, 5), 0);
    assert_eq!(pixel(&frame, 4, 30), palette::GREEN_RGB565);
    assert_eq!(pixel(&frame, 87, 30), palette::BLACK_RGB565);
    assert_eq!(pixel(&frame, 88, 31), palette::GREEN_RGB565);
    assert_ne!(pixel(&frame, 125, 18), 0);
    assert_ne!(pixel(&frame, 102, 118), 0);
    assert_ne!(pixel(&frame, 119, 119), 0);

    let mut unselected = base();
    unselected.selected_row = None;
    let frame = render_oled_frame(&unselected);
    assert!((17..114).all(|y| pixel(&frame, 3, y) == 0));
}

#[test]
fn bars_render_empty_partial_full_marker_and_selected_contrast() {
    let empty = with_bar(0.0, OledBarStyle::Fill, 0);
    assert_eq!(pixel(&empty, 87, 30), palette::GREEN_RGB565);
    assert_eq!(pixel(&empty, 88, 31), green_track());

    let partial = with_bar(0.5, OledBarStyle::Fill, 0);
    assert_eq!(pixel(&partial, 88, 31), palette::GREEN_RGB565);
    assert_eq!(pixel(&partial, 105, 31), green_track());

    let full = with_bar(1.0, OledBarStyle::Fill, 0);
    assert_eq!(pixel(&full, 121, 31), palette::GREEN_RGB565);

    let marker = with_bar(0.5, OledBarStyle::Marker, 0);
    assert_eq!(pixel(&marker, 105, 32), palette::GREEN_RGB565);
    assert_eq!(pixel(&marker, 104, 32), green_track());

    let selected = with_bar(1.0, OledBarStyle::Fill, 1);
    assert_eq!(pixel(&selected, 87, 30), palette::BLACK_RGB565);
    assert_eq!(pixel(&selected, 121, 31), palette::GREEN_RGB565);
}

#[test]
fn scrollbar_thumb_tracks_offset_and_hides_without_overflow() {
    let thumb = palette::WHITE_RGB565;
    let track = rgb565(dim(palette::GRAY, 4));
    for (offset, total, thumb_rows) in [(0, 12, 18..69), (2, 12, 32..83), (5, 12, 55..106)] {
        let mut input = base();
        input.display.scroll = Some(OledScrollInput {
            offset,
            total_rows: total,
            visible_rows: 7,
        });
        let frame = render_oled_frame(&input);
        for y in 18..106 {
            let expected = if thumb_rows.contains(&y) {
                thumb
            } else {
                track
            };
            assert_eq!(pixel(&frame, 125, y), expected, "offset {offset} y={y}");
        }
    }
    for scroll in [
        None,
        Some(OledScrollInput::default()),
        Some(OledScrollInput {
            offset: 0,
            total_rows: 7,
            visible_rows: 7,
        }),
    ] {
        let mut input = base();
        input.display.scroll = scroll;
        let frame = render_oled_frame(&input);
        assert!((18..106).all(|y| pixel(&frame, 125, y) == 0));
    }
}

#[test]
fn rows_use_their_colours_scaled_by_brightness_and_default_to_white() {
    let colors = [
        palette::MAGENTA_RGB565,
        palette::GREEN_RGB565,
        palette::BLUE_RGB565,
        palette::YELLOW_RGB565,
        palette::GRAY_RGB565,
        palette::WHITE_RGB565,
    ];
    let mut input = base();
    input.selected_row = None;
    input.display.colors = colors.to_vec();
    let frame = render_oled_frame(&input);
    for (row, color) in colors.into_iter().enumerate() {
        assert!(row_has_color(&frame, row, color), "row {row}");
    }
    assert!(row_has_color(&frame, 6, palette::WHITE_RGB565));

    input.display.colors.clear();
    let frame = render_oled_frame(&input);
    assert!((0..7).all(|row| row_has_color(&frame, row, palette::WHITE_RGB565)));

    let mut dimmed = base();
    dimmed.display_brightness = 50;
    let frame = render_oled_frame(&dimmed);
    assert_eq!(
        pixel(&frame, 4, 30),
        rgb565(scale(rgb565_to_rgb(palette::GREEN_RGB565), 0.5))
    );
}

#[test]
fn transport_icon_and_event_dot_colours_follow_state() {
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
        assert_eq!(pixel(&frame, 101, 118), color);
    }
    let mut input = base();
    assert_eq!(
        pixel(&render_oled_frame(&input), 119, 119),
        palette::WHITE_RGB565
    );
    input.metrics.voice_steal = true;
    assert_eq!(
        pixel(&render_oled_frame(&input), 119, 119),
        palette::MAGENTA_RGB565
    );
    input.event_dot_on = false;
    assert_eq!(pixel(&render_oled_frame(&input), 119, 119), 0);
}

#[test]
fn toast_replaces_footer_and_long_toasts_stay_inside_their_rect() {
    let mut input = base();
    input.display.toast = "A deliberately long toast message for the OLED".into();
    input.save_flash = OledSaveFlash::Flash;
    let frame = render_oled_frame(&input);
    assert!((118..125).any(|y| (5..11).any(|x| pixel(&frame, x, y) != 0)));
    assert_eq!(pixel(&frame, 101, 118), 0);
    assert_eq!(pixel(&frame, 119, 119), 0);
    assert!((107..128).all(|x| (114..128).all(|y| pixel(&frame, x, y) == 0)));
    assert_eq!(pixel(&frame, 107, 5), palette::YELLOW_RGB565);
}

#[test]
fn status_icons_stay_hidden_until_high_cpu_or_save_flash() {
    let frame = render_oled_frame(&base());
    assert_eq!(pixel(&frame, 118, 6), 0);
    assert_eq!(pixel(&frame, 107, 5), 0);
    let mut saving = base();
    saving.save_flash = OledSaveFlash::Flash;
    assert_eq!(
        pixel(&render_oled_frame(&saving), 107, 5),
        palette::YELLOW_RGB565
    );
}

#[test]
fn off_beats_errors_and_splash_while_errors_beat_splash() {
    let error = OledRuntimeErrorMetadata {
        domain: Some("sample".into()),
        code: Some("not_found".into()),
        operation: Some("audio_command".into()),
        message: Some("sample not found".into()),
    };
    let mut input = base();
    input.display.splash = OledSplash::Boot;
    input.display.toast = "Saved".into();
    input.runtime_error = Some(error);
    let frame = render_oled_frame(&input);
    assert_eq!(pixel(&frame, 0, 0), rgb565(dim(palette::MAGENTA, 6)));
    assert_eq!(pixel(&frame, 10, 10), palette::MAGENTA_RGB565);
    assert_eq!(pixel(&frame, 119, 119), palette::BLACK_RGB565);
    assert!((0..128).all(|x| pixel(&frame, x, 14) != palette::BLUE_RGB565));

    input.display.off = true;
    assert!(render_oled_frame(&input).iter().all(|byte| *byte == 0));
}

#[test]
fn concise_midi_error_keeps_the_menu_frame_over_splash() {
    let mut input = base();
    input.display.splash = OledSplash::Boot;
    input.display.title = "MIDI INPUTS".into();
    input.display.lines = vec!["MIDI unavailable".into()];
    input.display.colors = vec![palette::WHITE_RGB565];
    input.display.bars = vec![None];
    input.selected_row = None;
    input.runtime_error = Some(OledRuntimeErrorMetadata {
        domain: Some("midi".into()),
        operation: Some("midi_list_inputs".into()),
        ..Default::default()
    });
    let frame = render_oled_frame(&input);
    assert_eq!(pixel(&frame, 0, 0), 0);
    assert_ne!(pixel(&frame, 5, 5), 0);
    assert_ne!(pixel(&frame, 6, 18), 0);
    assert_eq!(pixel(&frame, 0, 14), palette::BLUE_RGB565);
}

#[test]
fn splash_overlays_toast_and_zero_brightness_blanks_every_screen() {
    let mut splash = base();
    splash.display.splash = OledSplash::Boot;
    splash.display.toast = "Loading".into();
    let frame = render_oled_frame(&splash);
    assert!((105..112).any(|y| (12..120).any(|x| pixel(&frame, x, y) == palette::GRAY_RGB565)));

    let mut error = base();
    error.runtime_error = Some(OledRuntimeErrorMetadata::default());
    let mut toast = base();
    toast.display.toast = "Loading".into();
    for mut input in [base(), error, splash, toast] {
        input.display_brightness = 0;
        assert!(render_oled_frame(&input).iter().all(|byte| *byte == 0));
    }
}

#[test]
fn reused_buffers_are_cleared_between_splash_menu_and_off() {
    let mut input = base();
    let mut reused = vec![0xa5_u8; OLED_FRAME_BYTES];
    render_oled_frame_into(&input, &mut reused);
    assert_eq!(reused, render_oled_frame(&input));

    input.display.splash = OledSplash::Sleep;
    render_oled_frame_into(&input, &mut reused);
    assert!(reused.iter().any(|byte| *byte != 0));
    input.display.splash = OledSplash::None;
    render_oled_frame_into(&input, &mut reused);
    assert_eq!(reused, render_oled_frame(&input));

    input.display.off = true;
    render_oled_frame_into(&input, &mut reused);
    assert!(reused.iter().all(|byte| *byte == 0));
}

#[test]
fn short_root_titles_expand_to_area_names() {
    assert_eq!(title_text_for_oled("B"), "/Build");
    assert_eq!(title_text_for_oled("/B"), "/Build");
    assert_eq!(title_text_for_oled("/SYS"), "/System");
    assert_eq!(title_text_for_oled("/B/L1: life"), "/B/L1: life");
    let mut short = base();
    short.display.title = "B".into();
    let mut long = base();
    long.display.title = "/Build".into();
    assert_eq!(render_oled_frame(&short), render_oled_frame(&long));
}

#[test]
fn every_defined_glyph_is_drawn_and_pinned_by_a_golden_frame() {
    let pinned = GLYPH_GOLDEN_TEXTS.concat();
    for ch in DEFINED_GLYPHS.chars() {
        assert_ne!(glyph_rows(ch), [0; 7], "missing glyph {ch:?}");
        assert!(pinned.contains(ch), "glyph {ch:?} has no golden frame");
    }
    assert!(GLYPH_GOLDEN_TEXTS
        .iter()
        .all(|text| text.chars().count() <= 19));
}

#[test]
fn sparse_menus_render_without_metadata() {
    let mut input = base();
    input.display.bars.clear();
    input.display.colors.clear();
    input.display.scroll = None;
    assert_ne!(pixel(&render_oled_frame(&input), 4, 30), 0);

    input.display.lines.clear();
    input.selected_row = None;
    let frame = render_oled_frame(&input);
    assert!((17..114).all(|y| (0..128).all(|x| pixel(&frame, x, y) == 0)));

    let empty_browser = modal(&["(empty)"], OledDisplayLayout::Rows);
    assert!(row_has_color(
        &render_oled_frame(&empty_browser),
        0,
        palette::BLACK_RGB565
    ));
}

#[test]
fn modal_rows_and_setup_cards_highlight_their_final_action() {
    for lines in [
        &["Volume", "Tap to change", "> Close"][..],
        &["Discard changes?", "  Yes", "> No"],
        &["..", "[drums]", "kick.wav", "(empty)"],
        &["Copying", "12 files", "> Stop Transfer"],
        &["Version 0.7.5", "CPU 12%", "> Back"],
    ] {
        let frame = render_oled_frame(&modal(lines, OledDisplayLayout::Rows));
        let y = 18 + (lines.len() - 1) * 13;
        assert_eq!(pixel(&frame, 3, y - 1), palette::WHITE_RGB565, "{lines:?}");
        assert!((18..106).all(|y| pixel(&frame, 125, y) == 0));
    }
    for lines in [
        &["Starting hotspot...", "> Hide"][..],
        &[
            "Hotspot:",
            "Octessera Setup 318d",
            "Data 192.168.42.1",
            "Port 8081",
            "Code Ab12Cd",
            "Portal: 10 minutes",
            "> Hide",
        ],
        &[
            "Setup complete",
            "IP in Maintenance > Sys. Info",
            "No reboot needed",
            "> Close",
        ],
        &["Not available on", "desktop", "> Close"],
    ] {
        let input = modal(lines, OledDisplayLayout::Card);
        let layout = layout_card_body(&input.display.lines, input.selected_row, CARD_BODY_RECT);
        let row = layout.iter().position(|row| row.selected).unwrap();
        let frame = render_oled_frame(&input);
        let y = CARD_BODY_RECT.y + row * CARD_BODY_RECT.row_advance;
        assert_eq!(pixel(&frame, 3, y - 1), palette::WHITE_RGB565, "{lines:?}");
        assert!(row_has_color(&frame, 0, palette::WHITE_RGB565), "{lines:?}");
    }
}
