use super::*;

fn input(runner: &mut NativeRunner, input: Value) -> Vec<RunnerMessage> {
    runner
        .send(HostMessage::DeviceInput {
            input,
            request_snapshot: None,
        })
        .unwrap()
}

fn shift(runner: &mut NativeRunner, pressed: bool) {
    let _ = input(
        runner,
        json!({ "type": "button_shift", "pressed": pressed }),
    );
}

fn shift_press(runner: &mut NativeRunner, x: usize, y: usize) -> Vec<RunnerMessage> {
    shift(runner, true);
    let mut messages = input(runner, json!({ "type": "grid_press", "x": x, "y": y }));
    messages.extend(input(
        runner,
        json!({ "type": "grid_release", "x": x, "y": y }),
    ));
    shift(runner, false);
    messages
}

fn mix_runner() -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let _ = input(&mut runner, json!({ "type": "button_fn", "pressed": true }));
    let _ = input(&mut runner, json!({ "type": "grid_press", "x": 7, "y": 0 }));
    let _ = input(
        &mut runner,
        json!({ "type": "button_fn", "pressed": false }),
    );
    assert_eq!(runner.active_play_mode, "mix");
    let _ = runner.messages_with_snapshot().unwrap();
    runner.config_dirty = false;
    runner.dirty_revision = None;
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

/// Every volume the engine receives for `bus`, from mixer and full config commands.
fn engine_bus_volumes(messages: &[RunnerMessage], bus: usize) -> Vec<f64> {
    messages
        .iter()
        .filter_map(|message| match message {
            RunnerMessage::AudioCommands { commands } => Some(commands),
            _ => None,
        })
        .flatten()
        .filter_map(|command| match command {
            RuntimeAudioCommand::SetFxBusMixer {
                bus_index,
                volume_pct: Some(volume),
                ..
            } if *bus_index == bus => Some(f64::from(*volume)),
            RuntimeAudioCommand::SetAudioConfig { config, .. } => {
                config["mixer"]["buses"][bus]["volumePct"].as_f64()
            }
            _ => None,
        })
        .collect()
}

#[test]
fn shift_view_sets_bus_and_master_volumes_through_their_menu_rows() {
    let mut runner = mix_runner();

    let messages = shift_press(&mut runner, 1, 4);
    assert_eq!(runner.fx_buses[1].volume_pct, 57);
    assert_eq!(engine_bus_volumes(&messages, 1), vec![57.0]);
    assert_eq!(runner.menu.number_for_key("mixer.buses.1.volume"), Some(57));
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["mixer"]["buses"][1]["volumePct"],
        57
    );

    let _ = shift_press(&mut runner, 5, 7);
    assert_eq!(runner.display.ui.master_volume, 100);
    let _ = shift_press(&mut runner, 5, 2);
    assert_eq!(runner.display.ui.master_volume, 29);

    let before = runner.config_payload();
    for (x, y) in [(4, 3), (6, 3), (7, 5), (5, 0)] {
        let _ = shift_press(&mut runner, x, y);
    }
    assert_eq!(runner.config_payload(), before, "free tiles do nothing");
    assert_eq!(
        runner.instruments[0].volume,
        before["runtimeConfig"]["instruments"][0]["mixer"]["volume"]
    );
}

#[test]
fn shift_view_shows_buses_in_pan_colors_and_master_in_white() {
    let mut runner = mix_runner();
    runner.fx_buses[0].volume_pct = 100;
    runner.fx_buses[2].volume_pct = 0;
    runner.display.ui.master_volume = 43;
    shift(&mut runner, true);

    assert_eq!(tile(&runner, 0, 0), led_rgb(platform_core::palette::GREEN));
    assert_eq!(
        tile(&runner, 0, 7),
        led_rgb(platform_core::palette::MAGENTA)
    );
    assert_eq!(tile(&runner, 2, 1), led_rgb(platform_core::palette::GREEN));
    assert_eq!(tile(&runner, 5, 3), led_rgb(platform_core::palette::WHITE));
    for (x, y) in [(4, 0), (4, 4), (5, 0), (6, 2), (7, 7)] {
        let free = tile(&runner, x, y);
        for channel in ["r", "g", "b"] {
            assert!(
                free[channel].as_u64().unwrap() < 64,
                "({x},{y}) only shows the dimmed behavior: {free}"
            );
        }
    }

    shift(&mut runner, false);
    assert_eq!(
        tile(&runner, 4, 0),
        led_rgb(platform_core::palette::GREEN),
        "releasing Shift returns to the instruments"
    );
}

#[test]
fn bus_mute_is_live_only_and_gates_every_bus_volume() {
    let mut runner = mix_runner();
    let _ = shift_press(&mut runner, 0, 5);
    let _ = runner.messages_with_snapshot().unwrap();
    runner.config_dirty = false;
    runner.dirty_revision = None;
    let saved = runner.config_payload();

    let muted = shift_press(&mut runner, 0, 0);
    assert_eq!(toast(&runner), "B1 muted");
    assert_eq!(engine_bus_volumes(&muted, 0), vec![0.0]);
    shift(&mut runner, true);
    assert_eq!(
        tile(&runner, 0, 0),
        led_rgb(platform_core::palette::BEHAVIOR_PRIMARY_RED)
    );
    assert_eq!(
        tile(&runner, 0, 5),
        led_rgb(dim_rgb(platform_core::palette::MAGENTA, 4))
    );
    shift(&mut runner, false);

    let louder = shift_press(&mut runner, 0, 7);
    assert_eq!(runner.fx_buses[0].volume_pct, 100);
    assert_eq!(engine_bus_volumes(&louder, 0), vec![0.0]);
    let _ = runner.messages_with_snapshot().unwrap();
    runner.config_dirty = false;
    runner.dirty_revision = None;

    let unmuted = shift_press(&mut runner, 0, 0);
    assert_eq!(toast(&runner), "B1 unmuted");
    assert_eq!(engine_bus_volumes(&unmuted, 0), vec![100.0]);
    assert!(!runner.config_dirty);
    let mut after = runner.config_payload();
    after["runtimeConfig"]["mixer"]["buses"][0]["volumePct"] =
        saved["runtimeConfig"]["mixer"]["buses"][0]["volumePct"].clone();
    assert_eq!(after["runtimeConfig"], saved["runtimeConfig"]);
}

#[test]
fn patch_load_and_clear_all_unmute_buses() {
    let mut runner = mix_runner();
    let _ = shift_press(&mut runner, 2, 0);
    assert!(runner.bus_muted(2));
    let payload = runner.config_payload();
    runner.apply_config_payload(payload).unwrap();
    let loaded = runner.messages_with_snapshot().unwrap();
    assert!(!runner.bus_muted(2));
    let volume = f64::from(runner.fx_buses[2].volume_pct);
    assert_eq!(engine_bus_volumes(&loaded, 2).last(), Some(&volume));

    let _ = shift_press(&mut runner, 3, 0);
    assert!(runner.bus_muted(3));
    runner.clear_patch_state().unwrap();
    assert!(!runner.bus_muted(3));
}
