use super::*;

fn input(runner: &mut NativeRunner, input: Value) {
    runner
        .send(HostMessage::DeviceInput {
            input,
            request_snapshot: None,
        })
        .unwrap();
}

fn button(runner: &mut NativeRunner, kind: &str, pressed: bool) {
    input(runner, json!({ "type": kind, "pressed": pressed }));
}

fn press(runner: &mut NativeRunner, x: usize, y: usize) {
    input(runner, json!({ "type": "grid_press", "x": x, "y": y }));
}

fn layer_column(runner: &NativeRunner) -> Vec<Value> {
    let leds = led_tiles(&runner.snapshot().unwrap());
    (0..4).map(|y| leds[display_index(0, y)].clone()).collect()
}

fn colors(rgbs: [[u8; 3]; 4]) -> Vec<Value> {
    rgbs.into_iter().map(led_rgb).collect()
}

const MAGENTA: [u8; 3] = platform_core::palette::MAGENTA;
const BLUE: [u8; 3] = platform_core::palette::BLUE;
const GREEN: [u8; 3] = platform_core::palette::GREEN;

fn runner() -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    for (layer, id) in ["life", "life", "life", "none"].into_iter().enumerate() {
        runner.layer_behavior_ids[layer] = id.into();
    }
    runner
}

#[test]
pub(crate) fn gated_layers_light_magenta_in_both_navigation_views() {
    let mut runner = runner();
    let none = dim_rgb(platform_core::palette::GRAY, 8);

    button(&mut runner, "button_shift", true);
    button(&mut runner, "button_fn", true);
    press(&mut runner, 0, 0);
    press(&mut runner, 0, 2);
    assert_eq!(runner.active_layer_index, 0);
    assert_eq!(
        layer_column(&runner),
        colors([MAGENTA, GREEN, dim_rgb(MAGENTA, 4), none])
    );
    button(&mut runner, "button_shift", false);
    assert_eq!(
        layer_column(&runner),
        colors([MAGENTA, GREEN, dim_rgb(MAGENTA, 4), none])
    );

    press(&mut runner, GRID_WIDTH - 1, 0);
    assert_eq!(runner.active_play_mode, ActivePlayMode::Page(PlayMode::Mix));
    assert_eq!(
        layer_column(&runner),
        colors([MAGENTA, BLUE, MAGENTA, none])
    );
    button(&mut runner, "button_fn", false);

    button(&mut runner, "button_shift", true);
    button(&mut runner, "button_fn", true);
    press(&mut runner, 0, 0);
    button(&mut runner, "button_shift", false);
    press(&mut runner, 0, 1);
    assert_eq!(runner.active_play_mode, ActivePlayMode::None);
    assert_eq!(
        layer_column(&runner),
        colors([GREEN, BLUE, dim_rgb(MAGENTA, 4), none])
    );
}
