use super::*;

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

fn shift_press(runner: &mut NativeRunner, x: usize, y: usize) {
    let _ = input(runner, json!({ "type": "button_shift", "pressed": true }));
    let _ = press(runner, x, y);
    let _ = input(runner, json!({ "type": "button_shift", "pressed": false }));
}

fn rate_runner() -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.layer_behavior_ids[5] = "none".into();
    for button in ["button_shift", "button_fn"] {
        let _ = input(&mut runner, json!({ "type": button, "pressed": true }));
    }
    let opened = press(&mut runner, GRID_WIDTH - 1, 3);
    for button in ["button_fn", "button_shift"] {
        let _ = input(&mut runner, json!({ "type": button, "pressed": false }));
    }
    assert_eq!(runner.active_play_mode, "rate");
    assert_eq!(snapshot_from(&opened)["display"]["toast"], "Play: rate");
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

fn tile(runner: &NativeRunner, x: usize, y: usize) -> Value {
    led_tiles(&runner.snapshot().unwrap())[display_index(x, y)].clone()
}

fn pulse(runner: &mut NativeRunner, pulses: u32) {
    runner
        .send(HostMessage::TransportPulseStep {
            pulses,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();
}

#[test]
fn rate_tiles_set_each_layers_saved_step_rate() {
    let mut runner = rate_runner();

    let _ = press(&mut runner, 4, 0);
    assert_eq!(toast(&runner), "L1 rate 1/8");
    assert_eq!(runner.transport.algorithm_step_pulses, 12);
    assert_eq!(
        runner.menu.value_for_key("algorithmStep").as_deref(),
        Some("1/8")
    );

    let _ = press(&mut runner, 1, 2);
    assert_eq!(toast(&runner), "L3 rate 1/16T");
    assert_eq!(runner.transport.layer_algorithm_step_pulses[2], 4);
    assert_eq!(runner.transport.algorithm_step_pulses, 12);

    let layers = runner.config_payload()["runtimeConfig"]["layers"].clone();
    assert_eq!(layers[0]["build"]["stepRate"], "1/8");
    assert_eq!(layers[2]["build"]["stepRate"], "1/16T");
}

#[test]
fn rate_leds_light_the_current_rate_and_skip_empty_layers() {
    let mut runner = rate_runner();
    let _ = press(&mut runner, 1, 2);

    assert_eq!(tile(&runner, 1, 2), led_rgb(platform_core::palette::BLUE));
    assert_eq!(
        tile(&runner, 2, 2),
        led_rgb(dim_rgb(platform_core::palette::GREEN, 5))
    );
    assert_eq!(
        tile(&runner, 3, 2),
        led_rgb(dim_rgb(platform_core::palette::BLUE, 5))
    );
    for x in 0..GRID_WIDTH {
        assert_eq!(tile(&runner, x, 5), led_rgb(platform_core::palette::BLACK));
    }

    runner.transport.layer_algorithm_step_pulses[3] = 96;
    for x in 0..GRID_WIDTH {
        assert_ne!(tile(&runner, x, 3), led_rgb(platform_core::palette::GREEN));
        assert_ne!(tile(&runner, x, 3), led_rgb(platform_core::palette::BLUE));
    }

    let before = runner.transport.layer_algorithm_step_pulses[5];
    let _ = press(&mut runner, 0, 5);
    assert_eq!(runner.transport.layer_algorithm_step_pulses[5], before);
}

#[test]
fn shift_press_resyncs_the_layer_to_the_next_bar() {
    let mut runner = rate_runner();
    let _ = press(&mut runner, 6, 1);
    assert_eq!(runner.transport.layer_algorithm_step_pulses[1], 24);
    runner.transport.transport = RuntimeTransportState::Playing;
    pulse(&mut runner, 90);
    runner.transport.layer_pulse_accumulators[1] = 5;
    let ticks = runner.transport.layer_ticks[1];

    shift_press(&mut runner, 2, 1);
    assert_eq!(toast(&runner), "L2 resync");
    assert_eq!(
        runner.transport.layer_algorithm_step_pulses[1], 24,
        "Shift + press does not change the rate"
    );
    pulse(&mut runner, 5);
    assert_eq!(runner.transport.layer_pulse_accumulators[1], 10);

    pulse(&mut runner, 1);
    assert_eq!(runner.transport.current_ppqn_pulse, 96);
    assert_eq!(runner.transport.layer_ticks[1], ticks + 1);
    assert_eq!(runner.transport.layer_pulse_accumulators[1], 0);
    pulse(&mut runner, 24);
    assert_eq!(runner.transport.layer_ticks[1], ticks + 2);
    assert_eq!(runner.transport.layer_pulse_accumulators[1], 0);
}
