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

/// The momentary FX commands as `start x` / `stop x` for the tiles on row 0.
fn audio(messages: &[RunnerMessage]) -> Vec<String> {
    let tile = |id: &str| id.trim_start_matches("momentary-fx:").replace(":0", "");
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
            RuntimeAudioCommand::MomentaryFxStart { id, params, .. } => Some(format!(
                "start {} {}",
                tile(id),
                params.get("semitones").map_or(json!("-"), Clone::clone)
            )),
            RuntimeAudioCommand::MomentaryFxStop { id, .. } => Some(format!("stop {}", tile(id))),
            _ => None,
        })
        .collect()
}

fn fx_page_runner() -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    for (x, semitones) in PITCH {
        runner.play_fx_assignments.push(NativePlayFxAssignment {
            x,
            y: 0,
            config: json!({
                "fxType": "pitch_shift",
                "targetKey": "master",
                "params": { "semitones": semitones }
            }),
        });
    }
    for (x, fx_type) in [(STUTTER_X, "stutter"), (FREEZE_X, "freeze")] {
        runner.play_fx_assignments.push(NativePlayFxAssignment {
            x,
            y: 0,
            config: json!({ "fxType": fx_type, "targetKey": "master", "params": {} }),
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

    assert_eq!(press(&mut runner, 1), ["stop 0", "start 1 7"]);
    assert_eq!(lit_pitch_tiles(&runner), [1]);
    assert_eq!(runner.active_play_fx.len(), 1);

    assert_eq!(release(&mut runner, 1), ["stop 1", "start 0 12"]);
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
    assert_eq!(press(&mut runner, 2), ["stop 1", "start 2 5"]);
    assert_eq!(lit_pitch_tiles(&runner), [2]);

    assert_eq!(release(&mut runner, 2), ["stop 2", "start 1 7"]);
    assert_eq!(lit_pitch_tiles(&runner), [1]);
    assert_eq!(release(&mut runner, 1), ["stop 1", "start 0 12"]);
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
    assert_eq!(release(&mut runner, 2), ["stop 2", "start 0 12"]);
    assert!(release(&mut runner, 1).is_empty());
    assert_eq!(release(&mut runner, 0), ["stop 0"]);
    assert_no_holds(&runner);
}

#[test]
fn a_takeover_does_not_count_against_the_two_type_limit() {
    let mut runner = fx_page_runner();
    let _ = press(&mut runner, 0);
    assert_eq!(press(&mut runner, STUTTER_X), ["start 3 \"-\""]);

    assert_eq!(press(&mut runner, 1), ["stop 0", "start 1 7"]);
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
    assert_eq!(release(&mut runner, 2), ["stop 2", "start 0 12"]);
    assert_eq!(release(&mut runner, STUTTER_X), ["stop 3"]);
    assert_eq!(release(&mut runner, 0), ["stop 0"]);
    assert_no_holds(&runner);
}

#[test]
fn clear_all_drops_the_tiles_held_underneath() {
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
    assert_eq!(stops, ["momentary-fx:1:0"]);
}
