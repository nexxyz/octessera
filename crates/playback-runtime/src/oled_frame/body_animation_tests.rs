use std::sync::Arc;

use super::model::*;
use super::render::{render_oled_frame, render_oled_frame_into};
use super::tests::{base, hint, menu_row, pixel, AREA};
use super::OLED_FRAME_BYTES;

const BODY: std::ops::RangeInclusive<usize> = 9..=113;

fn animated(mut input: OledPresentationInput, body: OledBodyAnimation) -> Vec<u8> {
    input.animation.body = Some(body);
    render_oled_frame(&input)
}

fn selected(row: usize) -> OledPresentationInput {
    let mut input = base();
    input.selected_row = Some(row);
    input
}

fn from(input: &OledPresentationInput) -> Arc<OledBodyFrom> {
    Arc::new(OledBodyFrom {
        display: input.display.clone(),
        selected_row: input.selected_row,
    })
}

/// The base Filter page scrolled one row further down: its rows move up by one.
fn scrolled_down() -> OledPresentationInput {
    let mut input = base();
    let display = &mut input.display;
    display.lines.remove(0);
    display.lines.push("Drive".into());
    display.rows.remove(0);
    display
        .rows
        .push(menu_row(OledMenuRowKind::Value, "Drive", "12", None));
    display.bars.remove(0);
    display.bars.push(None);
    display.scroll.as_mut().unwrap().offset += 1;
    input
}

fn unselected(input: &OledPresentationInput) -> Vec<u8> {
    let mut input = input.clone();
    input.selected_row = None;
    render_oled_frame(&input)
}

fn assert_rows_equal(frame: &[u8], reference: &[u8], rows: impl IntoIterator<Item = usize>) {
    for y in rows {
        for x in 0..128 {
            assert_eq!(pixel(frame, x, y), pixel(reference, x, y), "pixel {x},{y}");
        }
    }
}

#[test]
fn selection_bar_slides_between_rows_with_the_text_in_place() {
    let start = animated(
        selected(1),
        OledBodyAnimation::Select {
            from_row: 0,
            moved: 0,
        },
    );
    assert_eq!(start, render_oled_frame(&selected(0)));
    let end = animated(
        selected(1),
        OledBodyAnimation::Select {
            from_row: 0,
            moved: 13,
        },
    );
    assert_eq!(end, render_oled_frame(&selected(1)));

    for moved in [5, 9] {
        let frame = animated(
            selected(1),
            OledBodyAnimation::Select { from_row: 0, moved },
        );
        let bar = 10 + moved..21 + moved;
        for y in 10..34 {
            let expected = if bar.contains(&y) { AREA } else { 0 };
            assert_eq!(pixel(&frame, 70, y), expected, "moved {moved}, row {y}");
        }
        assert_rows_equal(&frame, &end, (0..10).chain(34..128));
    }

    let upward = animated(
        selected(0),
        OledBodyAnimation::Select {
            from_row: 1,
            moved: 5,
        },
    );
    assert!((18..29).all(|y| pixel(&upward, 70, y) == AREA));
    assert!([17, 29].iter().all(|y| pixel(&upward, 70, *y) == 0));
}

#[test]
fn scroll_moves_the_list_under_a_fixed_bar_and_the_thumb_follows() {
    let old = base();
    let new = scrolled_down();
    let scroll = |moved| OledBodyAnimation::Scroll {
        from: from(&old),
        down: true,
        moved,
    };
    assert_eq!(animated(new.clone(), scroll(0)), render_oled_frame(&old));
    assert_eq!(animated(new.clone(), scroll(13)), render_oled_frame(&new));

    let frame = animated(new.clone(), scroll(5));
    let (old_plain, new_plain) = (unselected(&old), unselected(&new));
    for y in BODY.filter(|y| !(23..34).contains(y)) {
        let (source, src) = if y + 5 <= 113 {
            (&old_plain, y + 5)
        } else {
            (&new_plain, y + 5 - 13)
        };
        for x in 0..126 {
            assert_eq!(pixel(&frame, x, y), pixel(source, x, src), "pixel {x},{y}");
        }
    }
    for y in 23..34 {
        let lit = (9..126).filter(|x| pixel(&frame, *x, y) == AREA).count();
        assert!(lit > 0, "the bar keeps its band, row {y}");
    }
    let thumb_top = |frame: &[u8]| (10..112).find(|y| pixel(frame, 126, *y) == AREA);
    let (before, during, after) = (
        thumb_top(&render_oled_frame(&old)),
        thumb_top(&frame),
        thumb_top(&render_oled_frame(&new)),
    );
    assert!(
        before < during && during < after,
        "{before:?} {during:?} {after:?}"
    );

    let upward = animated(
        old.clone(),
        OledBodyAnimation::Scroll {
            from: from(&new),
            down: false,
            moved: 13,
        },
    );
    assert_eq!(upward, render_oled_frame(&old));
}

#[test]
fn a_scroll_onto_a_page_that_fits_draws_no_scrollbar() {
    let mut old = base();
    let mut new = scrolled_down();
    for input in [&mut old, &mut new] {
        input.display.scroll.as_mut().unwrap().total_rows = 8;
    }
    let frame = animated(
        new,
        OledBodyAnimation::Scroll {
            from: from(&old),
            down: true,
            moved: 5,
        },
    );
    assert!((10..112).all(|y| pixel(&frame, 126, y) == 0));
}

#[test]
fn enter_and_back_slide_only_the_body_in_from_their_side() {
    let mut parent = selected(5);
    parent.display.title = "../I1: synth/Synth".into();
    let child = base();
    let old = render_oled_frame(&parent);
    let new = render_oled_frame(&child);
    for moved in [48, 88, 116] {
        let keep = 128 - moved;
        let enter = animated(
            child.clone(),
            OledBodyAnimation::Slide {
                from: from(&parent),
                forward: true,
                moved,
            },
        );
        let back = animated(
            parent.clone(),
            OledBodyAnimation::Slide {
                from: from(&child),
                forward: false,
                moved,
            },
        );
        assert_rows_equal(&enter, &new, (0..9).chain(114..128));
        assert_rows_equal(&back, &old, (0..9).chain(114..128));
        for y in BODY {
            for x in 0..128 {
                let expected_enter = if x < keep {
                    pixel(&old, x + moved, y)
                } else {
                    pixel(&new, x - keep, y)
                };
                assert_eq!(
                    pixel(&enter, x, y),
                    expected_enter,
                    "enter {moved}: {x},{y}"
                );
                let reference = if x < moved {
                    pixel(&old, x + keep, y)
                } else {
                    pixel(&new, x - moved, y)
                };
                assert_eq!(pixel(&back, x, y), reference, "back {moved}: {x},{y}");
            }
        }
    }
    let done = animated(
        child.clone(),
        OledBodyAnimation::Slide {
            from: from(&parent),
            forward: true,
            moved: 128,
        },
    );
    assert_eq!(done, new);
}

#[test]
fn edit_box_grows_from_and_shrinks_to_the_bar_end() {
    let plain = selected(1);
    let mut editing = selected(1);
    editing.display.editing = true;
    let (plain_frame, edit_frame) = (render_oled_frame(&plain), render_oled_frame(&editing));
    let box_x = (0..128)
        .find(|x| (23..34).all(|y| pixel(&edit_frame, *x, y) != AREA) && *x > 60)
        .unwrap();
    let cut = 128 - (128 - box_x) / 2;
    for input in [editing.clone(), plain.clone()] {
        let frame = animated(input.clone(), OledBodyAnimation::Edit);
        assert_rows_equal(&frame, &render_oled_frame(&input), (0..23).chain(34..128));
        for y in 23..34 {
            for x in 0..128 {
                let source = if x < cut { &plain_frame } else { &edit_frame };
                assert_eq!(pixel(&frame, x, y), pixel(source, x, y), "pixel {x},{y}");
            }
        }
    }
}

#[test]
fn toast_box_rises_from_the_bottom_edge_over_the_hints() {
    let mut input = base();
    input.display.toast = "Saved".into();
    let full = render_oled_frame(&input);
    let mut hints_only = input.clone();
    hints_only.display.toast.clear();
    let hints = render_oled_frame(&hints_only);
    let mut rising = input.clone();
    rising.animation.toast = Some(OledToastSlide {
        text: "Saved".into(),
        visible_rows: 5,
    });
    let frame = render_oled_frame(&rising);
    assert_rows_equal(&frame, &hints, 0..123);
    for y in 123..128 {
        for x in 0..108 {
            assert_eq!(pixel(&frame, x, y), pixel(&full, x, y - 8), "pixel {x},{y}");
        }
        for x in 108..128 {
            assert_eq!(pixel(&frame, x, y), pixel(&full, x, y), "icon {x},{y}");
        }
    }

    hints_only.animation.toast = Some(OledToastSlide {
        text: "Saved".into(),
        visible_rows: 13,
    });
    assert_eq!(
        render_oled_frame(&hints_only),
        full,
        "a leaving toast keeps its text"
    );
    hints_only.animation.toast.as_mut().unwrap().visible_rows = 0;
    assert_eq!(render_oled_frame(&hints_only), hints);
}

/// Prints the raster cost of a static, the busiest static and the animated frames; run with
/// `cargo test --release -p playback-runtime --lib frame_raster_cost -- --ignored --nocapture`.
#[test]
#[ignore = "release-mode micro-benchmark"]
fn frame_raster_cost() {
    let mut busiest = base();
    busiest.display.editing = true;
    busiest.display.toast = "Saved: Patch 3".into();
    busiest.save_flash = OledSaveFlash::Flash;
    busiest.metrics.missed_quantum_flash = true;
    busiest.recording_active = true;
    busiest.aux_hints.normal[0].click = hint("Map");
    busiest.aux_hints.normal[2].click = hint("Record");
    let mut parent = selected(5);
    parent.display.title = "../I1: synth/Synth".into();
    let mut slide = base();
    slide.animation.body = Some(OledBodyAnimation::Slide {
        from: from(&parent),
        forward: true,
        moved: 88,
    });
    let mut scroll = scrolled_down();
    scroll.animation.body = Some(OledBodyAnimation::Scroll {
        from: from(&base()),
        down: true,
        moved: 9,
    });
    let mut frame = vec![0_u8; OLED_FRAME_BYTES];
    for (name, input) in [
        ("static", base()),
        ("busiest static", busiest),
        ("enter slide", slide),
        ("scroll", scroll),
    ] {
        let runs = 2000;
        let started = std::time::Instant::now();
        for _ in 0..runs {
            render_oled_frame_into(std::hint::black_box(&input), &mut frame);
        }
        let per_frame = started.elapsed().as_secs_f64() * 1e6 / f64::from(runs);
        println!("{name} OLED frame raster: {per_frame:.1} us/frame");
    }
    std::hint::black_box(&frame);
}
