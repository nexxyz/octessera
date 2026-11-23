//! What releases keys besides their sources' own note-offs: a pitch-mapping change of a layer
//! and the drains, each for every key on the instruments the layer's note-on rows play, whoever
//! pressed it.

use super::key_model::{keys_layers, tile, words};
use super::tempo_modulation::{map_cc, send_cc};
use super::*;

/// Layer 1 holds a note on instrument 1, which layer 0's On also plays, after a step of both.
fn held_by_layer_one() -> (NativeRunner, String) {
    let mut runner = keys_layers(true);
    let on = tile(&mut runner, 1, "grid_press", 0, 2);
    let [on] = &on[..] else { panic!("{on:?}") };
    let off = on.replace('+', "-");
    runner.select_active_layer(0).unwrap();
    step(&mut runner);
    (runner, off)
}

fn step(runner: &mut NativeRunner) -> Vec<String> {
    let messages = runner
        .send(HostMessage::TransportPulseStep {
            pulses: runner.step_pulses_for_layer(0),
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();
    words(&messages, false)
}

#[test]
fn a_root_change_releases_another_layers_note_on_the_layers_instrument_at_its_next_event() {
    let (mut runner, off) = held_by_layer_one();
    runner.edit_menu_key("layers.0.link.pitch.root", 2).unwrap();
    let pressed = tile(&mut runner, 0, "grid_press", 0, 4);
    assert_eq!(pressed.first(), Some(&off), "{pressed:?}");
    assert!(tile(&mut runner, 1, "grid_release", 0, 2).is_empty());
}

#[test]
fn a_root_change_on_a_layer_playing_other_instruments_releases_nothing() {
    let (mut runner, _) = held_by_layer_one();
    runner.layers[0].link.activate_slot = 1;
    runner.edit_menu_key("layers.0.link.pitch.root", 2).unwrap();
    let pressed = tile(&mut runner, 0, "grid_press", 0, 4);
    assert!(
        pressed.iter().all(|word| word.starts_with('+')),
        "{pressed:?}"
    );
    assert_eq!(runner.held_keys.held(0).len(), 1);
}

/// A CC on Root that lands on the same Root, or moves Root under the chromatic set, moves no
/// pitch and releases nothing; moving Root under the pentatonic set moves them, and the held
/// note ends at the layer's next step.
#[test]
fn a_modulated_root_releases_only_when_the_mapping_moves() {
    for (scale, releases) in [("chromatic", false), ("major_pentatonic", true)] {
        let (mut runner, off) = held_by_layer_one();
        runner.test_set_midi_enabled(true);
        runner.layers[0].link.scale = scale.into();
        runner.refresh_active_mapping_config();
        step(&mut runner);
        map_cc(&mut runner, 0, "layers.0.link.pitch.root");
        let mut played = Vec::new();
        for value in [0, 0, 64] {
            send_cc(&mut runner, 1, value);
            played.extend(step(&mut runner));
        }
        assert_ne!(
            runner.layers[0].link.root, "D",
            "{scale}: the CC moves Root"
        );
        assert_eq!(played.contains(&off), releases, "{scale}: {played:?}");
    }
}

/// A drain run on the runner, with what it sends.
type Drain = fn(&mut NativeRunner) -> Vec<RunnerMessage>;

#[test]
fn every_drain_releases_the_notes_on_the_layers_instruments_whoever_pressed_them() {
    let drains: [(&str, Drain); 5] = [
        ("trigger gate", |runner| {
            runner.toggle_layer_trigger_gate(0);
            runner.messages_without_snapshot().unwrap()
        }),
        ("behavior change", |runner| {
            runner
                .replace_layer_engine_with_config(0, NativeBehavior::Life, Value::Null, None)
                .unwrap();
            runner.messages_without_snapshot().unwrap()
        }),
        ("scan axis", |runner| {
            runner.layers[0].link.scan_mode = ScanMode::Scanning;
            runner.menu.rebuild(runner.menu_config());
            runner.edit_menu_key("layers.0.link.scanAxis", -1).unwrap();
            runner.messages_without_snapshot().unwrap()
        }),
        ("stop", |runner| {
            runner.send(HostMessage::TransportStop).unwrap()
        }),
        ("patch load", |runner| {
            let payload = runner.config_payload();
            runner.apply_config_payload(payload).unwrap();
            runner.messages_without_snapshot().unwrap()
        }),
    ];
    for (name, drain) in drains {
        let (mut runner, off) = held_by_layer_one();
        let drained = drain(&mut runner);
        assert_eq!(words(&drained, false), std::slice::from_ref(&off), "{name}");
        assert_eq!(runner.held_keys.held_count(), 0, "{name}");
        assert!(
            tile(&mut runner, 1, "grid_release", 0, 2).is_empty(),
            "{name}"
        );
    }
}

#[test]
fn a_gate_drain_leaves_the_notes_of_instruments_the_layer_does_not_play() {
    let (mut runner, _) = held_by_layer_one();
    runner.layers[0].link.activate_slot = 1;
    runner.toggle_layer_trigger_gate(0);
    let drained = runner.messages_without_snapshot().unwrap();
    assert!(words(&drained, false).is_empty());
    assert_eq!(runner.held_keys.held(0).len(), 1);
}
