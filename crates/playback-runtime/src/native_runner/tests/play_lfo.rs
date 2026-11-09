use super::*;

fn press(runner: &mut NativeRunner, x: usize, y: usize) -> Vec<RunnerMessage> {
    let mut messages = device_input(runner, json!({ "type": "grid_press", "x": x, "y": y }));
    messages.extend(device_input(
        runner,
        json!({ "type": "grid_release", "x": x, "y": y }),
    ));
    messages
}

fn shift_press(runner: &mut NativeRunner, x: usize, y: usize) {
    let _ = device_input(runner, json!({ "type": "button_shift", "pressed": true }));
    let _ = press(runner, x, y);
    let _ = device_input(runner, json!({ "type": "button_shift", "pressed": false }));
}

/// L1 wobbles I1's volume over a bar; L2 has no target.
fn lfo_runner() -> NativeRunner {
    let mut source = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    source.link_lfos[0].enabled = true;
    source.link_lfos[0].period = "1/1".into();
    source.link_lfos[0].target = Some(NativeParamBinding {
        key: "instruments.0.mixer.volume".into(),
        label: Some("Volume".into()),
        kind: "number".into(),
        min: Some(0.0),
        max: Some(100.0),
        step: Some(1.0),
        user_min: None,
        user_max: None,
        options: vec![],
        invert: false,
    });
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner
        .apply_config_payload(source.config_payload())
        .unwrap();
    assert!(runner.link_lfos[0].target.is_some());
    let _ = device_input(&mut runner, json!({ "type": "button_fn", "pressed": true }));
    let opened = press(&mut runner, GRID_WIDTH - 1, 4);
    let _ = device_input(
        &mut runner,
        json!({ "type": "button_fn", "pressed": false }),
    );
    assert_eq!(runner.active_play_mode, ActivePlayMode::Page(PlayMode::Lfo));
    assert_eq!(snapshot_from(&opened)["display"]["toast"], "LFO");
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
fn column_zero_toggles_an_lfo_or_says_it_has_no_target() {
    let mut runner = lfo_runner();

    let _ = press(&mut runner, 0, 0);
    assert_eq!(toast(&runner), "LFO1 off");
    assert!(!runner.link_lfos[0].enabled);
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["linkLfos"][0]["enabled"],
        false
    );
    assert_eq!(
        tile(&runner, 0, 0),
        led_rgb(dim_rgb(platform_core::palette::GRAY, 6))
    );
    let _ = press(&mut runner, 0, 0);
    assert_eq!(toast(&runner), "LFO1 on");
    assert!(runner.link_lfos[0].enabled);

    let _ = press(&mut runner, 0, 1);
    assert_eq!(toast(&runner), "LFO2: no target");
    assert!(!runner.link_lfos[1].enabled);
    assert_eq!(tile(&runner, 0, 1), led_rgb(platform_core::palette::BLACK));
}

#[test]
fn column_zero_follows_the_lfo_value() {
    let mut runner = lfo_runner();
    runner.transport.transport = RuntimeTransportState::Playing;
    let middle = tile(&runner, 0, 0);
    pulse(&mut runner, 24);
    assert_eq!(tile(&runner, 0, 0), led_rgb(platform_core::palette::GREEN));
    pulse(&mut runner, 48);
    assert_eq!(
        tile(&runner, 0, 0),
        led_rgb(dim_rgb(platform_core::palette::GREEN, 8))
    );
    pulse(&mut runner, 24);
    assert_eq!(tile(&runner, 0, 0), middle);
}

#[test]
fn an_lfo_switched_off_for_three_bars_comes_back_in_phrase() {
    let mut runner = lfo_runner();
    runner.transport.transport = RuntimeTransportState::Playing;
    pulse(&mut runner, 24);
    let _ = press(&mut runner, 0, 0);
    assert!(!runner.link_lfos[0].enabled);

    pulse(&mut runner, 3 * 96 + 10);
    let _ = runner.outbox.drain_audio_commands();
    let _ = press(&mut runner, 0, 0);

    assert!(runner.link_lfos[0].enabled);
    assert_eq!(runner.link_lfos[0].phase_pulses, (24 + 3 * 96 + 10) % 96);
    let mut free_running = lfo_runner();
    free_running.transport.transport = RuntimeTransportState::Playing;
    pulse(&mut free_running, 24 + 3 * 96 + 10);
    assert_eq!(
        runner.link_lfos[0].phase_pulses,
        free_running.link_lfos[0].phase_pulses
    );
    assert_eq!(tile(&runner, 0, 0), tile(&free_running, 0, 0));
}

#[test]
fn period_tiles_set_the_period_and_a_second_press_toggles_triplet() {
    let mut runner = lfo_runner();
    assert_eq!(tile(&runner, 4, 0), led_rgb(platform_core::palette::GREEN));

    let _ = press(&mut runner, 2, 0);
    assert_eq!(toast(&runner), "LFO1 period 1/4");
    assert_eq!(runner.link_lfos[0].period, "1/4");
    let _ = press(&mut runner, 2, 0);
    assert_eq!(toast(&runner), "LFO1 period 1/4T");
    assert_eq!(runner.link_lfos[0].period, "1/4T");
    assert_eq!(tile(&runner, 2, 0), led_rgb(platform_core::palette::BLUE));
    let _ = press(&mut runner, 2, 0);
    assert_eq!(runner.link_lfos[0].period, "1/4");

    let _ = press(&mut runner, 7, 0);
    let _ = press(&mut runner, 7, 0);
    assert_eq!(runner.link_lfos[0].period, "8/1T");
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["linkLfos"][0]["period"],
        "8/1T"
    );
    assert_eq!(
        runner.menu.value_for_key("linkLfos.0.period").as_deref(),
        Some("8/1T")
    );
}

#[test]
fn a_period_change_keeps_the_wave_position() {
    let mut runner = lfo_runner();
    runner.transport.transport = RuntimeTransportState::Playing;
    pulse(&mut runner, 24);
    assert_eq!(runner.link_lfos[0].phase_pulses, 24);

    let _ = press(&mut runner, 5, 0);
    assert_eq!(runner.link_lfos[0].period, "2/1");
    assert_eq!(runner.link_lfos[0].phase_pulses, 48, "a quarter of 192");

    let _ = press(&mut runner, 5, 0);
    assert_eq!(runner.link_lfos[0].period, "2/1T");
    assert_eq!(runner.link_lfos[0].phase_pulses, 32, "a quarter of 128");
}

#[test]
fn shift_tiles_set_depth_in_seven_steps() {
    let mut runner = lfo_runner();
    shift_press(&mut runner, 3, 0);
    assert_eq!(toast(&runner), "LFO1 depth 43%");
    assert_eq!(runner.link_lfos[0].depth_pct, 43);
    assert_eq!(
        runner.link_lfos[0].period, "1/1",
        "Shift never touches the period"
    );
    shift_press(&mut runner, 7, 2);
    assert_eq!(runner.link_lfos[2].depth_pct, 100);

    let _ = device_input(
        &mut runner,
        json!({ "type": "button_shift", "pressed": true }),
    );
    assert_eq!(
        tile(&runner, 3, 0),
        led_rgb(platform_core::palette::MAGENTA)
    );
    assert_eq!(
        tile(&runner, 4, 0),
        led_rgb(dim_rgb(platform_core::palette::MAGENTA, 6))
    );
}
