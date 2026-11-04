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

/// Bus 1 runs a reverb, bus 2 a duck keyed from I1, global slot 1 a compressor, I8 is MIDI.
fn route_runner() -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut payload = runner.config_payload();
    let mixer = &mut payload["runtimeConfig"]["mixer"];
    mixer["buses"][0]["slot1"] = json!({ "type": "reverb", "params": fx_default_params("reverb") });
    let mut duck = fx_default_params("duck");
    duck["source"] = json!("I1");
    duck["sourceTap"] = json!("post");
    mixer["buses"][1]["slot1"] = json!({ "type": "duck", "params": duck });
    mixer["master"]["slots"][0] =
        json!({ "type": "compressor", "params": fx_default_params("compressor") });
    payload["runtimeConfig"]["instruments"][7]["type"] = json!("midi");
    runner.apply_config_payload(payload).unwrap();
    let _ = runner.messages_with_snapshot().unwrap();
    let _ = input(&mut runner, json!({ "type": "button_fn", "pressed": true }));
    let opened = press(&mut runner, GRID_WIDTH - 1, 2);
    let _ = input(
        &mut runner,
        json!({ "type": "button_fn", "pressed": false }),
    );
    assert_eq!(runner.active_play_mode, "route");
    assert_eq!(snapshot_from(&opened)["display"]["toast"], "Play: route");
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

fn audio_commands(messages: &[RunnerMessage]) -> Vec<RuntimeAudioCommand> {
    messages
        .iter()
        .filter_map(|message| match message {
            RunnerMessage::AudioCommands { commands } => Some(commands.clone()),
            _ => None,
        })
        .flatten()
        .collect()
}

/// `(type, params)` of the bus slot the engine last received, from slot or full config commands.
fn engine_bus_slot(messages: &[RunnerMessage], bus: usize, slot: usize) -> Option<(String, Value)> {
    audio_commands(messages)
        .into_iter()
        .rev()
        .find_map(|command| match command {
            RuntimeAudioCommand::SetFxBusSlot {
                bus_index,
                slot_index,
                fx_type,
                params,
                ..
            } if bus_index == bus && slot_index == slot => Some((fx_type, json!(params))),
            RuntimeAudioCommand::SetAudioConfig { config, .. } => {
                let slot = &config["mixer"]["buses"][bus][format!("slot{}", slot + 1)];
                Some((slot["type"].as_str()?.to_string(), slot["params"].clone()))
            }
            _ => None,
        })
}

fn engine_global_slot(messages: &[RunnerMessage], slot: usize) -> Option<String> {
    audio_commands(messages)
        .into_iter()
        .rev()
        .find_map(|command| match command {
            RuntimeAudioCommand::SetGlobalFxSlot {
                slot_index,
                fx_type,
                ..
            } if slot_index == slot => Some(fx_type),
            _ => None,
        })
}

#[test]
fn route_rows_set_the_saved_route_without_moving_the_menu() {
    let mut runner = route_runner();

    let _ = press(&mut runner, 1, 2);
    assert_eq!(runner.instruments[1].route, "fx_bus_2");
    assert_eq!(toast(&runner), "I2 B2");
    assert_eq!(
        runner
            .menu
            .value_for_key("instruments.1.mixer.route")
            .as_deref(),
        Some("fx_bus_2")
    );
    assert_eq!(runner.menu.current_key(), Some("play.page.route"));
    assert!(runner.config_dirty);
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["instruments"][1]["mixer"]["route"],
        "fx_bus_2"
    );
    assert_eq!(tile(&runner, 1, 2), led_rgb(platform_core::palette::BLUE));
    assert_eq!(
        tile(&runner, 1, 0),
        led_rgb(dim_rgb(platform_core::palette::WHITE, 5))
    );

    let _ = press(&mut runner, 1, 0);
    assert_eq!(runner.instruments[1].route, "direct");
    assert_eq!(toast(&runner), "I2 direct");
}

#[test]
fn midi_slots_are_dark_and_ignore_route_and_duck_presses() {
    let mut runner = route_runner();
    for y in [0, 3, 7] {
        assert_eq!(tile(&runner, 7, y), led_rgb(platform_core::palette::BLACK));
    }
    let _ = press(&mut runner, 7, 3);
    let _ = press(&mut runner, 7, 7);
    assert_eq!(runner.instruments[7].route, "direct");
    assert_eq!(runner.play_live.fx_overrides, Default::default());
}

#[test]
fn bus_bypass_is_live_only_and_gates_every_slot_command() {
    let mut runner = route_runner();
    assert_eq!(
        tile(&runner, 0, 5),
        led_rgb(platform_core::palette::MAGENTA)
    );

    let bypassed = press(&mut runner, 0, 5);
    assert_eq!(toast(&runner), "B1 FX bypassed");
    assert_eq!(
        engine_bus_slot(&bypassed, 0, 0).map(|(fx_type, _)| fx_type),
        Some("none".into())
    );
    assert_eq!(
        tile(&runner, 0, 5),
        led_rgb(dim_rgb(platform_core::palette::MAGENTA, 6))
    );
    assert_eq!(runner.fx_buses[0].slot1_type, "reverb");
    assert!(!runner.config_dirty);

    let mut rerouted = press(&mut runner, 2, 1);
    assert_eq!(runner.instruments[2].route, "fx_bus_1");
    rerouted.extend(runner.messages_with_snapshot().unwrap());
    assert!(audio_commands(&rerouted)
        .iter()
        .any(|command| matches!(command, RuntimeAudioCommand::SetAudioConfig { .. })));
    assert_eq!(
        engine_bus_slot(&rerouted, 0, 0).map(|(fx_type, _)| fx_type),
        Some("none".into()),
        "a full config keeps the bypass"
    );

    let restored = press(&mut runner, 0, 5);
    assert_eq!(toast(&runner), "B1 FX on");
    assert_eq!(
        engine_bus_slot(&restored, 0, 0).map(|(fx_type, _)| fx_type),
        Some("reverb".into())
    );
}

#[test]
fn global_fx_bypass_passes_its_slot_through() {
    let mut runner = route_runner();
    assert_eq!(tile(&runner, 4, 5), led_rgb(platform_core::palette::WHITE));
    assert_eq!(tile(&runner, 6, 5), led_rgb(platform_core::palette::BLACK));

    let bypassed = press(&mut runner, 4, 5);
    assert_eq!(toast(&runner), "Global FX 1 bypassed");
    assert_eq!(engine_global_slot(&bypassed, 0), Some("none".into()));
    assert_eq!(runner.global_fx_slots[0], "compressor");

    let restored = press(&mut runner, 4, 5);
    assert_eq!(toast(&runner), "Global FX 1 on");
    assert_eq!(engine_global_slot(&restored, 0), Some("compressor".into()));
}

#[test]
fn duck_key_points_every_duck_at_one_instrument_until_pressed_again() {
    let mut runner = route_runner();

    let keyed = press(&mut runner, 2, 7);
    assert_eq!(toast(&runner), "Duck key: I3");
    let (fx_type, params) = engine_bus_slot(&keyed, 1, 0).unwrap();
    assert_eq!(fx_type, "duck");
    assert_eq!(params["source"], "I3");
    assert_eq!(params["sourceTap"], "post");
    assert_eq!(
        tile(&runner, 2, 7),
        led_rgb(platform_core::palette::BEHAVIOR_PRIMARY_RED)
    );
    assert!(!runner.config_dirty);
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["mixer"]["buses"][1]["slot1"]["params"]["source"],
        "I1"
    );

    let moved = press(&mut runner, 4, 7);
    assert_eq!(toast(&runner), "Duck key: I5");
    assert_eq!(engine_bus_slot(&moved, 1, 0).unwrap().1["source"], "I5");

    let released = press(&mut runner, 4, 7);
    assert_eq!(toast(&runner), "Duck key off");
    let (_, params) = engine_bus_slot(&released, 1, 0).unwrap();
    assert_eq!(params["source"], "I1");
    assert_eq!(params["sourceTap"], "post");
}

#[test]
fn patch_load_clears_bypass_and_duck_key() {
    let mut runner = route_runner();
    let _ = press(&mut runner, 0, 5);
    let _ = press(&mut runner, 2, 7);
    let payload = runner.config_payload();

    runner.apply_config_payload(payload).unwrap();
    let loaded = runner.messages_with_snapshot().unwrap();

    assert_eq!(runner.play_live.fx_overrides, Default::default());
    assert_eq!(
        engine_bus_slot(&loaded, 0, 0).map(|(fx_type, _)| fx_type),
        Some("reverb".into())
    );
    assert_eq!(engine_bus_slot(&loaded, 1, 0).unwrap().1["source"], "I1");
}

#[test]
fn duck_key_on_a_slot_that_turned_midi_stays_red_and_clears_on_press() {
    let mut runner = route_runner();
    let _ = press(&mut runner, 2, 7);
    assert!(runner
        .menu
        .set_enum_value_for_key("instruments.2.type", "midi"));
    runner
        .apply_or_schedule_menu_key("instruments.2.type")
        .unwrap();
    assert_eq!(runner.instruments[2].kind, "midi");
    assert_eq!(
        tile(&runner, 2, 7),
        led_rgb(platform_core::palette::BEHAVIOR_PRIMARY_RED)
    );

    let released = press(&mut runner, 2, 7);
    assert_eq!(toast(&runner), "Duck key off");
    assert_eq!(engine_bus_slot(&released, 1, 0).unwrap().1["source"], "I1");
    assert_eq!(tile(&runner, 2, 7), led_rgb(platform_core::palette::BLACK));
    let _ = press(&mut runner, 2, 7);
    assert_eq!(runner.play_live.fx_overrides, Default::default());
}
