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

/// Bus 1 runs a reverb, buses 2 and 3 a duck keyed from I1, global slot 1 a compressor, I8 is MIDI.
fn route_runner() -> NativeRunner {
    route_runner_with_ducks(&[(1, 1, "I1", "post"), (2, 2, "I1", "pre")])
}

/// The Route page with a duck `(bus, slot, source, sourceTap)` per entry beside the reverb.
fn route_runner_with_ducks(ducks: &[(usize, usize, &str, &str)]) -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut payload = runner.config_payload();
    let mixer = &mut payload["runtimeConfig"]["mixer"];
    mixer["buses"][0]["slot1"] = json!({ "type": "reverb", "params": fx_default_params("reverb") });
    for (bus, slot, source, tap) in ducks {
        let mut duck = fx_default_params("duck");
        duck["source"] = json!(source);
        duck["sourceTap"] = json!(tap);
        mixer["buses"][*bus][format!("slot{slot}")] = json!({ "type": "duck", "params": duck });
    }
    mixer["master"]["slots"][0] =
        json!({ "type": "compressor", "params": fx_default_params("compressor") });
    payload["runtimeConfig"]["instruments"][7]["type"] = json!("midi");
    runner.apply_config_payload(payload).unwrap();
    let _ = runner.messages_with_snapshot().unwrap();
    open_route_page(&mut runner);
    runner.config_dirty = false;
    runner.dirty_revision = None;
    runner
}

fn open_route_page(runner: &mut NativeRunner) {
    let _ = input(runner, json!({ "type": "button_fn", "pressed": true }));
    let opened = press(runner, GRID_WIDTH - 1, 2);
    let _ = input(runner, json!({ "type": "button_fn", "pressed": false }));
    assert_eq!(
        runner.active_play_mode,
        ActivePlayMode::Page(PlayMode::Route)
    );
    assert_eq!(snapshot_from(&opened)["display"]["toast"], "Play: Route");
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
    assert_eq!(duck_source(&runner, 1, 1), "I1");
    assert!(!runner.config_dirty);
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
    assert_eq!(toast(&runner), "G1 FX bypassed");
    assert_eq!(engine_global_slot(&bypassed, 0), Some("none".into()));
    assert_eq!(runner.global_fx_slots[0], "compressor");

    let restored = press(&mut runner, 4, 5);
    assert_eq!(toast(&runner), "G1 FX on");
    assert_eq!(engine_global_slot(&restored, 0), Some("compressor".into()));
}

fn duck_source(runner: &NativeRunner, bus: usize, slot: usize) -> String {
    runner.config_payload()["runtimeConfig"]["mixer"]["buses"][bus][format!("slot{slot}")]["params"]
        ["source"]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

fn load_duck_sources(runner: &mut NativeRunner, ducks: &[(usize, usize, &str)]) {
    let mut payload = runner.config_payload();
    for (bus, slot, source) in ducks {
        payload["runtimeConfig"]["mixer"]["buses"][*bus][format!("slot{slot}")]["params"]
            ["source"] = json!(source);
    }
    runner.apply_config_payload(payload).unwrap();
    let _ = runner.messages_with_snapshot().unwrap();
}

fn red(divisor: u8) -> Value {
    led_rgb(dim_rgb(
        platform_core::palette::BEHAVIOR_PRIMARY_RED,
        divisor,
    ))
}

#[test]
fn patch_load_clears_bus_bypass() {
    let mut runner = route_runner();
    let _ = press(&mut runner, 0, 5);
    let payload = runner.config_payload();

    runner.apply_config_payload(payload).unwrap();
    let loaded = runner.messages_with_snapshot().unwrap();

    assert_eq!(runner.play_live.fx_overrides, Default::default());
    assert_eq!(
        engine_bus_slot(&loaded, 0, 0).map(|(fx_type, _)| fx_type),
        Some("reverb".into())
    );
}

#[test]
fn duck_source_press_saves_the_instrument_on_every_bus_duck() {
    let mut runner = route_runner();
    assert_eq!(tile(&runner, 0, 7), red(1));
    assert_eq!(tile(&runner, 2, 7), red(8));

    let keyed = press(&mut runner, 2, 7);
    assert_eq!(toast(&runner), "Duck source: I3 (2 ducks)");
    let (fx_type, params) = engine_bus_slot(&keyed, 1, 0).unwrap();
    assert_eq!(fx_type, "duck");
    assert_eq!(params["source"], "I3");
    assert_eq!(params["sourceTap"], "post");
    assert_eq!(engine_bus_slot(&keyed, 2, 1).unwrap().1["sourceTap"], "pre");
    assert!(runner.config_dirty);
    assert!(runner.pending.pending_autosave_payload_due_at.is_some());
    assert_eq!(duck_source(&runner, 1, 1), "I3");
    assert_eq!(duck_source(&runner, 2, 2), "I3");
    for key in [
        "mixer.buses.1.slot1.params.source",
        "mixer.buses.2.slot2.params.source",
    ] {
        assert_eq!(runner.menu.value_for_key(key).as_deref(), Some("I3"));
    }
    assert_eq!(tile(&runner, 2, 7), red(1));
    assert_eq!(tile(&runner, 0, 7), red(8));
}

#[test]
fn duck_source_press_on_the_lit_tile_does_nothing() {
    let mut runner = route_runner();
    let pressed = press(&mut runner, 0, 7);
    assert_eq!(toast(&runner), "Play: Route");
    assert_eq!(resent_slots(&pressed), (vec![], vec![]));
    assert!(!runner.config_dirty);
    assert_eq!(tile(&runner, 0, 7), red(1));
}

#[test]
fn mixed_duck_sources_show_dim_and_bus_sources_stay_off_the_row() {
    let mut runner = route_runner_with_ducks(&[
        (1, 1, "I2", "pre"),
        (2, 3, "I5", "pre"),
        (3, 1, "B1", "pre"),
    ]);
    assert_eq!(tile(&runner, 1, 7), red(3));
    assert_eq!(tile(&runner, 4, 7), red(3));
    assert_eq!(tile(&runner, 0, 7), red(8));

    let _ = press(&mut runner, 1, 7);
    assert_eq!(toast(&runner), "Duck source: I2 (3 ducks)");
    assert_eq!(duck_source(&runner, 3, 1), "I2");
    assert_eq!(tile(&runner, 1, 7), red(1));
    assert_eq!(tile(&runner, 4, 7), red(8));
}

#[test]
fn duck_source_press_without_a_duck_only_toasts() {
    let mut runner = route_runner_with_ducks(&[]);
    let pressed = press(&mut runner, 2, 7);
    assert_eq!(toast(&runner), "No duck on any bus");
    assert_eq!(resent_slots(&pressed), (vec![], vec![]));
    assert!(!runner.config_dirty);
}

#[test]
fn patch_load_shows_the_loaded_duck_sources() {
    let mut runner = route_runner();
    load_duck_sources(&mut runner, &[(1, 1, "I4"), (2, 2, "I4")]);
    open_route_page(&mut runner);

    assert_eq!(tile(&runner, 3, 7), red(1));
    assert_eq!(tile(&runner, 0, 7), red(8));
    assert_eq!(
        runner
            .menu
            .value_for_key("mixer.buses.2.slot2.params.source")
            .as_deref(),
        Some("I4")
    );
}

#[test]
fn duck_source_on_a_slot_that_turned_midi_stays_lit_and_ignores_presses() {
    let mut runner = route_runner();
    assert!(runner
        .menu
        .set_enum_value_for_key("instruments.0.type", "midi"));
    runner
        .apply_or_schedule_menu_key("instruments.0.type")
        .unwrap();
    assert_eq!(runner.instruments[0].kind, InstrumentType::Midi);
    assert_eq!(tile(&runner, 0, 7), red(1));
    runner.config_dirty = false;

    let pressed = press(&mut runner, 0, 7);
    assert_eq!(resent_slots(&pressed), (vec![], vec![]));
    assert!(!runner.config_dirty);
    assert_eq!(duck_source(&runner, 1, 1), "I1");
}

/// `(bus, slot)` of every bus slot command and `slot` of every global slot command.
fn resent_slots(messages: &[RunnerMessage]) -> (Vec<(usize, usize)>, Vec<usize>) {
    let mut buses = Vec::new();
    let mut globals = Vec::new();
    for command in audio_commands(messages) {
        match command {
            RuntimeAudioCommand::SetFxBusSlot {
                bus_index,
                slot_index,
                ..
            } => buses.push((bus_index, slot_index)),
            RuntimeAudioCommand::SetGlobalFxSlot { slot_index, .. } => globals.push(slot_index),
            _ => {}
        }
    }
    (buses, globals)
}

#[test]
fn route_toggles_resend_only_the_chain_they_change() {
    let mut runner = route_runner();
    let bus = press(&mut runner, 2, 5);
    assert_eq!(resent_slots(&bus), (vec![(2, 0), (2, 1), (2, 2)], vec![]));
    let global = press(&mut runner, 5, 5);
    assert_eq!(resent_slots(&global), (vec![], vec![1]));
    let duck = press(&mut runner, 3, 7);
    assert_eq!(resent_slots(&duck), (vec![(1, 0), (2, 1)], vec![]));
}

#[test]
fn duck_source_set_on_a_bypassed_bus_lands_when_the_bypass_ends() {
    let mut runner = route_runner();
    let _ = press(&mut runner, 1, 5);
    assert_eq!(toast(&runner), "B2 FX bypassed");

    let keyed = press(&mut runner, 2, 7);
    assert_eq!(toast(&runner), "Duck source: I3 (2 ducks)");
    assert_eq!(
        engine_bus_slot(&keyed, 1, 0).map(|(fx_type, _)| fx_type),
        Some("none".into()),
        "the bypass gates the resent duck"
    );
    assert_eq!(engine_bus_slot(&keyed, 2, 1).unwrap().1["source"], "I3");
    assert_eq!(duck_source(&runner, 1, 1), "I3");

    let restored = press(&mut runner, 1, 5);
    assert_eq!(toast(&runner), "B2 FX on");
    let (fx_type, params) = engine_bus_slot(&restored, 1, 0).unwrap();
    assert_eq!(fx_type, "duck");
    assert_eq!(params["source"], "I3");
}
