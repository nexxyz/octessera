use super::*;

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
    assert_eq!(runner.active_play_mode, "trigger-gate");
    runner
}

fn modes(runner: &NativeRunner) -> Vec<&str> {
    runner
        .link_layers
        .iter()
        .map(|layer| layer.trigger_probability_mode.as_str())
        .collect()
}

#[test]
fn bottom_row_right_tiles_set_every_layer() {
    let mut runner = gate_runner();
    for (x, mode) in [(5, "zero"), (6, "custom"), (7, "full"), (5, "zero")] {
        press(&mut runner, x, 0);
        assert_eq!(modes(&runner), vec![mode; GRID_HEIGHT], "tile ({x}, 0)");
        assert!(runner.trigger_gate_modes.iter().all(|value| value == mode));
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
