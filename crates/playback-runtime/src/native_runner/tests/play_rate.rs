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

fn shift(runner: &mut NativeRunner, pressed: bool) {
    let _ = input(
        runner,
        json!({ "type": "button_shift", "pressed": pressed }),
    );
}

fn shift_press(runner: &mut NativeRunner, x: usize, y: usize) {
    shift(runner, true);
    let _ = press(runner, x, y);
    shift(runner, false);
}

fn set_menu(runner: &mut NativeRunner, key: &str, value: &str) {
    runner.menu.set_enum_value_for_key(key, value);
    runner.apply_or_schedule_menu_key(key).unwrap();
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

fn pulse(runner: &mut NativeRunner, pulses: u32) -> Vec<RunnerMessage> {
    runner
        .send(HostMessage::TransportPulseStep {
            pulses,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap()
}

#[test]
fn rate_tiles_set_each_layers_saved_step_rate() {
    let mut runner = rate_runner();

    let _ = press(&mut runner, 6, 0);
    assert_eq!(toast(&runner), "L1 rate 1/4");
    assert_eq!(runner.transport.algorithm_step_pulses, 24);
    assert_eq!(
        runner.menu.value_for_key("algorithmStep").as_deref(),
        Some("1/4")
    );

    let _ = press(&mut runner, 1, 2);
    assert_eq!(toast(&runner), "L3 rate 1/16T");
    assert_eq!(runner.transport.layer_algorithm_step_pulses[2], 4);
    assert_eq!(runner.transport.algorithm_step_pulses, 24);

    let layers = runner.config_payload()["runtimeConfig"]["layers"].clone();
    assert_eq!(layers[0]["build"]["stepRate"], "1/4");
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
fn shift_view_shows_scan_units_and_dims_layers_that_are_not_scanning() {
    let mut runner = rate_runner();
    set_menu(&mut runner, "layers.2.link.scanMode", "scanning");
    let _ = press(&mut runner, 4, 2);

    shift(&mut runner, true);
    assert_eq!(tile(&runner, 2, 2), led_rgb(platform_core::palette::WHITE));
    assert_eq!(
        tile(&runner, 4, 2),
        led_rgb(dim_rgb(platform_core::palette::WHITE, 5))
    );
    assert_eq!(
        tile(&runner, 3, 2),
        led_rgb(dim_rgb(platform_core::palette::BLUE, 5))
    );
    for x in 0..GRID_WIDTH {
        assert_ne!(tile(&runner, x, 1), led_rgb(platform_core::palette::WHITE));
        assert_ne!(tile(&runner, x, 1), led_rgb(platform_core::palette::GREEN));
        assert_eq!(tile(&runner, x, 5), led_rgb(platform_core::palette::BLACK));
    }

    shift(&mut runner, false);
    assert_eq!(tile(&runner, 4, 2), led_rgb(platform_core::palette::GREEN));
    assert_eq!(
        tile(&runner, 2, 2),
        led_rgb(dim_rgb(platform_core::palette::GREEN, 5))
    );
}

#[test]
fn shift_press_sets_the_saved_scan_unit_and_leaves_step_rate() {
    let mut runner = rate_runner();
    set_menu(&mut runner, "layers.2.link.scanMode", "scanning");
    let step_rate = runner.transport.layer_algorithm_step_pulses[2];
    runner.pending.pending_autosave_payload_due_at = None;

    shift_press(&mut runner, 5, 2);
    assert_eq!(toast(&runner), "L3 scan 1/4T");
    assert_eq!(runner.link_layers[2].scan_unit, "1/4T");
    assert_eq!(runner.step_pulses_for_layer(2), 16);
    assert_eq!(
        runner
            .menu
            .value_for_key("layers.2.link.scanUnit")
            .as_deref(),
        Some("1/4T")
    );
    assert_eq!(runner.transport.layer_algorithm_step_pulses[2], step_rate);
    assert!(runner.pending.pending_autosave_payload_due_at.is_some());
    assert!(!runner.play_live.rate_resyncs[2]);
    let layers = runner.config_payload()["runtimeConfig"]["layers"].clone();
    assert_eq!(layers[2]["link"]["scanUnit"], "1/4T");
}

#[test]
fn shift_press_on_a_layer_that_is_not_scanning_changes_nothing() {
    let mut runner = rate_runner();
    let step_rate = runner.transport.layer_algorithm_step_pulses[2];

    shift_press(&mut runner, 5, 2);
    assert_eq!(toast(&runner), "L3: not scanning");
    assert_eq!(runner.link_layers[2].scan_mode, "none");
    assert_eq!(runner.link_layers[2].scan_unit, "1/16");
    assert_eq!(runner.transport.layer_algorithm_step_pulses[2], step_rate);
    assert!(!runner.play_live.rate_resyncs[2]);
}

#[test]
fn shift_press_on_another_tile_no_longer_resyncs() {
    let mut runner = rate_runner();
    let _ = press(&mut runner, 6, 1);
    shift_press(&mut runner, 2, 1);
    assert!(!runner.play_live.rate_resyncs[1]);
    assert_eq!(runner.transport.layer_algorithm_step_pulses[1], 24);
}

#[test]
fn pressing_the_lit_step_rate_tile_resyncs_the_layer_to_the_next_bar() {
    let mut runner = rate_runner();
    let _ = press(&mut runner, 6, 1);
    assert_eq!(runner.transport.layer_algorithm_step_pulses[1], 24);
    runner.transport.transport = RuntimeTransportState::Playing;
    let _ = pulse(&mut runner, 90);
    runner.transport.layer_pulse_accumulators[1] = 5;
    assert_ne!(runner.transport.layer_ticks[1], 0);

    let _ = press(&mut runner, 6, 1);
    assert_eq!(toast(&runner), "L2 resync");
    assert_eq!(runner.transport.layer_algorithm_step_pulses[1], 24);
    let _ = pulse(&mut runner, 5);
    assert_eq!(runner.transport.layer_pulse_accumulators[1], 10);

    let _ = pulse(&mut runner, 1);
    assert_eq!(runner.transport.current_ppqn_pulse, 96);
    assert_eq!(runner.transport.layer_ticks[1], 1);
    assert_eq!(runner.transport.layer_pulse_accumulators[1], 0);
    let _ = pulse(&mut runner, 24);
    assert_eq!(runner.transport.layer_ticks[1], 2);
    assert_eq!(runner.transport.layer_pulse_accumulators[1], 0);
}

fn scanning_rows_runner() -> NativeRunner {
    let mut runner = rate_runner();
    let link = &mut runner.link_layers[0];
    link.scan_mode = "scanning".into();
    link.scan_axis = "rows".into();
    link.scan_unit = "1/8T".into();
    link.event_enabled = false;
    link.scanned_slot = 0;
    link.scanned_action = "note_on".into();
    link.scanned_empty_slot = 0;
    link.scanned_empty_action = "note_on".into();
    link.y_pitch_enabled = true;
    link.y_pitch_steps = 1;
    runner.refresh_active_mapping_config();
    runner.refresh_active_interpretation_profile();
    runner
        .engine
        .set_interpretation_profile(runner.interpretation_profile.clone());
    runner.transport.transport = RuntimeTransportState::Playing;
    runner
}

#[test]
fn pressing_the_lit_tile_in_either_view_restarts_the_scan_on_the_next_bar() {
    for scan_view in [false, true] {
        let mut runner = scanning_rows_runner();
        let lit = if scan_view { 3 } else { 4 };
        let origin_notes = musical_note_ons(&pulse(&mut runner, 8));
        assert!(!origin_notes.is_empty());
        let _ = pulse(&mut runner, 82);

        shift(&mut runner, scan_view);
        let _ = press(&mut runner, lit, 0);
        shift(&mut runner, false);
        assert_eq!(toast(&runner), "L1 resync", "scan view {scan_view}");
        assert_eq!(runner.link_layers[0].scan_unit, "1/8T");
        assert_eq!(runner.transport.algorithm_step_pulses, 12);

        let _ = pulse(&mut runner, 5);
        let bar_notes = musical_note_ons(&pulse(&mut runner, 1));
        assert_eq!(runner.transport.current_ppqn_pulse, 96);
        assert_eq!(bar_notes, origin_notes, "scan view {scan_view}");
        assert_eq!(runner.transport.layer_ticks[0], 1);
        assert_eq!(runner.transport.layer_pulse_accumulators[0], 0);
        let _ = pulse(&mut runner, 8);
        assert_eq!(runner.transport.layer_ticks[0], 2);
    }
}

#[test]
fn stop_and_clear_all_drop_a_pending_resync() {
    let mut runner = rate_runner();
    let _ = press(&mut runner, 6, 1);
    let play = |runner: &mut NativeRunner| {
        for pressed in [true, false] {
            let _ = input(runner, json!({ "type": "button_s", "pressed": pressed }));
        }
    };
    play(&mut runner);
    let _ = pulse(&mut runner, 50);
    let _ = press(&mut runner, 6, 1);
    assert!(runner.play_live.rate_resyncs[1]);

    shift(&mut runner, true);
    play(&mut runner);
    shift(&mut runner, false);
    assert_eq!(runner.transport.transport, RuntimeTransportState::Stopped);
    play(&mut runner);
    let _ = pulse(&mut runner, 100);
    assert!(
        runner.transport.layer_ticks[1] >= 4,
        "a stale resync restarted the layer at the bar: {}",
        runner.transport.layer_ticks[1]
    );

    let _ = press(&mut runner, 6, 1);
    runner.clear_patch_state().unwrap();
    assert!(!runner.play_live.rate_resyncs.contains(&true));
}
