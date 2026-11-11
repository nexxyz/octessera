use super::*;
use crate::native_runner::led_color::LedColor;

fn input(runner: &mut NativeRunner, input: Value) {
    runner
        .send(HostMessage::DeviceInput {
            input,
            request_snapshot: None,
        })
        .unwrap();
}

fn press(runner: &mut NativeRunner, x: usize, y: usize) {
    input(runner, json!({ "type": "grid_press", "x": x, "y": y }));
    input(runner, json!({ "type": "grid_release", "x": x, "y": y }));
}

fn gate_runner() -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    for button in ["button_shift", "button_fn"] {
        input(&mut runner, json!({ "type": button, "pressed": true }));
    }
    press(&mut runner, GRID_WIDTH - 1, 4);
    for button in ["button_fn", "button_shift"] {
        input(&mut runner, json!({ "type": button, "pressed": false }));
    }
    assert_eq!(
        runner.active_play_mode,
        ActivePlayMode::Page(PlayMode::TriggerGate)
    );
    runner
}

fn modes(runner: &NativeRunner) -> Vec<&str> {
    runner
        .layers
        .iter()
        .map(|layer| &layer.link)
        .map(|layer| layer.trigger_probability_mode.wire_name())
        .collect()
}

#[test]
fn bottom_row_right_tiles_set_every_layer() {
    let mut runner = gate_runner();
    for (x, mode) in [(5, "zero"), (6, "custom"), (7, "full"), (5, "zero")] {
        press(&mut runner, x, 0);
        assert_eq!(modes(&runner), vec![mode; GRID_HEIGHT], "tile ({x}, 0)");
        assert!(runner
            .layers
            .iter()
            .map(|layer| &layer.link.trigger_probability_mode)
            .all(|value| value.wire_name() == mode));
    }
}

#[test]
fn left_columns_set_one_layer_and_unlit_tiles_do_nothing() {
    let mut runner = gate_runner();
    press(&mut runner, 0, 3);
    let expected = modes(&runner)
        .iter()
        .map(|mode| mode.to_string())
        .collect::<Vec<_>>();
    assert_eq!(expected[3], "zero");
    assert_eq!(expected.iter().filter(|mode| *mode == "zero").count(), 1);
    for (x, y) in [(3, 0), (4, 0), (3, 2), (5, 3), (6, 1), (7, 7)] {
        press(&mut runner, x, y);
        assert_eq!(modes(&runner), expected, "tile ({x}, {y})");
    }
}

#[test]
fn the_page_shows_each_layer_mode_a_loaded_patch_saved() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["layers"][4]["link"]["triggerProbabilityMode"] = json!("zero");
    payload["runtimeConfig"]["layers"][1]["link"]["triggerProbabilityMode"] = json!("custom");
    runner.apply_config_payload(payload).unwrap();
    for button in ["button_shift", "button_fn"] {
        input(&mut runner, json!({ "type": button, "pressed": true }));
    }
    press(&mut runner, GRID_WIDTH - 1, 4);
    for button in ["button_fn", "button_shift"] {
        input(&mut runner, json!({ "type": button, "pressed": false }));
    }
    let leds = led_tiles(&runner.snapshot().unwrap());
    let led = |x: usize, y: usize| {
        let tile = &leds[display_index(x, y)];
        [tile["r"].as_u64(), tile["g"].as_u64(), tile["b"].as_u64()]
            .map(|channel| u8::try_from(channel.unwrap()).unwrap())
    };
    let bright = |color: LedColor| [color.r, color.g, color.b];
    assert_eq!(led(0, 4), bright(LedColor::MAGENTA), "L5 sleeps at 0%");
    assert_eq!(led(1, 1), bright(LedColor::YELLOW), "L2 plays custom");
    assert_eq!(led(2, 0), bright(LedColor::GREEN), "L1 plays 100%");
}
