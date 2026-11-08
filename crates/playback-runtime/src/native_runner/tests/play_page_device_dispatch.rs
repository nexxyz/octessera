use super::*;
use crate::native_runner::play_pages::{PlayBank, PLAY_PAGES};

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

fn held(runner: &mut NativeRunner, buttons: &[&str], x: usize, y: usize) {
    for button in buttons {
        input(runner, json!({ "type": button, "pressed": true }));
    }
    press(runner, x, y);
    for button in buttons.iter().rev() {
        input(runner, json!({ "type": button, "pressed": false }));
    }
}

#[test]
fn every_play_page_opens_from_its_bank_tile_and_saves_its_mode() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    for page in &PLAY_PAGES {
        let buttons: &[&str] = match page.bank {
            PlayBank::Technical => &["button_fn"],
            PlayBank::Musical => &["button_shift", "button_fn"],
        };
        held(&mut runner, buttons, GRID_WIDTH - 1, page.row);
        assert_eq!(runner.active_play_mode, ActivePlayMode::Page(page.mode));
        assert_eq!(
            runner.config_payload()["runtimeConfig"]["playMode"],
            page.mode.wire_name()
        );
        assert_eq!(runner.test_active_play_mode(), page.mode.wire_name());
    }

    held(&mut runner, &["button_fn"], 0, 2);
    assert_eq!(runner.active_play_mode, ActivePlayMode::None);
    assert_eq!(runner.test_active_play_mode(), "none");
    assert_eq!(runner.play_mode, PlayMode::TriggerGate);
}

#[test]
fn play_fx_page_tiles_start_their_effect_through_device_input() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.play_fx_assignments.push(NativePlayFxAssignment {
        x: 1,
        y: 2,
        config: json!({ "fxType": "crush", "targetKey": "master", "params": {} }),
    });
    held(
        &mut runner,
        &["button_shift", "button_fn"],
        GRID_WIDTH - 1,
        0,
    );
    let started = input(&mut runner, json!({ "type": "grid_press", "x": 1, "y": 2 }));
    assert!(started.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects } if effects.iter().any(|effect| matches!(
            effect,
            RuntimePlatformEffect::AudioCommand {
                command: RuntimeAudioCommand::MomentaryFxStart { fx_type, .. }
            } if fx_type == "crush"
        ))
    )));
}

#[test]
fn unversioned_play_mode_none_loads_as_mix_and_unknown_modes_are_ignored() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner
        .apply_config_payload(json!({ "runtimeConfig": { "playMode": "xy" } }))
        .unwrap();
    assert_eq!(runner.play_mode, PlayMode::Xy);
    runner
        .apply_config_payload(json!({ "runtimeConfig": { "playMode": "none" } }))
        .unwrap();
    assert_eq!(runner.play_mode, PlayMode::Mix);
    assert_eq!(runner.config_payload()["runtimeConfig"]["playMode"], "mix");
    runner
        .apply_config_payload(json!({ "runtimeConfig": { "playMode": "dance" } }))
        .unwrap();
    assert_eq!(runner.play_mode, PlayMode::Mix);
    assert_eq!(runner.active_play_mode, ActivePlayMode::None);
}
