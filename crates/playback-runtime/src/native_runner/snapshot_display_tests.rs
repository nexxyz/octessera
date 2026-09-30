use super::super::{NativeRunner, NativeRunnerConfig};
use super::prefix_line;
use crate::native_menu::{NativeMenuItem, NativeMenuValue};
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
