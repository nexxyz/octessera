use super::super::{
    NativeParamBinding, NativeRunnerConfig, NativeToast, PendingNativeToast, TransportFlash,
};
use super::*;

const SNAPSHOT_TICK: Duration = Duration::from_millis(33);

fn runner_at(now: Instant) -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.skip_startup_splash();
    runner.test_set_display_time(now);
    runner.display.last_interaction_at = now;
    runner
}

#[test]
fn one_shot_deadlines_cover_native_display_transitions() {
    let start = Instant::now();
    let mut runner = runner_at(start);
    runner.display.ui.dim_timer_seconds = 1;
    runner.display.ui.screen_sleep_seconds = 2;
    assert_eq!(
        runner.next_timed_display_snapshot_deadline(),
        Some(start + Duration::from_secs(1))
    );

    runner.display.ui.dim_timer_seconds = 0;
    assert_eq!(
        runner.next_timed_display_snapshot_deadline(),
        Some(start + Duration::from_secs(2))
    );

    runner.display.ui.screen_sleep_seconds = 0;
    runner.display.oled_mode = NativeOledMode::Splash;
    runner.display.oled_splash_until = Some(start + Duration::from_secs(3));
    assert_eq!(
        runner.next_timed_display_snapshot_deadline(),
        Some(start + Duration::from_secs(3))
    );

    runner.display.oled_mode = NativeOledMode::Normal;
    runner.display.oled_splash_until = None;
    runner.display.toast_expires_at = Some(start + Duration::from_secs(4));
    assert_eq!(
        runner.next_timed_display_snapshot_deadline(),
        Some(start + Duration::from_secs(4))
    );

    runner.display.toast_expires_at = None;
    runner.display.auto_save_flash_until = Some(start + Duration::from_secs(5));
    assert_eq!(
        runner.next_timed_display_snapshot_deadline(),
        Some(start + Duration::from_secs(5))
    );

    runner.display.auto_save_flash_until = None;
    runner.display.transients.trigger_event_dot(start);
    runner.display.transients.acknowledge_snapshot_pending();
    assert_eq!(
        runner.next_timed_display_snapshot_deadline(),
        Some(start + Duration::from_millis(100))
    );

    runner.display.transients.reset(start);
    runner
        .display
        .transients
        .trigger_transport_flash(TransportFlash::Beat, start);
    runner.display.transients.acknowledge_snapshot_pending();
    assert_eq!(
        runner.next_timed_display_snapshot_deadline(),
        Some(start + Duration::from_millis(90))
    );
}

#[test]
fn delayed_hint_and_queued_aux_toast_have_one_shot_deadlines() {
    let start = Instant::now();
    let mut runner = runner_at(start);
    runner.display.ui.fn_held = true;
    runner.display.modifier_hint_started_at = Some(start);
    assert_eq!(
        runner.next_timed_display_snapshot_deadline(),
        Some(start + MODIFIER_HINT_DELAY)
    );

    let mut runner = runner_at(start);
    runner.pending.pending_aux_turn_toast = Some(PendingNativeToast {
        head: "Trn-1: Cutoff: ".into(),
        value: "56".into(),
    });
    runner.display.aux_turn_toast_cooldown_until = Some(start + Duration::from_millis(500));
    assert_eq!(
        runner.next_timed_display_snapshot_deadline(),
        Some(start + Duration::from_millis(500))
    );
}

#[test]
fn overdue_modifier_hint_reappears_immediately_after_toast_expiry() {
    let start = Instant::now();
    let mut runner = runner_at(start);
    runner.display.ui.fn_held = true;
    runner.display.modifier_hint_started_at = Some(start);
    runner.display.toast = Some(NativeToast {
        message: "blocking".into(),
        offset: 0,
    });
    let unblocked = start + Duration::from_secs(2);
    runner.display.toast_expires_at = Some(unblocked);
    runner.test_set_display_time(unblocked);

    assert_eq!(
        runner.next_timed_display_snapshot_deadline_after(Some(unblocked)),
        Some(unblocked)
    );
}

#[test]
fn held_aux_toast_stops_scrolling_once_it_reaches_its_end() {
    let start = Instant::now();
    let mut runner = runner_at(start);
    let head = "Trn-1: Filter Cutoff: ";
    let message = format!("{head}56");
    let end = message.chars().count() - TOAST_SCROLL_WIDTH;
    runner.display.held_toast_head = Some(head.into());
    runner.display.toast = Some(NativeToast {
        message,
        offset: end - 1,
    });
    runner.display.toast_expires_at = Some(start + Duration::from_secs(2));
    assert!(runner
        .next_continuous_display_snapshot_deadline(start, Duration::from_millis(33))
        .is_some());

    runner.display.toast.as_mut().unwrap().offset = end;
    assert_eq!(
        runner.next_continuous_display_snapshot_deadline(start, Duration::from_millis(33)),
        None
    );
}

#[test]
fn continuous_deadline_uses_attempt_time_for_long_toast_and_long_row() {
    let start = Instant::now();
    let mut runner = runner_at(start);
    runner.display.toast = Some(NativeToast {
        message: "a".repeat(TOAST_SCROLL_WIDTH + 1),
        offset: 0,
    });
    runner.display.toast_expires_at = Some(start + Duration::from_secs(2));

    assert_eq!(
        runner.next_continuous_display_snapshot_deadline(start, SNAPSHOT_TICK),
        Some(start + SNAPSHOT_TICK)
    );
    let next_attempt = start + SNAPSHOT_TICK;
    assert_eq!(
        runner.next_continuous_display_snapshot_deadline(next_attempt, SNAPSHOT_TICK),
        Some(next_attempt + SNAPSHOT_TICK)
    );

    let mut runner = runner_at(start);
    runner.param_mods[0].x[0] = Some(NativeParamBinding {
        key: "instruments.0.sample.filter.resonance".into(),
        label: Some("Filter Resonance Amount".into()),
        kind: "number".into(),
        min: Some(0.0),
        max: Some(100.0),
        step: Some(1.0),
        user_min: None,
        user_max: None,
        options: vec![],
        invert: false,
    });
    runner.menu.rebuild(runner.menu_config());
    assert!(runner.menu.focus_item_key("param:0:x:0"));
    assert_eq!(
        runner.next_continuous_display_snapshot_deadline(start, SNAPSHOT_TICK),
        Some(start + SNAPSHOT_TICK)
    );
}

#[test]
fn toast_scroll_boundary_is_seventeen_columns() {
    let start = Instant::now();
    let mut runner = runner_at(start);
    runner.display.toast = Some(NativeToast {
        message: "a".repeat(TOAST_SCROLL_WIDTH),
        offset: 0,
    });
    runner.display.toast_expires_at = Some(start + Duration::from_secs(2));
    assert_eq!(
        runner.next_continuous_display_snapshot_deadline(start, SNAPSHOT_TICK),
        None
    );

    runner.display.toast = Some(NativeToast {
        message: "a".repeat(TOAST_SCROLL_WIDTH + 1),
        offset: 0,
    });
    assert_eq!(
        runner.next_continuous_display_snapshot_deadline(start, SNAPSHOT_TICK),
        Some(start + SNAPSHOT_TICK)
    );
}

#[test]
fn selected_row_deadline_uses_the_auto_map_prefixed_line() {
    let mut runner = runner_at(Instant::now());
    let instrument_children = &runner.menu.root.children[2].children[0].children[0].children;
    let synth_group = instrument_children
        .iter()
        .position(|item| item.label == "Synth")
        .expect("synth group should exist");
    let filter = instrument_children[synth_group]
        .children
        .iter()
        .position(|item| item.label == "Filter")
        .expect("filter group should exist");
    runner.menu.state.stack = vec![2, 0, 0, 2, filter];
    let mut menu = runner.menu.snapshot();
    menu.lines = vec!["Cutoff".into()];
    menu.full_lines = vec![Some("a".repeat(DISPLAY_LINE_WIDTH))];
    menu.line_keys = vec![Some("instruments.0.synth.filter.cutoffHz".into())];
    menu.line_actions = vec![None];
    menu.selected_row = Some(0);

    let line = selected_menu_presentation_line(&runner, &menu)
        .expect("selected menu line should be available");
    assert!(line.chars().count() > DISPLAY_LINE_WIDTH);
}

#[test]
fn continuous_deadline_is_absent_for_inactive_or_short_content() {
    let start = Instant::now();
    let mut runner = runner_at(start);
    assert_eq!(
        runner.next_continuous_display_snapshot_deadline(start, SNAPSHOT_TICK),
        None
    );
    runner.display.toast = Some(NativeToast {
        message: "short".into(),
        offset: 0,
    });
    runner.display.toast_expires_at = Some(start + Duration::from_secs(2));
    assert_eq!(
        runner.next_continuous_display_snapshot_deadline(start, SNAPSHOT_TICK),
        None
    );

    runner.display.toast = Some(NativeToast {
        message: "a".repeat(TOAST_SCROLL_WIDTH + 1),
        offset: 0,
    });
    runner.display.toast_expires_at = Some(start - Duration::from_millis(1));
    assert_eq!(
        runner.next_continuous_display_snapshot_deadline(start, SNAPSHOT_TICK),
        None
    );
}

#[test]
fn failed_one_shot_snapshot_remains_due_until_retry() {
    let start = Instant::now();
    let mut runner = runner_at(start);
    runner.display.ui.dim_timer_seconds = 0;
    runner.display.ui.screen_sleep_seconds = 0;
    runner.display.transients.trigger_event_dot(start);
    runner.display.transients.acknowledge_snapshot_pending();
    let expiry = start + Duration::from_millis(100);
    runner.test_set_display_time(expiry);
    runner.test_fail_next_snapshot();

    assert!(runner.messages_with_snapshot().is_err());
    assert!(runner.display.transients.snapshot_pending());
    assert_eq!(
        runner.next_timed_display_snapshot_deadline_after(Some(start)),
        Some(start)
    );

    runner.messages_with_snapshot().unwrap();
    assert!(!runner.display.transients.snapshot_pending());
    assert_eq!(
        runner.next_timed_display_snapshot_deadline_after(Some(start)),
        Some(start + Duration::from_millis(200))
    );
}
