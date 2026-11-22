//! BPM and Swing as CC In and XY targets: a held source sets the tempo and its row like a menu
//! edit, and the last edit wins.

use super::audio_restart_persistence::{press, turn};
use super::in_place_menu_actions::press_key;
use super::*;
use crate::native_runner::modulation::source::ModulationSourceId;
use crate::{MidiControlChange, MidiControlChangeBatch};
use std::time::Instant;

fn midi_runner() -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.test_set_midi_enabled(true);
    runner
}

fn send_cc(runner: &mut NativeRunner, cc: u8, value: u8) -> Vec<RunnerMessage> {
    let mut batch = MidiControlChangeBatch::default();
    batch.push(MidiControlChange::parse(&[0xB0, cc, value]).expect("a Control Change"));
    batch
        .into_iter()
        .flat_map(|change| runner.send(change.into()).unwrap())
        .collect()
}

pub(super) fn set_menu_number(runner: &mut NativeRunner, key: &str, value: i32) {
    assert!(runner.menu.set_number_value_for_key(key, value), "{key}");
    runner.apply_or_schedule_menu_key(key).unwrap();
}

pub(super) fn set_menu_enum(runner: &mut NativeRunner, key: &str, value: &str) {
    runner.menu.set_enum_value_for_key(key, value);
    runner.apply_or_schedule_menu_key(key).unwrap();
}

fn map_cc(runner: &mut NativeRunner, slot: usize, key: &str) {
    press_key(runner, &format!("midiCc.{slot}.target.{key}"));
    assert_eq!(
        runner.midi_cc_mappings[slot]
            .target
            .as_ref()
            .map(|binding| binding.key.as_str()),
        Some(key)
    );
    runner.midi_cc_mappings[slot].cc = u8::try_from(slot + 1).unwrap();
}

pub(super) fn published_bpm(messages: &[RunnerMessage]) -> Option<f64> {
    messages.iter().rev().find_map(|message| match message {
        RunnerMessage::RuntimeConfigChanged { config } => Some(config.bpm),
        _ => None,
    })
}

pub(super) fn saved_transport(runner: &NativeRunner) -> Value {
    runner.config_payload()["runtimeConfig"]["transport"].clone()
}

pub(super) fn press_play(runner: &mut NativeRunner) -> Vec<RunnerMessage> {
    [true, false]
        .into_iter()
        .flat_map(|pressed| device_input(runner, json!({ "type": "button_s", "pressed": pressed })))
        .collect()
}

#[test]
fn a_cc_sets_bpm_and_swing_over_their_whole_range() {
    let mut runner = midi_runner();
    map_cc(&mut runner, 0, "transport.bpm");
    map_cc(&mut runner, 1, "transport.swingA");
    map_cc(&mut runner, 2, "transport.swingB");

    for (value, bpm) in [(0, 40.0), (64, 140.0), (127, 240.0)] {
        let messages = send_cc(&mut runner, 1, value);
        assert_eq!(runner.transport.bpm, bpm, "CC {value}");
        assert_eq!(runner.clock_bpm(), bpm);
        assert_eq!(published_bpm(&messages), Some(bpm), "the clock hears it");
    }
    for (value, swing) in [(0, 1), (64, 4), (127, 7)] {
        send_cc(&mut runner, 2, value);
        send_cc(&mut runner, 3, 127 - value);
        assert_eq!(runner.transport.swing_a, swing, "CC {value}");
        assert_eq!(runner.transport.swing_b, 8 - swing, "CC {value}");
    }
    assert_eq!(saved_transport(&runner)["bpm"], 240);
}

#[test]
fn a_cc_on_bpm_autosaves_like_any_held_target_only_when_the_tempo_moves() {
    let mut runner = midi_runner();
    map_cc(&mut runner, 0, "transport.bpm");
    runner.fast_autosave_marks = 0;

    send_cc(&mut runner, 1, 127);
    assert_eq!(runner.fast_autosave_marks, 1);
    send_cc(&mut runner, 1, 127);
    assert_eq!(
        runner.fast_autosave_marks, 1,
        "the same tempo is not a change"
    );
}

#[test]
fn the_last_edit_wins_between_the_bpm_row_and_a_cc() {
    let mut runner = midi_runner();
    map_cc(&mut runner, 0, "transport.bpm");
    send_cc(&mut runner, 1, 127);
    assert_eq!(runner.transport.bpm, 240.0);

    set_menu_number(&mut runner, "transport.bpm", 100);
    assert_eq!(runner.transport.bpm, 100.0);
    let source = ModulationSourceId::midi_cc(0).unwrap();
    assert!(!runner.modulation_process.has_source(source));
    assert_eq!(saved_transport(&runner)["bpm"], 100);

    send_cc(&mut runner, 1, 0);
    assert_eq!(runner.transport.bpm, 40.0, "a later CC takes it back");
}

#[test]
fn xy_sets_bpm_and_a_center_release_takes_the_middle_tempo() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    set_menu_enum(&mut runner, "play.xy.release", "reset-center");
    press_key(&mut runner, "xy:x.transport.bpm");
    assert_eq!(
        runner.xy.x_binding.as_ref().map(|b| b.key.as_str()),
        Some("transport.bpm")
    );
    runner.xy.smoothing_ms = 0;
    set_menu_number(&mut runner, "transport.bpm", 100);
    let now = Instant::now();

    runner.handle_play_xy_press_at(GRID_WIDTH - 1, 0, now);
    assert_eq!(runner.transport.bpm, 240.0);
    runner.handle_play_xy_press_at(0, 0, now);
    assert_eq!(runner.transport.bpm, 40.0);

    runner.handle_play_xy_release_at(now);
    assert_eq!(runner.transport.bpm, 140.0);
    set_menu_number(&mut runner, "transport.bpm", 101);
    assert_eq!(
        runner.transport.bpm, 101.0,
        "a menu edit takes it from the XY"
    );
}

#[test]
fn under_external_clock_a_cc_on_bpm_sets_the_tempo_row_and_the_clock_ignores_it() {
    let mut runner = midi_runner();
    set_menu_enum(&mut runner, "midiSyncMode", "external");
    assert_eq!(runner.transport.sync_source, SyncSource::External);
    map_cc(&mut runner, 0, "transport.bpm");
    let mut runtime = crate::PlaybackRuntime::new(crate::RuntimeConfig {
        bpm: runner.transport.bpm,
        sync_source: SyncSource::External,
        ..crate::RuntimeConfig::default()
    });
    let mut host = crate::tests::support::FakeHost::default();
    let messages = press_play(&mut runner);
    runtime.ingest_runner_messages(messages, &mut host).unwrap();

    let messages = send_cc(&mut runner, 1, 127);
    runtime.ingest_runner_messages(messages, &mut host).unwrap();
    assert_eq!(
        runner.transport.bpm, 240.0,
        "as the BPM row does under external sync"
    );
    let before = runner.transport.current_ppqn_pulse;
    for _ in 0..100 {
        runtime
            .advance_duration(Duration::from_millis(10), &mut runner, &mut host)
            .unwrap();
    }
    assert_eq!(
        runner.transport.current_ppqn_pulse, before,
        "only the external clock moves it"
    );
}

fn click_row(runner: &mut NativeRunner, key: &str, delta: i32) -> i32 {
    assert!(runner.menu.focus_item_key(key), "{key}");
    press(runner);
    turn(runner, delta);
    press(runner);
    runner.menu.number_for_key(key).unwrap()
}

/// The encoder steps from the tempo a CC left, as it does from a row the menu set there.
#[test]
fn one_encoder_click_after_a_cc_steps_from_the_cc_tempo() {
    for (key, cc_value, shown, delta) in [
        ("transport.bpm", 127, 240, -1),
        ("transport.swingA", 127, 7, -1),
        ("transport.swingB", 0, 1, 1),
    ] {
        let mut by_menu = midi_runner();
        set_menu_number(&mut by_menu, key, shown);
        let expected = click_row(&mut by_menu, key, delta);
        assert_ne!(expected, shown, "{key}");

        let mut runner = midi_runner();
        map_cc(&mut runner, 0, key);
        send_cc(&mut runner, 1, cc_value);
        assert_eq!(runner.menu.number_for_key(key), Some(shown), "{key}");
        assert_eq!(click_row(&mut runner, key, delta), expected, "{key}");
        assert_eq!(
            runner.tempo_base_value(key),
            Some(f64::from(expected)),
            "{key}"
        );
    }
}
