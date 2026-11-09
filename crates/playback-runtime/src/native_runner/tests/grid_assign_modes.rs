use super::*;
use std::time::Duration;

fn drum_runner() -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["instruments"][0]["type"] = json!("drum");
    payload["runtimeConfig"]["instruments"][1]["type"] = json!("midi_drum");
    runner.apply_config_payload(payload).unwrap();
    runner.display.oled_mode = NativeOledMode::Normal;
    runner.display.oled_splash_text.clear();
    runner.display.oled_splash_until = None;
    runner
}

fn enter(runner: &mut NativeRunner, action: &str) {
    runner
        .execute_menu_action(crate::native_menu::NativeMenuAction::PlatformEffect(
            action.into(),
        ))
        .unwrap();
}

fn back(runner: &mut NativeRunner) {
    device_input(runner, json!({ "type": "button_a", "pressed": true }));
    device_input(runner, json!({ "type": "button_a", "pressed": false }));
}

fn clear_toast(runner: &mut NativeRunner) {
    runner.display.toast = None;
    runner.display.toast_expires_at = None;
}

fn hold_shift_past_the_hint_delay(runner: &mut NativeRunner) -> Option<String> {
    device_input(runner, json!({ "type": "button_shift", "pressed": true }));
    std::thread::sleep(Duration::from_millis(1100));
    runner.capture_display_scene().unwrap();
    runner
        .display
        .toast
        .as_ref()
        .map(|toast| toast.message.clone())
}

#[test]
fn entering_an_assign_mode_replaces_the_previous_one() {
    let mut runner = drum_runner();
    runner.faders.action_cols = 2;
    let modes = [
        (
            "trigger.probability.assign:2",
            GridAssign::TriggerProbability(2),
        ),
        (
            "sample.assign:3:1",
            GridAssign::Sample {
                slot: 3,
                sample_slot: 1,
            },
        ),
        ("drum.assign:0:4", GridAssign::Drum { slot: 0, voice: 4 }),
        ("midiDrum.assign:1", GridAssign::MidiDrum { slot: 1 }),
        (
            crate::native_menu::FADER_TILE_MAP_ACTION,
            GridAssign::FaderTiles,
        ),
    ];
    for (action, mode) in modes {
        enter(&mut runner, action);
        assert_eq!(runner.grid_assign, Some(mode), "{action}");
    }
    enter(&mut runner, "play.fx.map");
    assert!(matches!(runner.grid_assign, Some(GridAssign::PlayFx(_))));
    enter(&mut runner, "drum.assign:0:1");
    assert_eq!(
        runner.grid_assign,
        Some(GridAssign::Drum { slot: 0, voice: 1 })
    );
}

#[test]
fn one_back_press_leaves_any_assign_mode_and_keeps_the_menu_where_it_was() {
    let mut runner = drum_runner();
    for action in [
        "trigger.probability.assign:0",
        "drum.assign:0:2",
        "midiDrum.assign:1",
    ] {
        enter(&mut runner, "sample.assign:3:0");
        enter(&mut runner, action);
        let path = runner.menu.state.stack.clone();
        back(&mut runner);
        assert_eq!(runner.grid_assign, None, "{action}");
        assert_eq!(runner.menu.state.stack, path, "{action}");
    }
}

#[test]
fn drum_assign_suppresses_the_shift_hint_until_back_leaves_it() {
    for action in ["drum.assign:0:0", "midiDrum.assign:1"] {
        let mut runner = drum_runner();
        enter(&mut runner, action);
        clear_toast(&mut runner);
        assert_eq!(
            hold_shift_past_the_hint_delay(&mut runner),
            None,
            "{action}"
        );

        back(&mut runner);
        assert_eq!(runner.grid_assign, None, "{action}");
        device_input(
            &mut runner,
            json!({ "type": "button_shift", "pressed": false }),
        );
        clear_toast(&mut runner);
        assert_eq!(
            hold_shift_past_the_hint_delay(&mut runner).as_deref(),
            Some("Shift: map/edit"),
            "{action}"
        );
    }
}
