use super::*;

fn input(runner: &mut NativeRunner, input: Value) -> Vec<RunnerMessage> {
    runner
        .send(HostMessage::DeviceInput {
            input,
            request_snapshot: None,
        })
        .unwrap()
}

fn tap(runner: &mut NativeRunner, x: usize, y: usize) {
    let _ = input(runner, json!({ "type": "grid_press", "x": x, "y": y }));
    let _ = input(runner, json!({ "type": "grid_release", "x": x, "y": y }));
}

fn shift_tap(runner: &mut NativeRunner, x: usize, y: usize) {
    let _ = input(runner, json!({ "type": "button_shift", "pressed": true }));
    tap(runner, x, y);
    let _ = input(runner, json!({ "type": "button_shift", "pressed": false }));
}

fn xy_runner(release: &str) -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner
        .menu
        .set_enum_value_for_key("play.xy.release", release);
    runner
        .apply_or_schedule_menu_key("play.xy.release")
        .unwrap();
    assert_eq!(runner.xy.release, release);
    open_xy_page(&mut runner);
    let _ = runner.messages_with_snapshot().unwrap();
    runner.config_dirty = false;
    runner.dirty_revision = None;
    runner
}

fn open_xy_page(runner: &mut NativeRunner) {
    for button in ["button_shift", "button_fn"] {
        let _ = input(runner, json!({ "type": button, "pressed": true }));
    }
    tap(runner, GRID_WIDTH - 1, 1);
    for button in ["button_fn", "button_shift"] {
        let _ = input(runner, json!({ "type": button, "pressed": false }));
    }
    assert_eq!(runner.active_play_mode, "xy");
}

fn tile(runner: &NativeRunner, x: usize, y: usize) -> Value {
    led_tiles(&runner.snapshot().unwrap())[display_index(x, y)].clone()
}

#[test]
fn shift_press_latches_the_position_even_with_center_release() {
    let mut runner = xy_runner("reset-center");
    let saved = runner.config_payload();

    shift_tap(&mut runner, 6, 5);
    assert!(!runner.xy.touch.active);
    assert!((runner.xy.touch.display_x - 6.0 / 7.0).abs() < 0.0001);
    assert!((runner.xy.touch.display_y - 5.0 / 7.0).abs() < 0.0001);
    assert_eq!(tile(&runner, 6, 5), led_rgb(platform_core::palette::WHITE));
    assert!(!runner.config_dirty, "the latch is live only");
    assert_eq!(
        runner.config_payload()["runtimeConfig"],
        saved["runtimeConfig"]
    );

    tap(&mut runner, 2, 2);
    assert_eq!(runner.xy.touch.display_x, 0.5);
    assert_eq!(runner.xy.touch.display_y, 0.5);
    assert_ne!(tile(&runner, 3, 4), led_rgb(platform_core::palette::WHITE));
}

#[test]
fn latch_behaves_like_hold_under_sample_hold() {
    let mut runner = xy_runner("sample-hold");
    shift_tap(&mut runner, 1, 6);
    assert!((runner.xy.touch.display_x - 1.0 / 7.0).abs() < 0.0001);
    assert_eq!(tile(&runner, 1, 6), led_rgb(platform_core::palette::WHITE));

    tap(&mut runner, 5, 2);
    assert!((runner.xy.touch.display_x - 5.0 / 7.0).abs() < 0.0001);
    assert_eq!(
        tile(&runner, 5, 2),
        led_rgb(dim_rgb(platform_core::palette::GRAY, 2))
    );
}

#[test]
fn clear_all_and_patch_load_drop_the_latch() {
    let mut runner = xy_runner("reset-center");
    shift_tap(&mut runner, 6, 5);
    assert!(runner.play_live.xy_latched);
    let payload = runner.config_payload();
    runner.apply_config_payload(payload).unwrap();
    assert!(!runner.play_live.xy_latched);
    assert_eq!(
        (runner.xy.touch.display_x, runner.xy.touch.display_y),
        (0.5, 0.5),
        "center release recentres once the latch is gone"
    );
    assert_ne!(tile(&runner, 6, 5), led_rgb(platform_core::palette::WHITE));

    open_xy_page(&mut runner);
    shift_tap(&mut runner, 6, 5);
    assert!(runner.play_live.xy_latched);
    runner.clear_patch_state().unwrap();
    assert!(!runner.play_live.xy_latched);
}
