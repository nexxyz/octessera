use super::*;
use crate::oled_frame::{
    presentation_input_from_snapshot, render_oled_frame, OledBodyAnimation, OledPresentationMetrics,
};
use std::time::{Duration, Instant};

const MS: Duration = Duration::from_millis(1);

fn turn(runner: &mut NativeRunner, input: Value) -> Value {
    let messages = runner
        .send(HostMessage::DeviceInput {
            input,
            request_snapshot: None,
        })
        .unwrap();
    snapshot_from(&messages)
}

fn system_page(start: Instant) -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.skip_startup_splash();
    runner.display.transients.set_test_now(start);
    runner.menu.open_labels(&["System"]);
    runner.messages_with_snapshot().unwrap();
    runner
}

#[test]
fn a_detent_moves_the_cursor_at_once_and_the_bar_animates_after_it() {
    let start = Instant::now();
    let mut runner = system_page(start);
    let snapshot = turn(
        &mut runner,
        json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
    );

    assert_eq!(
        runner.menu.state.cursor, 1,
        "the menu moves before any frame"
    );
    assert_eq!(snapshot["selectedRow"], 1);
    assert_eq!(snapshot["oledAnimation"]["body"]["kind"], "select");
    assert_eq!(snapshot["oledAnimation"]["body"]["moved"], 5);
    assert_eq!(
        runner.next_timed_display_snapshot_deadline_after(Some(start)),
        Some(start + 33 * MS)
    );

    runner.display.transients.set_test_now(start + 66 * MS);
    let settled = snapshot_from(&runner.messages_with_snapshot().unwrap());
    assert!(settled["oledAnimation"].is_null());
    assert!(runner
        .next_timed_display_snapshot_deadline_after(Some(start + 66 * MS))
        .is_none_or(|deadline| deadline >= start + Duration::from_secs(1)));
}

#[test]
fn desktop_snapshot_and_board_scene_render_the_same_animated_frame() {
    let start = Instant::now();
    let mut runner = system_page(start);
    runner.menu.select_label("Display");
    runner.messages_with_snapshot().unwrap();
    let snapshot = turn(
        &mut runner,
        json!({ "type": "encoder_press", "id": "main" }),
    );
    assert_eq!(snapshot["oledAnimation"]["body"]["kind"], "slide");

    let metrics = OledPresentationMetrics::default();
    let desktop = presentation_input_from_snapshot(&snapshot, metrics.clone())
        .unwrap()
        .unwrap();
    let board = runner
        .capture_display_scene()
        .unwrap()
        .oled_presentation_input(metrics, None);
    let Some(OledBodyAnimation::Slide {
        from,
        forward,
        moved,
    }) = &board.animation.body
    else {
        panic!("the board scene slides too: {:?}", board.animation);
    };
    assert_eq!((*forward, *moved), (true, 48));
    assert_eq!(from.display.title, "/System");
    assert_eq!(desktop.animation, board.animation);
    assert_eq!(render_oled_frame(&desktop), render_oled_frame(&board));
}
