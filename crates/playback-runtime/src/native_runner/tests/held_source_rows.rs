//! A held source (CC In, Play XY, layer axis) sets its target the way a menu edit does: the row
//! shows the value it set, the encoder steps from there, and an LFO leaves the row alone.

use super::audio_restart_persistence::{press, turn};
use super::in_place_menu_actions::press_key;
use super::tempo_lfo::{lfo_on, pulse};
use super::tempo_modulation::{
    click_row, map_cc, midi_runner, press_play, send_cc, set_menu_enum, set_menu_number,
};
use super::*;
use crate::native_runner::modulation::process_audio::audio_base_value;
use std::time::Instant;

/// What plays for `key`, in its row's units.
fn played(runner: &NativeRunner, key: &str) -> f64 {
    if key == "sound.velocityScalePct" {
        return f64::from(runner.global_sound.velocity_scale_pct);
    }
    audio_base_value(runner, key).unwrap_or_else(|| panic!("{key} plays nothing"))
}

const NUMBER_TARGETS: [(&str, u8, i32); 5] = [
    ("instruments.0.mixer.volume", 64, -1),
    ("instruments.0.synth.filter.cutoffHz", 90, 1),
    ("instruments.0.synth.ampEnv.releaseMs", 64, 1),
    ("mixer.buses.0.volume", 0, 1),
    ("sound.velocityScalePct", 100, -1),
];

#[test]
fn a_cc_shows_what_it_set_on_the_row() {
    for (key, cc_value, _) in NUMBER_TARGETS {
        let mut runner = midi_runner();
        let before = runner.menu.number_for_key(key).unwrap();
        map_cc(&mut runner, 0, key);
        send_cc(&mut runner, 1, cc_value);

        let shown = runner.menu.number_for_key(key).unwrap();
        assert_ne!(shown, before, "{key} moved");
        assert_eq!(f64::from(shown), played(&runner, key).round(), "{key}");
    }
}

#[test]
fn a_cc_on_a_curved_row_shows_the_curved_value() {
    let key = "instruments.0.synth.ampEnv.releaseMs";
    let mut runner = midi_runner();
    map_cc(&mut runner, 0, key);
    send_cc(&mut runner, 1, 64);
    let (min, max) = match runner.menu.item_for_key(key).unwrap().value {
        crate::native_menu::NativeMenuValue::Number { min, max, .. } => (min, max),
        _ => panic!("{key} is a number row"),
    };
    let shown = runner.menu.number_for_key(key).unwrap();
    assert!(
        shown < (min + max) / 2,
        "{shown} sits on the curve, below the linear middle of {min}..{max}"
    );
    assert_eq!(f64::from(shown), played(&runner, key).round());
}

/// The encoder steps from the value a CC left, as it does from a row the menu set there.
#[test]
fn one_encoder_click_after_a_cc_steps_from_the_cc_value() {
    for (key, cc_value, delta) in NUMBER_TARGETS {
        let mut runner = midi_runner();
        map_cc(&mut runner, 0, key);
        send_cc(&mut runner, 1, cc_value);
        let shown = runner.menu.number_for_key(key).unwrap();

        let mut by_menu = midi_runner();
        set_menu_number(&mut by_menu, key, shown);
        let expected = click_row(&mut by_menu, key, delta);
        assert_ne!(expected, shown, "{key}");

        assert_eq!(click_row(&mut runner, key, delta), expected, "{key}");
        assert_eq!(played(&runner, key), played(&by_menu, key), "{key}");
    }
}

#[test]
fn a_cc_on_an_option_row_shows_the_option_and_the_encoder_steps_from_it() {
    let key = "instruments.0.synth.osc1.waveform";
    let mut runner = midi_runner();
    let before = runner.menu.value_for_key(key).unwrap();
    map_cc(&mut runner, 0, key);
    send_cc(&mut runner, 1, 127);
    let shown = runner.menu.value_for_key(key).unwrap();
    assert_ne!(shown, before);

    let mut by_menu = midi_runner();
    set_menu_enum(&mut by_menu, key, &shown);
    for runner in [&mut runner, &mut by_menu] {
        assert!(runner.menu.focus_item_key(key));
        press(runner);
        turn(runner, -1);
        press(runner);
    }
    let stepped = runner.menu.value_for_key(key).unwrap();
    assert_ne!(stepped, shown);
    assert_eq!(Some(stepped), by_menu.menu.value_for_key(key));
    assert_eq!(runner.instruments[0], by_menu.instruments[0]);
}

#[test]
fn xy_shows_what_it_set_and_the_earlier_value_returns_when_it_lets_go() {
    let key = "instruments.0.mixer.volume";
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.xy.smoothing_ms = 0;
    set_menu_number(&mut runner, key, 30);
    press_key(&mut runner, &format!("xy:x.{key}"));
    let now = Instant::now();

    runner.handle_play_xy_press_at(GRID_WIDTH - 1, 0, now);
    assert_eq!(runner.instruments[0].volume, 100);
    assert_eq!(runner.menu.number_for_key(key), Some(100));
    runner.handle_play_xy_press_at(0, 0, now);
    assert_eq!(runner.menu.number_for_key(key), Some(0));

    runner.set_param_binding_target("xy:x", None);
    assert_eq!(runner.instruments[0].volume, 30);
    assert_eq!(runner.menu.number_for_key(key), Some(30));
}

#[test]
fn an_lfo_plays_on_top_of_the_row_and_the_row_keeps_the_set_value() {
    let key = "instruments.0.mixer.volume";
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    set_menu_number(&mut runner, key, 50);
    lfo_on(&mut runner, key, "1/4", 50);
    press_play(&mut runner);

    let mut moved = false;
    for _ in 0..24 {
        let messages = pulse(&mut runner);
        moved |= messages.iter().any(|message| {
            matches!(message, RunnerMessage::AudioCommands { commands } if commands.iter().any(
                |command| matches!(command, RuntimeAudioCommand::SetInstrumentMixer {
                    instrument_slot: 0,
                    volume_pct: Some(volume),
                    ..
                } if (*volume - 50.0).abs() > 1.0)
            ))
        });
        assert_eq!(runner.menu.number_for_key(key), Some(50));
        assert_eq!(runner.instruments[0].volume, 50);
    }
    assert!(moved, "the LFO plays");
}
