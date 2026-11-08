use super::*;
use std::time::Duration;

fn input(runner: &mut NativeRunner, input: Value) -> Vec<RunnerMessage> {
    runner
        .send(HostMessage::DeviceInput {
            input,
            request_snapshot: None,
        })
        .unwrap()
}

fn press(runner: &mut NativeRunner, x: usize, y: usize) -> Vec<RunnerMessage> {
    let mut messages = input(runner, json!({ "type": "grid_press", "x": x, "y": y }));
    messages.extend(input(
        runner,
        json!({ "type": "grid_release", "x": x, "y": y }),
    ));
    messages
}

fn tempo_runner() -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let _ = input(&mut runner, json!({ "type": "button_fn", "pressed": true }));
    let opened = press(&mut runner, GRID_WIDTH - 1, 3);
    let _ = input(
        &mut runner,
        json!({ "type": "button_fn", "pressed": false }),
    );
    assert_eq!(
        runner.active_play_mode,
        ActivePlayMode::Page(PlayMode::Tempo)
    );
    assert_eq!(snapshot_from(&opened)["display"]["toast"], "Play: Tempo");
    assert_eq!(runner.menu.current_key(), Some("play.page.tempo"));
    runner
}

fn toast(runner: &NativeRunner) -> String {
    runner
        .display
        .toast
        .as_ref()
        .map(|toast| toast.message.clone())
        .unwrap_or_default()
}

fn published_bpm(messages: &[RunnerMessage]) -> Option<f64> {
    messages.iter().find_map(|message| match message {
        RunnerMessage::RuntimeConfigChanged { config } => Some(config.bpm),
        _ => None,
    })
}

fn tile(runner: &NativeRunner, x: usize, y: usize) -> Value {
    led_tiles(&runner.snapshot().unwrap())[display_index(x, y)].clone()
}

#[test]
fn tempo_tiles_and_nudges_set_the_saved_bpm_at_once() {
    let mut runner = tempo_runner();

    let messages = press(&mut runner, 1, 7);
    assert_eq!(runner.transport.bpm, 50.0);
    assert_eq!(published_bpm(&messages), Some(50.0));
    assert_eq!(toast(&runner), "Tempo 50");
    assert_eq!(runner.menu.number_for_key("transport.bpm"), Some(50));

    let _ = press(&mut runner, 7, 3);
    assert_eq!(runner.transport.bpm, 240.0);
    let _ = press(&mut runner, 7, 2);
    assert_eq!(runner.transport.bpm, 240.0, "nudges clamp at 240");
    let _ = press(&mut runner, 0, 2);
    assert_eq!(runner.transport.bpm, 236.0);
    assert_eq!(toast(&runner), "Tempo 236");
    let _ = press(&mut runner, 5, 2);
    assert_eq!(runner.transport.bpm, 238.0);
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["transport"]["bpm"],
        238
    );
}

#[test]
fn tempo_leds_mark_the_exact_tile_green_and_the_nearest_yellow() {
    let mut runner = tempo_runner();
    assert_eq!(tile(&runner, 7, 6), led_rgb(platform_core::palette::GREEN));

    let _ = press(&mut runner, 7, 6);
    let _ = press(&mut runner, 1, 2);
    assert_eq!(runner.transport.bpm, 117.0);
    assert_eq!(tile(&runner, 6, 6), led_rgb(platform_core::palette::YELLOW));
    assert_eq!(
        tile(&runner, 7, 6),
        led_rgb(dim_rgb(platform_core::palette::GRAY, 4))
    );
}

#[test]
fn swing_rows_set_a_and_b_with_two_tile_markers() {
    let mut runner = tempo_runner();
    let white = led_rgb(platform_core::palette::WHITE);
    assert_eq!(tile(&runner, 3, 1), white);
    assert_eq!(tile(&runner, 4, 1), white);

    let _ = press(&mut runner, 5, 1);
    assert_eq!(toast(&runner), "Swing 5:4");
    let _ = press(&mut runner, 2, 0);
    assert_eq!(toast(&runner), "Swing 5:3");
    assert_eq!((runner.transport.swing_a, runner.transport.swing_b), (5, 3));
    assert_eq!(runner.menu.number_for_key("transport.swingA"), Some(5));
    assert_eq!(tile(&runner, 4, 1), white);
    assert_eq!(tile(&runner, 5, 1), white);
    assert_eq!(tile(&runner, 2, 0), white);
    assert_eq!(tile(&runner, 3, 0), white);
    assert_ne!(tile(&runner, 4, 0), white);

    let _ = press(&mut runner, 4, 0);
    assert_eq!(toast(&runner), "Swing 5:4");
    let transport = &runner.config_payload()["runtimeConfig"]["transport"];
    assert_eq!(transport["swingA"], 5);
    assert_eq!(transport["swingB"], 4);
}

#[test]
fn external_sync_blocks_tempo_tiles_but_keeps_swing() {
    let mut runner = tempo_runner();
    runner.transport.sync_source = SyncSource::External;
    for (x, y) in [(1, 7), (0, 7), (7, 2)] {
        let _ = press(&mut runner, x, y);
        assert_eq!(toast(&runner), "External sync");
        assert_eq!(runner.transport.bpm, 120.0);
    }
    assert_eq!(
        tile(&runner, 7, 6),
        led_rgb(dim_rgb(platform_core::palette::GREEN, 4))
    );
    let _ = press(&mut runner, 6, 1);
    assert_eq!(toast(&runner), "Swing 6:4");
}

#[test]
fn tap_averages_the_last_four_taps_and_resets_after_two_seconds() {
    let mut runner = tempo_runner();
    let start = Instant::now();
    let tap = |runner: &mut NativeRunner, ms: u64| {
        runner.handle_play_tempo_press_at(0, 7, start + Duration::from_millis(ms));
    };

    tap(&mut runner, 0);
    assert_eq!(runner.transport.bpm, 120.0, "one tap sets nothing");
    assert_eq!(toast(&runner), "Tap");
    tap(&mut runner, 400);
    assert_eq!(runner.transport.bpm, 150.0);
    assert_eq!(toast(&runner), "Tap 150");
    tap(&mut runner, 900);
    tap(&mut runner, 1400);
    tap(&mut runner, 1900);
    // The last four taps span 1500 ms in three beats.
    assert_eq!(runner.transport.bpm, 120.0);

    tap(&mut runner, 4000);
    assert_eq!(runner.transport.bpm, 120.0, "a 2 s gap starts over");
    tap(&mut runner, 5000);
    assert_eq!(runner.transport.bpm, 60.0);
    tap(&mut runner, 7100);
    assert_eq!(
        runner.transport.bpm, 60.0,
        "a gap over 2 s starts over again"
    );
    tap(&mut runner, 7600);
    assert_eq!(
        runner.transport.bpm, 120.0,
        "the reset dropped the old taps"
    );
    tap(&mut runner, 10000);
    tap(&mut runner, 10100);
    assert_eq!(runner.transport.bpm, 240.0, "fast taps clamp at 240");
    assert_eq!(toast(&runner), "Tap 240");
}

#[test]
fn tap_tile_pulses_with_the_beat_only_while_playing() {
    let mut runner = tempo_runner();
    let stopped = led_rgb(dim_rgb(platform_core::palette::GREEN, 6));
    assert_eq!(tile(&runner, 0, 7), stopped);

    let _ = input(&mut runner, json!({ "type": "button_s", "pressed": true }));
    let _ = input(&mut runner, json!({ "type": "button_s", "pressed": false }));
    let now = Instant::now();
    runner.display.transients.set_test_now(now);
    runner
        .display
        .transients
        .trigger_transport_flash(TransportFlash::Measure, now);
    assert_eq!(tile(&runner, 0, 7), led_rgb(platform_core::palette::GREEN));
    runner
        .display
        .transients
        .set_test_now(now + Duration::from_millis(200));
    assert_eq!(tile(&runner, 0, 7), stopped);
    runner
        .display
        .transients
        .trigger_transport_flash(TransportFlash::Beat, now + Duration::from_millis(200));
    assert_eq!(
        tile(&runner, 0, 7),
        led_rgb(dim_rgb(platform_core::palette::GREEN, 2))
    );
}

#[test]
fn fn_or_shift_fn_with_a_page_tile_leaves_the_hidden_page_alone() {
    let mut runner = tempo_runner();
    let hold = |runner: &mut NativeRunner, buttons: &[&str], pressed: bool| {
        for button in buttons {
            let _ = input(runner, json!({ "type": button, "pressed": pressed }));
        }
    };
    for buttons in [&["button_fn"][..], &["button_shift", "button_fn"][..]] {
        hold(&mut runner, buttons, true);
        let _ = press(&mut runner, 1, 7);
        let _ = press(&mut runner, 6, 1);
        hold(&mut runner, buttons, false);
        assert_eq!(runner.transport.bpm, 120.0, "{buttons:?}");
        assert_eq!(
            (runner.transport.swing_a, runner.transport.swing_b),
            (4, 4),
            "{buttons:?}"
        );
        assert_eq!(
            runner.active_play_mode,
            ActivePlayMode::Page(PlayMode::Tempo)
        );
    }
    let _ = press(&mut runner, 1, 7);
    assert_eq!(runner.transport.bpm, 50.0);
}
