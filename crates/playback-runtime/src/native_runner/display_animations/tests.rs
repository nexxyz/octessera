use super::*;
use crate::native_runner::snapshot_display::DisplayScrollMetadata;

const MS: Duration = Duration::from_millis(1);

fn page(title: &str, selected: usize, offset: usize) -> DisplaySnapshot {
    let lines = (offset..offset + 8)
        .map(|row| format!("{title} {row}"))
        .collect::<Vec<_>>();
    DisplaySnapshot {
        body_layout: OledDisplayLayout::Rows,
        title: title.into(),
        colors: vec![0x001f; lines.len()],
        bar_values: vec![None; lines.len()],
        full_lines: vec![None; lines.len()],
        rows: vec![None; lines.len()],
        lines,
        scroll: Some(DisplayScrollMetadata {
            scroll_offset: offset,
            total_rows: 12,
            visible_rows: 8,
        }),
        selected_row: Some(selected),
    }
}

struct Screen {
    display: DisplaySnapshot,
    editing: bool,
    toast: String,
    stack: Option<Vec<usize>>,
}

impl Screen {
    fn menu(display: DisplaySnapshot, stack: &[usize]) -> Self {
        Self {
            display,
            editing: false,
            toast: String::new(),
            stack: Some(stack.to_vec()),
        }
    }

    fn observe(&self, animations: &mut DisplayAnimations, now: Instant) -> OledAnimationInput {
        animations.observe(
            &AnimationFrame {
                display: &self.display,
                title_color: 0x001f,
                editing: self.editing,
                toast: &self.toast,
                menu_stack: self.stack.as_deref(),
                menu_frame: true,
            },
            now,
        )
    }
}

fn moved(animation: &OledAnimationInput) -> Option<usize> {
    match animation.body.as_ref()? {
        OledBodyAnimation::Select { moved, .. }
        | OledBodyAnimation::Scroll { moved, .. }
        | OledBodyAnimation::Slide { moved, .. } => Some(*moved),
        OledBodyAnimation::Edit => Some(1),
    }
}

/// Starts from a settled screen showing `first` at `start`.
fn settled(first: &Screen, start: Instant) -> DisplayAnimations {
    let mut animations = DisplayAnimations::default();
    assert!(first.observe(&mut animations, start).is_empty());
    animations
}

#[test]
fn selection_move_eases_over_three_frames_and_ends_exactly() {
    let start = Instant::now();
    let mut animations = settled(&Screen::menu(page("Filter", 1, 0), &[2]), start);
    let next = Screen::menu(page("Filter", 2, 0), &[2]);
    let at = start + 500 * MS;

    let first = next.observe(&mut animations, at);
    assert_eq!(
        first.body,
        Some(OledBodyAnimation::Select {
            from_row: 1,
            moved: 5
        })
    );
    assert_eq!(moved(&next.observe(&mut animations, at + 32 * MS)), Some(5));
    assert_eq!(moved(&next.observe(&mut animations, at + 33 * MS)), Some(9));
    assert!(next.observe(&mut animations, at + 66 * MS).is_empty());
    assert!(next.observe(&mut animations, at + 400 * MS).is_empty());
}

#[test]
fn deadlines_follow_the_frames_while_active_and_stop_at_the_end_state() {
    let start = Instant::now();
    let mut animations = settled(&Screen::menu(page("Filter", 1, 0), &[2]), start);
    assert_eq!(animations.next_deadline(None), None);
    let at = start + 500 * MS;
    Screen::menu(page("Filter", 2, 0), &[2]).observe(&mut animations, at);

    assert_eq!(animations.next_deadline(Some(at)), Some(at + 33 * MS));
    assert!(!animations.advance(at + 20 * MS));
    assert!(animations.advance(at + 33 * MS), "the second frame is due");
    assert_eq!(
        animations.next_deadline(Some(at + 33 * MS)),
        Some(at + 66 * MS)
    );
    assert!(animations.advance(at + 70 * MS), "the end state is due");
    assert_eq!(animations.next_deadline(None), None);
    assert!(!animations.advance(at + 200 * MS));
}

#[test]
fn a_late_board_skips_steps_but_still_finishes_on_time() {
    let start = Instant::now();
    let mut animations = settled(&Screen::menu(page("Synth", 3, 0), &[1]), start);
    let child = Screen::menu(page("Filter", 0, 0), &[1, 3]);
    let at = start + 500 * MS;
    assert_eq!(moved(&child.observe(&mut animations, at)), Some(48));
    assert_eq!(
        animations.next_deadline(Some(at + 90 * MS)),
        Some(at + 99 * MS)
    );
    assert!(child.observe(&mut animations, at + 99 * MS).is_empty());
}

#[test]
fn enter_and_back_slide_through_their_offsets_from_the_page_left() {
    let start = Instant::now();
    let parent = Screen::menu(page("Synth", 3, 0), &[1]);
    let child = Screen::menu(page("Filter", 0, 0), &[1, 3]);
    let mut animations = settled(&parent, start);

    let at = start + 500 * MS;
    let offsets = (0..4)
        .map(|frame| child.observe(&mut animations, at + 33 * MS * frame))
        .collect::<Vec<_>>();
    assert_eq!(
        offsets.iter().map(moved).collect::<Vec<_>>(),
        [Some(48), Some(88), Some(116), None]
    );
    let Some(OledBodyAnimation::Slide { from, forward, .. }) = &offsets[0].body else {
        panic!("enter slides: {:?}", offsets[0]);
    };
    assert!(*forward);
    assert_eq!(from.display.title, "Synth");
    assert_eq!(from.selected_row, Some(3));

    let back_at = at + 500 * MS;
    let back = parent.observe(&mut animations, back_at);
    let Some(OledBodyAnimation::Slide {
        from,
        forward,
        moved,
    }) = back.body
    else {
        panic!("back slides: {back:?}");
    };
    assert_eq!((forward, moved), (false, 48));
    assert_eq!(from.display.title, "Filter");
}

#[test]
fn scroll_keeps_the_bar_and_names_its_direction() {
    let start = Instant::now();
    let mut animations = settled(&Screen::menu(page("Life", 3, 1), &[0]), start);
    let at = start + 500 * MS;
    let down = Screen::menu(page("Life", 3, 2), &[0]).observe(&mut animations, at);
    let Some(OledBodyAnimation::Scroll { from, down, moved }) = down.body else {
        panic!("scrolls: {down:?}");
    };
    assert_eq!((down, moved), (true, 5));
    assert_eq!(
        from.display.scroll.as_ref().map(|scroll| scroll.offset),
        Some(1)
    );

    let up = Screen::menu(page("Life", 3, 1), &[0]).observe(&mut animations, at + 500 * MS);
    assert!(matches!(
        up.body,
        Some(OledBodyAnimation::Scroll { down: false, .. })
    ));
}

#[test]
fn edit_box_takes_two_frames_each_way() {
    let start = Instant::now();
    let mut animations = settled(&Screen::menu(page("Filter", 1, 0), &[2]), start);
    let mut editing = Screen::menu(page("Filter", 1, 0), &[2]);
    editing.editing = true;
    let at = start + 500 * MS;
    assert_eq!(
        editing.observe(&mut animations, at).body,
        Some(OledBodyAnimation::Edit)
    );
    assert!(editing.observe(&mut animations, at + 33 * MS).is_empty());

    let plain = Screen::menu(page("Filter", 1, 0), &[2]);
    let leave = at + 500 * MS;
    assert_eq!(
        plain.observe(&mut animations, leave).body,
        Some(OledBodyAnimation::Edit)
    );
    assert!(plain.observe(&mut animations, leave + 33 * MS).is_empty());
}

#[test]
fn new_input_mid_animation_jumps_to_the_end_and_animates_from_there() {
    let start = Instant::now();
    let mut animations = settled(&Screen::menu(page("Synth", 2, 0), &[1]), start);
    let at = start + 500 * MS;
    Screen::menu(page("Synth", 3, 0), &[1]).observe(&mut animations, at);

    let entered =
        Screen::menu(page("Filter", 0, 0), &[1, 3]).observe(&mut animations, at + 20 * MS);
    let Some(OledBodyAnimation::Slide { from, moved, .. }) = entered.body else {
        panic!("the slide starts at once: {entered:?}");
    };
    assert_eq!(moved, 48);
    assert_eq!(
        from.selected_row,
        Some(3),
        "it leaves the select's end state"
    );
    assert_eq!(
        animations.next_deadline(Some(at + 20 * MS)),
        Some(at + 53 * MS)
    );
}

#[test]
fn a_fast_spin_shows_end_states_until_the_detents_slow_down() {
    let start = Instant::now();
    let mut animations = settled(&Screen::menu(page("Life", 0, 0), &[0]), start);
    let at = start + 500 * MS;
    let detent = |row: usize, when: Instant, animations: &mut DisplayAnimations| {
        Screen::menu(page("Life", row, 0), &[0]).observe(animations, when)
    };
    assert_eq!(moved(&detent(1, at, &mut animations)), Some(5));
    for (row, ms) in [(2, 20), (3, 40), (4, 60), (5, 80)] {
        let shown = detent(row, at + ms * MS, &mut animations);
        assert!(shown.is_empty(), "detent {row} shows its end state");
        assert_eq!(animations.next_deadline(None), None);
    }
    assert_eq!(moved(&detent(6, at + 180 * MS, &mut animations)), Some(5));
}

#[test]
fn toast_slides_up_on_show_and_down_on_hide_with_its_text() {
    let start = Instant::now();
    let mut screen = Screen::menu(page("Filter", 1, 0), &[2]);
    let mut animations = settled(&screen, start);
    screen.toast = "Saved".into();
    let at = start + 500 * MS;
    let rows = |animations: &mut DisplayAnimations, screen: &Screen, when: Instant| {
        screen
            .observe(animations, when)
            .toast
            .map(|slide| (slide.text, slide.visible_rows))
    };
    let shown = (0..4)
        .map(|frame| rows(&mut animations, &screen, at + 33 * MS * frame))
        .collect::<Vec<_>>();
    let saved = |visible| Some(("Saved".to_owned(), visible));
    assert_eq!(shown, [saved(5), saved(9), saved(12), None]);

    screen.toast.clear();
    let leave = at + 2_000 * MS;
    let hidden = (0..4)
        .map(|frame| rows(&mut animations, &screen, leave + 33 * MS * frame))
        .collect::<Vec<_>>();
    assert_eq!(hidden, [saved(8), saved(4), saved(1), None]);
}

#[test]
fn modals_splash_and_unrelated_page_changes_do_not_animate() {
    let start = Instant::now();
    let mut animations = settled(&Screen::menu(page("Filter", 1, 0), &[2]), start);
    let mut modal = Screen::menu(page("Help", 2, 0), &[2]);
    modal.stack = None;
    assert!(modal.observe(&mut animations, start + 500 * MS).is_empty());
    let mut modal_select = Screen::menu(page("Help", 1, 0), &[2]);
    modal_select.stack = None;
    assert!(modal_select
        .observe(&mut animations, start + 1_000 * MS)
        .is_empty());

    let other_section = Screen::menu(page("Build", 1, 0), &[0, 4]);
    assert!(other_section
        .observe(&mut animations, start + 1_500 * MS)
        .is_empty());

    let mut animations = settled(&Screen::menu(page("Filter", 1, 0), &[2]), start);
    let mut splash = Screen::menu(page("Filter", 1, 0), &[2]);
    splash.toast = "Saved".into();
    let frame = AnimationFrame {
        display: &splash.display,
        title_color: 0x001f,
        editing: false,
        toast: &splash.toast,
        menu_stack: None,
        menu_frame: false,
    };
    assert!(animations.observe(&frame, start + 500 * MS).is_empty());
}

#[test]
fn value_changes_keep_a_running_animation() {
    let start = Instant::now();
    let mut animations = settled(&Screen::menu(page("Filter", 1, 0), &[2]), start);
    let at = start + 500 * MS;
    let mut next = Screen::menu(page("Filter", 2, 0), &[2]);
    next.observe(&mut animations, at);
    next.display.lines[4] = "Cutoff 900".into();
    assert_eq!(moved(&next.observe(&mut animations, at + 40 * MS)), Some(9));
}
