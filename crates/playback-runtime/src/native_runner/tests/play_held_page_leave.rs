use super::*;

fn input(runner: &mut NativeRunner, input: Value) -> Vec<RunnerMessage> {
    runner
        .send(HostMessage::DeviceInput {
            input,
            request_snapshot: None,
        })
        .unwrap()
}

fn button(runner: &mut NativeRunner, button: &str, pressed: bool) {
    let _ = input(runner, json!({ "type": button, "pressed": pressed }));
}

fn press(runner: &mut NativeRunner, x: usize, y: usize) -> Vec<RunnerMessage> {
    input(runner, json!({ "type": "grid_press", "x": x, "y": y }))
}

fn release(runner: &mut NativeRunner, x: usize, y: usize) -> Vec<RunnerMessage> {
    input(runner, json!({ "type": "grid_release", "x": x, "y": y }))
}

fn open_musical_page(runner: &mut NativeRunner, row: usize) {
    button(runner, "button_shift", true);
    button(runner, "button_fn", true);
    let _ = press(runner, GRID_WIDTH - 1, row);
    let _ = release(runner, GRID_WIDTH - 1, row);
    button(runner, "button_fn", false);
    button(runner, "button_shift", false);
}

fn fx_runner() -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.play_fx_assignments.push(NativePlayFxAssignment {
        x: 3,
        y: 4,
        config: json!({ "fxType": "stutter", "targetKey": "master", "params": {} }),
    });
    open_musical_page(&mut runner, 0);
    assert_eq!(runner.active_play_mode, "fx");
    runner
}

fn stops(messages: &[RunnerMessage]) -> Vec<String> {
    messages
        .iter()
        .filter_map(|message| match message {
            RunnerMessage::PlatformEffects { effects } => Some(effects),
            _ => None,
        })
        .flatten()
        .filter_map(|effect| match effect {
            RuntimePlatformEffect::AudioCommand {
                command: RuntimeAudioCommand::MomentaryFxStop { id, .. },
            } => Some(id.clone()),
            _ => None,
        })
        .collect()
}

fn assert_release_stops_the_held_fx(leave: impl FnOnce(&mut NativeRunner)) {
    let mut runner = fx_runner();
    let _ = press(&mut runner, 3, 4);
    assert_eq!(runner.active_play_fx.len(), 1);

    leave(&mut runner);
    assert_ne!(runner.active_play_mode, "fx");

    let messages = release(&mut runner, 3, 4);
    assert_eq!(stops(&messages), vec!["momentary-fx:3:4".to_string()]);
    assert!(runner.active_play_fx.is_empty());
}

#[test]
fn a_held_play_fx_stops_on_release_after_fn_layer_select() {
    assert_release_stops_the_held_fx(|runner| {
        button(runner, "button_fn", true);
        let _ = press(runner, 0, 2);
        let _ = release(runner, 0, 2);
        button(runner, "button_fn", false);
    });
}

#[test]
fn a_held_play_fx_stops_on_release_after_back() {
    assert_release_stops_the_held_fx(|runner| button(runner, "button_a", true));
}

#[test]
fn a_held_play_fx_stops_on_release_after_a_page_switch() {
    assert_release_stops_the_held_fx(|runner| {
        button(runner, "button_fn", true);
        let _ = press(runner, GRID_WIDTH - 1, 0);
        let _ = release(runner, GRID_WIDTH - 1, 0);
        button(runner, "button_fn", false);
        assert_eq!(runner.active_play_mode, "mix");
    });
}

#[test]
fn a_held_xy_touch_ends_on_release_after_a_page_switch() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner
        .menu
        .set_enum_value_for_key("play.xy.release", "reset-center");
    runner
        .apply_or_schedule_menu_key("play.xy.release")
        .unwrap();
    open_musical_page(&mut runner, 1);
    assert_eq!(runner.active_play_mode, "xy");

    let _ = press(&mut runner, 6, 5);
    assert!(runner.xy_touch.active);
    button(&mut runner, "button_fn", true);
    let _ = press(&mut runner, GRID_WIDTH - 1, 0);
    let _ = release(&mut runner, GRID_WIDTH - 1, 0);
    button(&mut runner, "button_fn", false);
    assert_eq!(runner.active_play_mode, "mix");
    assert!(
        runner.xy_touch.active,
        "another tile's release keeps the hold"
    );

    let _ = release(&mut runner, 6, 5);
    assert!(!runner.xy_touch.active);
    assert_eq!(
        (runner.xy_touch.display_x, runner.xy_touch.display_y),
        (0.5, 0.5)
    );
}
