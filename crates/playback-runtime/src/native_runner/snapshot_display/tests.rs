use super::super::{NativeRunner, NativeRunnerConfig};
use super::prefix_line;
use crate::native_menu::{NativeMenuAction, NativeMenuItem, NativeMenuValue};
use std::time::{Duration, Instant};

const SNAPSHOT_TICK: Duration = Duration::from_millis(33);

fn set_enum_option(item: &mut NativeMenuItem, key: &str, value: &str) -> bool {
    if item.key.as_deref() == Some(key) {
        if let NativeMenuValue::Enum { options, .. } = &mut item.value {
            options.fill(value.into());
            return true;
        }
    }
    item.children
        .iter_mut()
        .any(|child| set_enum_option(child, key, value))
}

fn runner_at(now: Instant) -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.skip_startup_splash();
    runner.test_set_display_time(now);
    runner.display.last_interaction_at = now;
    runner
}

#[test]
fn auto_mapped_action_rows_keep_equal_prefix_alignment() {
    assert_eq!(
        prefix_line(">!Do It".into(), Some("1!".into())),
        "> 1!Do It"
    );
    assert_eq!(prefix_line(" !Do It".into(), Some("1!".into())), "1!Do It");
}

#[test]
fn auto_mapped_value_rows_keep_turn_prefix_alignment() {
    assert_eq!(
        prefix_line("> Cutoff".into(), Some("1-".into())),
        "> 1-Cutoff"
    );
    assert_eq!(
        prefix_line("  Cutoff".into(), Some("1-".into())),
        "1-Cutoff"
    );
}

#[test]
fn system_save_and_load_confirmations_render_complete_nineteen_column_details() {
    for (action_type, detail) in [
        ("system.save", "Save System"),
        ("system.load", "Load System"),
    ] {
        let mut runner = runner_at(Instant::now());
        let action = NativeMenuAction::PlatformEffect(action_type.into());
        runner.display.confirm_dialog = runner.confirmation_for_action(&action);
        let snapshot = runner.snapshot().unwrap();
        let lines = snapshot["display"]["lines"].as_array().unwrap();

        assert_eq!(snapshot["display"]["title"], "Confirm System");
        assert_eq!(lines.len(), 4);
        assert_eq!(lines[0], detail);
        assert_eq!(lines[1], "settings?");
        assert_eq!(lines[2], "> Cancel");
        assert_eq!(lines[3], "  Confirm");
        assert!(lines.len() <= 7);
        assert!(lines
            .iter()
            .all(|line| line.as_str().unwrap().chars().count() <= 19));
    }
}

#[test]
fn link_instrument_edit_detail_scrolls_on_timed_snapshots_only() {
    let start = Instant::now();
    let mut runner = runner_at(start);
    let key = "layers.0.link.mapping.activate.slot";
    assert!(runner.menu.focus_item_key(key));
    assert!(set_enum_option(
        &mut runner.menu.root,
        key,
        "I1: This Long Instrument Name Scrolls Across the OLED Display"
    ));
    runner.menu.state.editing = true;

    let menu = runner.menu.snapshot();
    let detail_row = menu.selected_row.expect("selected detail row");
    assert_eq!(menu.lines[detail_row - 1], "  On Inst:");
    assert_eq!(
        menu.lines[detail_row],
        ">* I1: This Long Instrument Name Scrolls Across the OLED Display"
    );
    assert_eq!(
        menu.full_lines[detail_row].as_deref(),
        Some(">* I1: This Long Instrument Name Scrolls Across the OLED Display")
    );
    assert_eq!(
        runner.next_continuous_display_snapshot_deadline(start, SNAPSHOT_TICK),
        Some(start + SNAPSHOT_TICK)
    );

    let first = runner.display_snapshot(menu);
    assert_eq!(
        first.full_lines[detail_row].as_deref(),
        Some(">* I1: This Long Instrument Name Scrolls Across the OLED Display")
    );
    assert!(first.lines.len() <= 7);
    runner.display.menu_scroll_offset = 8;
    let next_attempt = start + SNAPSHOT_TICK;
    assert_eq!(
        runner.next_continuous_display_snapshot_deadline(next_attempt, SNAPSHOT_TICK),
        Some(next_attempt + SNAPSHOT_TICK)
    );
    let scrolled = runner.display_snapshot(runner.menu.snapshot());
    assert_ne!(first.lines[detail_row], scrolled.lines[detail_row]);
    assert!(scrolled.lines.len() <= 7);

    assert!(runner.menu.focus_item_key("layers.0.link.pitch.scale"));
    runner.menu.state.editing = true;
    assert_eq!(
        runner.next_continuous_display_snapshot_deadline(next_attempt, SNAPSHOT_TICK),
        None
    );
}

#[test]
fn header_rule_uses_the_current_menu_area_colour() {
    use crate::oled_frame::{render_oled_frame, OledPresentationMetrics};
    use platform_core::palette;

    let mut runner = runner_at(Instant::now());
    runner.display.ui.display_brightness = 100;
    for (key, expected) in [
        (None, palette::GRAY_RGB565),
        (Some("behaviorId"), palette::GREEN_RGB565),
        (Some("play.xy.smoothingMs"), palette::YELLOW_RGB565),
    ] {
        if let Some(key) = key {
            assert!(runner.menu.focus_item_key(key), "{key}");
        }
        let input = runner
            .capture_presentation_scene(false)
            .unwrap()
            .oled_presentation_input(OledPresentationMetrics::default(), None);
        assert_eq!(input.display.title_color, expected, "{key:?}");
        let frame = render_oled_frame(&input);
        let pixel = |x: usize, y: usize| {
            let offset = (y * 128 + x) * 2;
            u16::from_be_bytes([frame[offset], frame[offset + 1]])
        };
        assert!((0..128).all(|x| pixel(x, 14) == expected), "{key:?}");
        assert!((0..128).all(|x| pixel(x, 13) != expected), "{key:?}");
    }
}
