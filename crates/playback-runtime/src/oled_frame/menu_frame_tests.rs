use super::font::M5X7;
use super::model::*;
use super::pixels::{dim, rgb565, rgb565_to_rgb, shade};
use super::render::{render_oled_frame, render_oled_frame_into};
use super::tests::{base, menu_row, pixel, AREA, GLYPH_GOLDEN_TEXTS};
use super::text::title_text_for_oled;
use super::{layout_card_body, OledDisplayLayout, CARD_BODY_RECT, OLED_FRAME_BYTES};
use platform_core::palette;

fn slot(index: usize) -> usize {
    11 + index * 13
}

fn lit_columns(
    frame: &[u8],
    rows: std::ops::Range<usize>,
    xs: std::ops::Range<usize>,
) -> Vec<usize> {
    xs.filter(|x| rows.clone().any(|y| pixel(frame, *x, y) != 0))
        .collect()
}

fn unscrolled(mut input: OledPresentationInput) -> OledPresentationInput {
    input.display.scroll = None;
    input
}

fn modal(lines: &[&str], layout: OledDisplayLayout) -> OledPresentationInput {
    let mut input = base();
    input.display.body_layout = layout;
    input.display.lines = lines.iter().map(|line| (*line).to_owned()).collect();
    input.display.colors = vec![palette::WHITE_RGB565; lines.len()];
    input.display.bars = vec![None; lines.len()];
    input.display.rows = vec![None; lines.len()];
    input.display.scroll = None;
    input.selected_row = Some(lines.len() - 1);
    input
}

#[test]
fn header_crumb_and_rule_use_the_area_colour_above_a_clear_row() {
    for expected in [palette::YELLOW_RGB565, palette::GREEN_RGB565] {
        for layout in [OledDisplayLayout::Rows, OledDisplayLayout::Card] {
            let mut input = base();
            input.display.title_color = expected;
            input.display.body_layout = layout;
            let frame = render_oled_frame(&input);
            let rule = shade(expected, 0.625);
            assert!((0..128).all(|x| pixel(&frame, x, 8) == rule));
            assert!((0..128).all(|x| pixel(&frame, x, 9) == 0));
            assert!((0..104).any(|x| (0..5).any(|y| pixel(&frame, x, y) == expected)));
            assert!((0..128).all(|x| (6..8).all(|y| pixel(&frame, x, y) == 0)));
        }
    }
}

#[test]
fn long_titles_clip_with_dots_before_the_header_icons() {
    let mut input = base();
    input.display.title = "A Page Name That Runs Far Past The Header".into();
    let frame = render_oled_frame(&input);
    assert!(!lit_columns(&frame, 0..6, 96..104).is_empty());
    assert!(lit_columns(&frame, 0..6, 104..120).is_empty());
}

#[test]
fn selection_bar_spans_the_capitals_and_stops_short_of_the_scrollbar() {
    let frame = render_oled_frame(&base());
    let top = slot(1);
    assert_eq!(pixel(&frame, 9, top - 1), AREA);
    assert_eq!(pixel(&frame, 9, top + 9), AREA);
    assert_eq!(pixel(&frame, 8, top + 3), 0);
    assert_eq!(pixel(&frame, 9, top - 2), 0);
    assert_eq!(pixel(&frame, 9, top + 10), 0);
    assert_eq!(pixel(&frame, 124, top + 3), AREA);
    assert_eq!(pixel(&frame, 125, top + 3), 0);

    let frame = render_oled_frame(&unscrolled(base()));
    assert_eq!(pixel(&frame, 127, top + 3), AREA);
    assert_eq!(pixel(&frame, 126, top + 3), AREA);

    let mut unselected = base();
    unselected.selected_row = None;
    let frame = render_oled_frame(&unselected);
    assert!((10..112).all(|y| pixel(&frame, 9, y) == 0));
}

#[test]
fn aux_caps_sit_on_the_capitals_with_pips_and_are_never_inverted() {
    let frame = render_oled_frame(&base());
    let disc = shade(AREA, 0.28);
    let top = slot(1);
    assert_eq!(pixel(&frame, 0, top + 3), disc);
    assert_eq!(pixel(&frame, 3, top + 1), disc);
    assert_eq!(pixel(&frame, 3, top + 4), AREA);
    assert_eq!(pixel(&frame, 0, top + 1), 0);
    let top = slot(3);
    for x in [1, 3, 5] {
        assert_eq!(pixel(&frame, x, top + 4), AREA, "pip {x}");
    }
    let top = slot(0);
    assert_eq!(pixel(&frame, 3, top), disc);
    assert_eq!(pixel(&frame, 2, top + 3), AREA);
    assert_eq!(pixel(&frame, 4, top + 3), AREA);
    assert_eq!(pixel(&frame, 3, top + 7), 0);
    assert!((1..6).all(|x| pixel(&frame, x, top + 8) == shade(AREA, 0.55)));
    assert!(lit_columns(&frame, slot(4)..slot(4) + 9, 0..8).is_empty());
}

#[test]
fn values_markers_and_minis_end_on_the_shared_right_column() {
    let mut input = unscrolled(base());
    input.selected_row = None;
    input.display.bars = vec![None; 8];
    input.display.rows[1].as_mut().unwrap().value = "808".into();
    let frame = render_oled_frame(&input);
    let ink = |row| lit_columns(&frame, slot(row) + 1..slot(row) + 10, 60..128);
    assert_eq!(ink(1).last(), Some(&123));
    let marker = |row| lit_columns(&frame, slot(row) + 1..slot(row) + 10, 100..128);
    assert_eq!(marker(5), [120, 121, 122]);
    assert_eq!(marker(6), [121]);
    assert!((slot(1) + 1..slot(1) + 8).any(|y| pixel(&frame, 123, y) == shade(AREA, 0.7)));

    let frame = render_oled_frame(&unscrolled(base()));
    assert_eq!(
        lit_columns(&frame, slot(2) + 2..slot(2) + 7, 60..128),
        (94..124).collect::<Vec<_>>()
    );
}

#[test]
fn mini_bars_fill_or_mark_their_fraction_in_each_row_state() {
    let frame = render_oled_frame(&base());
    let dimmed = shade(AREA, 0.7);
    let (top, mini) = (slot(1), slot(1) + 2);
    assert_eq!(pixel(&frame, 93, top), AREA);
    assert_eq!(pixel(&frame, 94, mini), 0);
    assert_eq!(pixel(&frame, 123, mini + 4), 0);
    assert_eq!(pixel(&frame, 100, mini + 1), 0);
    assert_eq!(pixel(&frame, 118, mini + 1), 0);
    assert_eq!(pixel(&frame, 119, mini + 1), AREA);

    let mini = slot(2) + 2;
    assert_eq!(pixel(&frame, 94, mini), dimmed);
    assert_eq!(pixel(&frame, 123, mini + 4), dimmed);
    assert_eq!(pixel(&frame, 95, mini + 1), 0);
    assert!((mini..mini + 5).all(|y| pixel(&frame, 93, y) == 0 && pixel(&frame, 124, y) == 0));

    let mini = slot(3) + 2;
    assert_eq!(pixel(&frame, 109, mini + 2), dimmed);
    assert_eq!(pixel(&frame, 108, mini + 1), dimmed);
    assert_eq!(pixel(&frame, 109, mini + 1), dimmed);
    assert_eq!(pixel(&frame, 107, mini + 1), 0);
    assert_eq!(pixel(&frame, 110, mini + 1), 0);
    assert_eq!(pixel(&frame, 102, mini + 2), 0);
}

#[test]
fn editing_box_runs_from_the_value_to_the_bar_end_with_and_without_a_scrollbar() {
    let top = slot(1);
    for (scrolls, bar_right) in [(true, 124), (false, 127)] {
        let mut input = base();
        input.display.editing = true;
        if !scrolls {
            input.display.scroll = None;
        }
        let frame = render_oled_frame(&input);
        assert_eq!(pixel(&frame, 91, top - 1), AREA, "scrolls {scrolls}");
        for y in [top - 1, top + 4, top + 9] {
            assert_eq!(pixel(&frame, 92, y), 0, "scrolls {scrolls} y={y}");
            assert_eq!(pixel(&frame, bar_right, y), 0, "scrolls {scrolls} y={y}");
        }
        assert_eq!(pixel(&frame, 94, top + 2), AREA);
        assert_eq!(pixel(&frame, 100, top + 3), AREA);
        assert_eq!(pixel(&frame, 120, top + 3), 0);
        assert_eq!(pixel(&frame, 126, top + 4), if scrolls { AREA } else { 0 });
    }
}

#[test]
fn text_editing_inverts_the_cursor_character_inside_the_box() {
    let mut input = unscrolled(base());
    input.selected_row = Some(7);
    input.display.editing = true;
    input.display.rows[7].as_mut().unwrap().text_cursor = Some(2);
    let frame = render_oled_frame(&input);
    let top = slot(7);
    let value_x = 124 - (M5X7.width("Synth") + usize::from(M5X7.glyph(' ').advance));
    let cursor_x = value_x + M5X7.advance("Sy");
    assert_eq!(pixel(&frame, value_x - 2, top + 4), 0);
    assert_eq!(pixel(&frame, cursor_x - 1, top), AREA);
    assert!((cursor_x..cursor_x + 5).any(|x| (top + 1..top + 8).any(|y| pixel(&frame, x, y) == 0)));
    assert!(
        (value_x..cursor_x - 1).any(|x| (top + 1..top + 8).any(|y| pixel(&frame, x, y) == AREA))
    );
}

#[test]
fn long_values_clip_with_dots_and_keep_the_label_whole() {
    let mut input = unscrolled(base());
    input.selected_row = None;
    input.display.rows[0].as_mut().unwrap().value = "an option name far wider than its room".into();
    let frame = render_oled_frame(&input);
    let ink = lit_columns(&frame, slot(0) + 1..slot(0) + 10, 12..128);
    assert_eq!(ink.last(), Some(&123));
    let label_end = 12 + M5X7.width("Type");
    assert!(ink.iter().all(|x| *x < label_end || *x >= label_end + 4));
}

#[test]
fn browser_lists_have_no_gutter() {
    use OledMenuRowKind::{Action, Entry};
    let mut input = unscrolled(base());
    input.display.rows = vec![
        menu_row(Entry, "..", "", None),
        menu_row(Entry, "kick_01.wav", "", None),
        menu_row(Action, "Set favourite", "", None),
    ];
    input.display.lines = vec!["..".into(), "kick_01.wav".into(), "Set favourite".into()];
    input.display.bars = vec![None; 3];
    input.selected_row = Some(1);
    let frame = render_oled_frame(&input);
    assert_eq!(pixel(&frame, 0, slot(1) - 1), AREA);
    assert_eq!(pixel(&frame, 127, slot(1) - 1), AREA);
    assert_eq!(
        lit_columns(&frame, slot(2)..slot(2) + 9, 0..12).first(),
        Some(&2)
    );
    assert_eq!(lit_columns(&frame, slot(2)..slot(2) + 9, 100..128), [121]);
}

#[test]
fn a_sounding_preview_marks_its_file_row_with_a_play_triangle() {
    use OledMenuRowKind::Entry;
    let name = "a_long_sample_file_name_that_clips.wav";
    let render = |preview: bool| {
        let mut input = unscrolled(base());
        let mut row = menu_row(Entry, name, "", None);
        row.as_mut().unwrap().preview = preview;
        input.display.rows = vec![menu_row(Entry, "..", "", None), row];
        input.display.lines = vec!["..".into(), name.into()];
        input.display.bars = vec![None; 2];
        input.selected_row = Some(1);
        render_oled_frame(&input)
    };
    let top = slot(1);
    let ink = |frame: &[u8], x: usize| {
        (top..top + 9)
            .filter(|y| pixel(frame, x, *y) == 0)
            .collect::<Vec<_>>()
    };

    let frame = render(true);
    assert_eq!(ink(&frame, 121), (top + 2..top + 7).collect::<Vec<_>>());
    assert_eq!(ink(&frame, 122), (top + 3..top + 6).collect::<Vec<_>>());
    assert_eq!(ink(&frame, 123), [top + 4]);
    assert!((118..121).all(|x| ink(&frame, x).is_empty()));
    assert!((2..118).any(|x| !ink(&frame, x).is_empty()));

    let frame = render(false);
    assert!((121..124).all(|x| ink(&frame, x) != ink(&render(true), x)));
}

#[test]
fn scrollbar_thumb_tracks_offset_and_hides_without_overflow() {
    let track = rgb565(dim(rgb565_to_rgb(AREA), 6));
    for (offset, total, thumb_rows) in [(0, 12, 10..78), (2, 12, 27..95), (4, 12, 44..112)] {
        let mut input = base();
        input.display.scroll = Some(OledScrollInput {
            offset,
            total_rows: total,
            visible_rows: 8,
        });
        let frame = render_oled_frame(&input);
        for y in 10..112 {
            let expected = if thumb_rows.contains(&y) { AREA } else { track };
            assert_eq!(pixel(&frame, 126, y), expected, "offset {offset} y={y}");
            assert_eq!(pixel(&frame, 127, y), expected, "offset {offset} y={y}");
        }
        assert_eq!(pixel(&frame, 126, 9), 0);
        assert_eq!(pixel(&frame, 126, 112), 0);
    }
    for scroll in [
        None,
        Some(OledScrollInput::default()),
        Some(OledScrollInput {
            offset: 0,
            total_rows: 8,
            visible_rows: 8,
        }),
    ] {
        let mut input = base();
        input.selected_row = None;
        input.display.scroll = scroll;
        let frame = render_oled_frame(&input);
        assert!((10..112).all(|y| pixel(&frame, 126, y) == 0));
    }
}

#[test]
fn rows_use_their_colours_and_default_to_white() {
    let mut input = base();
    input.selected_row = None;
    input.display.colors = vec![palette::MAGENTA_RGB565, palette::GREEN_RGB565];
    let frame = render_oled_frame(&input);
    let has = |row: usize, color| {
        (slot(row)..slot(row) + 9).any(|y| (12..90).any(|x| pixel(&frame, x, y) == color))
    };
    assert!(has(0, palette::MAGENTA_RGB565));
    assert!(has(1, palette::GREEN_RGB565));
    assert!(has(2, palette::WHITE_RGB565));
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
    assert!((0..128).all(|x| pixel(&frame, x, 8) != AREA));

    input.display.off = true;
    assert!(render_oled_frame(&input).iter().all(|byte| *byte == 0));
}

#[test]
fn concise_midi_error_keeps_the_menu_frame_over_splash() {
    let mut input = modal(&["MIDI unavailable"], OledDisplayLayout::Rows);
    input.display.splash = OledSplash::Boot;
    input.display.title = "MIDI INPUTS".into();
    input.selected_row = None;
    input.runtime_error = Some(OledRuntimeErrorMetadata {
        domain: Some("midi".into()),
        operation: Some("midi_list_inputs".into()),
        ..Default::default()
    });
    let frame = render_oled_frame(&input);
    assert_eq!(pixel(&frame, 0, 8), shade(AREA, 0.625));
    assert!(!lit_columns(&frame, 0..5, 0..40).is_empty());
    assert_eq!(
        lit_columns(&frame, slot(0)..slot(0) + 9, 0..12).first(),
        Some(&2)
    );
}

#[test]
fn splash_overlays_toast_and_brightness_leaves_every_screen_at_full_intensity() {
    let mut splash = base();
    splash.display.splash = OledSplash::Boot;
    splash.display.toast = "Loading".into();
    let frame = render_oled_frame(&splash);
    assert!((105..112).any(|y| (12..120).any(|x| pixel(&frame, x, y) == palette::GRAY_RGB565)));

    let mut error = base();
    error.runtime_error = Some(OledRuntimeErrorMetadata::default());
    let mut toast = base();
    toast.display.toast = "Loading".into();
    for input in [base(), error, splash, toast] {
        for brightness in [1, 8, 12] {
            let mut dimmed = input.clone();
            dimmed.display_brightness = brightness;
            assert_eq!(render_oled_frame(&dimmed), render_oled_frame(&input));
        }
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
fn every_printable_glyph_has_ink_and_a_golden_frame() {
    let pinned = GLYPH_GOLDEN_TEXTS.concat();
    for code in 33..127_u8 {
        let ch = char::from(code);
        assert!(M5X7.glyph(ch).rows.iter().any(|row| *row != 0), "{ch:?}");
        assert!(pinned.contains(ch), "glyph {ch:?} has no golden frame");
    }
    assert_eq!(M5X7.glyph('é').rows, M5X7.glyph('?').rows);
    assert_eq!(M5X7.glyph('★').rows, M5X7.glyph('*').rows);
    assert!((b'0'..=b'9').all(|digit| M5X7.glyph(char::from(digit)).advance == 6));
    assert!(GLYPH_GOLDEN_TEXTS
        .iter()
        .all(|text| M5X7.width(text) <= 122));
}

#[test]
fn sparse_menus_render_without_metadata() {
    let mut input = base();
    input.display.bars.clear();
    input.display.colors.clear();
    input.display.rows.clear();
    input.display.scroll = None;
    assert_ne!(pixel(&render_oled_frame(&input), 0, slot(1)), 0);

    input.display.lines.clear();
    input.selected_row = None;
    let frame = render_oled_frame(&input);
    assert!((10..113).all(|y| (0..128).all(|x| pixel(&frame, x, y) == 0)));

    let empty_browser = modal(&["(empty)"], OledDisplayLayout::Rows);
    let frame = render_oled_frame(&empty_browser);
    assert_eq!(pixel(&frame, 0, slot(0)), palette::WHITE_RGB565);
    assert!((2..20).any(|x| (slot(0)..slot(0) + 9).any(|y| pixel(&frame, x, y) == 0)));
}

#[test]
fn modal_rows_and_setup_cards_highlight_their_final_action() {
    for lines in [
        &["Volume", "Tap to change", "> Close"][..],
        &["Discard changes?", "  Yes", "> No"],
        &["Copying", "12 files", "> Stop Transfer"],
    ] {
        let frame = render_oled_frame(&modal(lines, OledDisplayLayout::Rows));
        let top = slot(lines.len() - 1);
        assert_eq!(
            pixel(&frame, 0, top - 1),
            palette::WHITE_RGB565,
            "{lines:?}"
        );
        assert_eq!(
            pixel(&frame, 127, top - 1),
            palette::WHITE_RGB565,
            "{lines:?}"
        );
        assert!((10..112).all(|y| pixel(&frame, 126, y) == 0 || y + 1 >= top));
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
        &["Not available on", "desktop", "> Close"],
    ] {
        let input = modal(lines, OledDisplayLayout::Card);
        let layout = layout_card_body(&input.display.lines, input.selected_row, CARD_BODY_RECT);
        let row = layout.iter().position(|row| row.selected).unwrap();
        let frame = render_oled_frame(&input);
        assert_eq!(
            pixel(&frame, 0, slot(row) - 1),
            palette::WHITE_RGB565,
            "{lines:?}"
        );
        assert!(!lit_columns(&frame, slot(0)..slot(0) + 9, 0..128).is_empty());
    }
}
