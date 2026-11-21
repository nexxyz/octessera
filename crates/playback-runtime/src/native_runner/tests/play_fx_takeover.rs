use super::*;

const PITCH: [(usize, i64); 3] = [(0, 12), (1, 7), (2, 5)];
const STUTTER_X: usize = 3;
const FREEZE_X: usize = 4;

fn button(runner: &mut NativeRunner, button: &str, pressed: bool) {
    let _ = device_input(runner, json!({ "type": button, "pressed": pressed }));
}

fn press(runner: &mut NativeRunner, x: usize) -> Vec<String> {
    audio(&device_input(
        runner,
        json!({ "type": "grid_press", "x": x, "y": 0 }),
    ))
}

fn release(runner: &mut NativeRunner, x: usize) -> Vec<String> {
    audio(&device_input(
        runner,
        json!({ "type": "grid_release", "x": x, "y": 0 }),
    ))
}

/// The momentary FX commands as `start x`, `move x` and `stop x` for the voices of the tiles on
/// row 0, with the interval or cutoff and the slide or sweep in time they carry.
fn audio(messages: &[RunnerMessage]) -> Vec<String> {
    let tile = |id: &str| id.trim_start_matches("momentary-fx:").replace(":0", "");
    let settings = |params: &BTreeMap<String, Value>| {
        let amount = ["semitones", "cutoffPct"]
            .iter()
            .find_map(|key| params.get(*key))
            .map_or("-".to_string(), Value::to_string);
        ["slideInMs", "sweepInMs"]
            .iter()
            .find_map(|key| params.get(*key))
            .map_or(amount.clone(), |time| format!("{amount} {time}"))
    };
    messages
        .iter()
        .filter_map(|message| match message {
            RunnerMessage::PlatformEffects { effects } => Some(effects),
            _ => None,
        })
        .flatten()
        .filter_map(|effect| match effect {
            RuntimePlatformEffect::AudioCommand { command } => Some(command),
            _ => None,
        })
        .filter_map(|command| match command {
            RuntimeAudioCommand::MomentaryFxStart { id, params, .. } => {
                Some(format!("start {} {}", tile(id), settings(params)))
            }
            RuntimeAudioCommand::MomentaryFxUpdate { id, params, .. } => {
                Some(format!("move {} {}", tile(id), settings(params)))
            }
            RuntimeAudioCommand::MomentaryFxStop { id, .. } => Some(format!("stop {}", tile(id))),
            _ => None,
        })
        .collect()
}

/// A runner on the Play FX page with each `(x, config)` mapped to a tile on row 0.
fn mapped_runner(tiles: Vec<(usize, Value)>) -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    for (x, config) in tiles {
        runner.play_fx_assignments.push(NativePlayFxAssignment {
            x,
            y: 0,
            config: literal_play_fx(config),
        });
    }
    button(&mut runner, "button_shift", true);
    button(&mut runner, "button_fn", true);
    grid_tap(&mut runner, GRID_WIDTH - 1, 0);
    button(&mut runner, "button_fn", false);
    button(&mut runner, "button_shift", false);
    assert_eq!(runner.active_play_mode, ActivePlayMode::Page(PlayMode::Fx));
    runner
}

fn fx_page_runner() -> NativeRunner {
    let pitch = PITCH.map(|(x, semitones)| {
        let config = json!({
            "fxType": "pitch_shift",
            "targetKey": "master",
            "params": { "semitones": semitones }
        });
        (x, config)
    });
    let others = [(STUTTER_X, "stutter"), (FREEZE_X, "freeze")].map(|(x, fx_type)| {
        let config = json!({ "fxType": fx_type, "targetKey": "master", "params": {} });
        (x, config)
    });
    mapped_runner(pitch.into_iter().chain(others).collect())
}

/// The pitch tiles shown brighter than their idle magenta.
fn lit_pitch_tiles(runner: &NativeRunner) -> Vec<usize> {
    let tiles = led_tiles(&runner.snapshot().unwrap());
    PITCH
        .iter()
        .map(|(x, _)| *x)
        .filter(|x| tiles[display_index(*x, 0)] != led_rgb(platform_core::palette::MAGENTA))
        .collect()
}

fn toast(runner: &NativeRunner) -> String {
    runner
        .display
        .toast
        .as_ref()
        .map(|toast| toast.message.clone())
        .unwrap_or_default()
}

fn assert_no_holds(runner: &NativeRunner) {
    assert!(runner.active_play_fx.is_empty());
    assert!(runner.displaced_play_fx.is_empty());
}

#[test]
fn a_newer_press_of_the_same_type_takes_over_and_release_falls_back() {
    let mut runner = fx_page_runner();
    assert_eq!(press(&mut runner, 0), ["start 0 12"]);
    assert_eq!(lit_pitch_tiles(&runner), [0]);

    assert_eq!(press(&mut runner, 1), ["move 0 7"]);
    assert_eq!(lit_pitch_tiles(&runner), [1]);
    assert_eq!(runner.active_play_fx.len(), 1);

    assert_eq!(release(&mut runner, 1), ["move 0 12"]);
    assert_eq!(lit_pitch_tiles(&runner), [0]);

    assert_eq!(release(&mut runner, 0), ["stop 0"]);
    assert!(lit_pitch_tiles(&runner).is_empty());
    assert_no_holds(&runner);
}

#[test]
fn a_three_deep_stack_pops_back_to_the_newest_tile_still_held() {
    let mut runner = fx_page_runner();
    let _ = press(&mut runner, 0);
    let _ = press(&mut runner, 1);
    assert_eq!(press(&mut runner, 2), ["move 0 5"]);
    assert_eq!(lit_pitch_tiles(&runner), [2]);

    assert_eq!(release(&mut runner, 2), ["move 0 7"]);
    assert_eq!(lit_pitch_tiles(&runner), [1]);
    assert_eq!(release(&mut runner, 1), ["move 0 12"]);
    assert_eq!(lit_pitch_tiles(&runner), [0]);
    assert_eq!(release(&mut runner, 0), ["stop 0"]);
    assert_no_holds(&runner);
}

#[test]
fn releasing_a_tile_held_underneath_only_drops_it() {
    let mut runner = fx_page_runner();
    let _ = press(&mut runner, 0);
    let _ = press(&mut runner, 1);
    let _ = press(&mut runner, 2);

    assert!(release(&mut runner, 1).is_empty());
    assert_eq!(lit_pitch_tiles(&runner), [2]);
    assert_eq!(release(&mut runner, 2), ["move 0 12"]);
    assert!(release(&mut runner, 1).is_empty());
    assert_eq!(release(&mut runner, 0), ["stop 0"]);
    assert_no_holds(&runner);
}

#[test]
fn the_moved_voice_outlives_the_tile_that_started_it() {
    let mut runner = fx_page_runner();
    let _ = press(&mut runner, 0);
    let _ = press(&mut runner, 1);

    assert!(release(&mut runner, 0).is_empty());
    assert_eq!(lit_pitch_tiles(&runner), [1]);
    assert_eq!(press(&mut runner, 2), ["move 0 5"]);
    assert_eq!(release(&mut runner, 2), ["move 0 7"]);
    assert_eq!(release(&mut runner, 1), ["stop 0"]);
    assert_no_holds(&runner);
}

#[test]
fn every_sweep_type_moves_its_running_voice_to_the_tile_on_top() {
    for (fx_type, amount, time) in [
        ("pitch_shift", "semitones", "slideInMs"),
        ("filter_sweep", "cutoffPct", "sweepInMs"),
        ("hp_riser", "cutoffPct", "sweepInMs"),
    ] {
        let tile = |value: i64, ms: i64| {
            json!({
                "fxType": fx_type,
                "targetKey": "fx_bus_2",
                "params": { (amount): value, (time): ms }
            })
        };
        let mut runner = mapped_runner(vec![(0, tile(5, 100)), (1, tile(12, 300))]);

        assert_eq!(press(&mut runner, 0), ["start 0 5 100"], "{fx_type}");
        assert_eq!(press(&mut runner, 1), ["move 0 12 300"], "{fx_type}");
        assert_eq!(release(&mut runner, 1), ["move 0 5 100"], "{fx_type}");
        assert_eq!(press(&mut runner, 1), ["move 0 12 300"], "{fx_type}");
        assert!(release(&mut runner, 0).is_empty(), "{fx_type}");
        assert_eq!(release(&mut runner, 1), ["stop 0"], "{fx_type}");
        assert_no_holds(&runner);
    }
}

#[test]
fn a_sweep_type_on_another_target_restarts_on_that_target() {
    for fx_type in ["pitch_shift", "filter_sweep", "hp_riser"] {
        let tile = |target: &str| json!({ "fxType": fx_type, "targetKey": target, "params": {} });
        let mut runner = mapped_runner(vec![(0, tile("master")), (1, tile("instrument_1"))]);

        let _ = press(&mut runner, 0);
        assert_eq!(press(&mut runner, 1), ["stop 0", "start 1 -"], "{fx_type}");
        assert_eq!(
            release(&mut runner, 1),
            ["stop 1", "start 0 -"],
            "{fx_type}"
        );
        assert_eq!(release(&mut runner, 0), ["stop 0"], "{fx_type}");
        assert_no_holds(&runner);
    }
}

#[test]
fn other_types_restart_on_a_takeover() {
    for fx_type in [
        "stutter",
        "freeze",
        "tape_stop",
        "reverse",
        "trance_gate",
        "dub_throw",
        "wash",
        "crush",
    ] {
        let tile = json!({ "fxType": fx_type, "targetKey": "master", "params": {} });
        let mut runner = mapped_runner(vec![(0, tile.clone()), (1, tile)]);

        let _ = press(&mut runner, 0);
        assert_eq!(press(&mut runner, 1), ["stop 0", "start 1 -"], "{fx_type}");
        assert_eq!(
            release(&mut runner, 1),
            ["stop 1", "start 0 -"],
            "{fx_type}"
        );
        assert_eq!(release(&mut runner, 0), ["stop 0"], "{fx_type}");
        assert_no_holds(&runner);
    }
}

#[test]
fn a_takeover_does_not_count_against_the_two_type_limit() {
    let mut runner = fx_page_runner();
    let _ = press(&mut runner, 0);
    assert_eq!(press(&mut runner, STUTTER_X), ["start 3 -"]);

    assert_eq!(press(&mut runner, 1), ["move 0 7"]);
    assert_eq!(runner.active_play_fx.len(), 2);
    let tiles = led_tiles(&runner.snapshot().unwrap());
    assert_eq!(
        tiles[display_index(2, 0)],
        led_rgb(platform_core::palette::MAGENTA),
        "a third pitch tile stays pressable at the limit"
    );

    assert!(press(&mut runner, FREEZE_X).is_empty());
    assert_eq!(toast(&runner), "FX limit: 2 at once");
}

#[test]
fn leaving_the_page_keeps_the_stack_until_each_tile_is_released() {
    let mut runner = fx_page_runner();
    let _ = press(&mut runner, 0);
    let _ = press(&mut runner, 1);
    let _ = press(&mut runner, 2);
    let _ = press(&mut runner, STUTTER_X);
    button(&mut runner, "button_a", true);
    button(&mut runner, "button_a", false);
    assert_ne!(runner.active_play_mode, ActivePlayMode::Page(PlayMode::Fx));

    assert!(release(&mut runner, 1).is_empty());
    assert_eq!(release(&mut runner, 2), ["move 0 12"]);
    assert_eq!(release(&mut runner, STUTTER_X), ["stop 3"]);
    assert_eq!(release(&mut runner, 0), ["stop 0"]);
    assert_no_holds(&runner);
}

#[test]
fn clear_all_stops_the_running_voice_and_drops_the_tiles_held_underneath() {
    let mut runner = fx_page_runner();
    let _ = press(&mut runner, 0);
    let _ = press(&mut runner, 1);
    let _ = runner.outbox.drain_audio_commands();

    runner.clear_patch_state().unwrap();

    assert_no_holds(&runner);
    let stops = runner
        .outbox
        .drain_audio_commands()
        .into_iter()
        .filter_map(|command| match command {
            RuntimeAudioCommand::MomentaryFxStop { id, .. } => Some(id),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(stops, ["momentary-fx:0:0"]);
}

#[test]
fn a_held_source_on_a_play_fx_row_moves_the_sounding_effect_of_that_type() {
    let mut runner = fx_page_runner();
    assert_eq!(press(&mut runner, 0), ["start 0 12"]);
    let _ = runner.apply_param_binding_value(
        "play.fx.type",
        json!("pitch_shift"),
        &mut BTreeMap::new(),
    );
    let binding = NativeParamBinding {
        key: "play.fx.params.semitones".into(),
        label: None,
        kind: "number".into(),
        min: Some(-24.0),
        max: Some(24.0),
        step: Some(1.0),
        user_min: None,
        user_max: None,
        options: vec![],
        invert: false,
    };
    let source = crate::native_runner::modulation::source::ModulationSourceId::midi_cc(0).unwrap();
    let _ = runner.outbox.drain_audio_commands();

    runner.set_runtime_source_input(source, binding, 1.0);
    runner.process_dirty_modulation_step(true).unwrap();

    let updates = runner
        .outbox
        .drain_audio_commands()
        .into_iter()
        .filter_map(|command| match command {
            RuntimeAudioCommand::MomentaryFxUpdate { id, params, .. } => {
                Some((id, params.get("semitones").cloned()))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(updates, [("momentary-fx:0:0".to_string(), Some(json!(24)))]);
    assert_eq!(
        runner.play_fx_assignments[0].config.params.get("semitones"),
        Some(&12),
        "the tile keeps its stored setting"
    );
    assert_eq!(release(&mut runner, 0), ["stop 0"]);
}
