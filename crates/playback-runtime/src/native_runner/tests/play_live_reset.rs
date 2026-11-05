use super::*;

fn live_runner() -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner
        .menu
        .set_enum_value_for_key("play.xy.release", "reset-center");
    runner
        .apply_or_schedule_menu_key("play.xy.release")
        .unwrap();
    runner
}

fn play_live(runner: &mut NativeRunner) {
    runner.play_live.instrument_mutes[0] = true;
    runner.play_live.bus_mutes[1] = true;
    runner.handle_play_route_press(0, 5);
    assert_ne!(runner.play_live.fx_overrides, Default::default());
    runner.play_live.xy_latched = true;
    runner.xy_touch.display_x = 0.9;
    runner.play_live.rate_resyncs[2] = true;
    runner.play_live.tempo_taps.push(Instant::now());
}

fn assert_live_play_ended(runner: &NativeRunner, load: &str) {
    let live = &runner.play_live;
    assert!(!live.instrument_mutes.contains(&true), "{load}");
    assert!(!live.bus_mutes.contains(&true), "{load}");
    assert_eq!(live.fx_overrides, Default::default(), "{load}");
    assert!(!live.xy_latched, "{load}");
    assert_eq!(runner.xy_touch.display_x, 0.5, "{load}: the latch let go");
    assert!(!live.rate_resyncs.contains(&true), "{load}");
    assert!(live.tempo_taps.is_empty(), "{load}");
}

#[test]
fn every_patch_load_and_clear_all_end_all_live_play_state() {
    let mut runner = live_runner();
    play_live(&mut runner);
    let same = runner.config_payload();
    runner.apply_config_payload(same).unwrap();
    assert_live_play_ended(&runner, "unchanged load");

    play_live(&mut runner);
    let mut changed = runner.config_payload();
    changed["runtimeConfig"]["instruments"][0]["mixer"]["volume"] = json!(42);
    runner.apply_config_payload(changed).unwrap();
    assert_eq!(runner.instruments[0].volume, 42);
    assert_live_play_ended(&runner, "changed load");

    play_live(&mut runner);
    runner.clear_patch_state().unwrap();
    assert_live_play_ended(&runner, "Clear All");
}
