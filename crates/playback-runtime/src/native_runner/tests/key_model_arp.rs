//! A layer's Arp sits between its keys and the instrument, like a hardware arpeggiator: the
//! layer's note-ons and note-offs press and release keys on the Arp's own list, and the Arp
//! plays the instrument.

use super::key_model::{keys_layers, tile};
use super::*;

/// Layer 0 of `keys_layers` with a held Arp in `mode`.
fn held_arp(mode: ArpMode) -> NativeRunner {
    let mut runner = keys_layers(true);
    runner.layers[0].link.arp.mode = mode;
    runner.layers[0].link.arp.source = ArpSource::Held;
    runner
}

/// The notes `words` start, in order.
fn ons(words: Vec<String>) -> Vec<String> {
    words
        .into_iter()
        .filter(|word| word.starts_with('+'))
        .collect()
}

#[test]
fn a_held_arp_plays_the_keys_down_as_they_go_down_and_up() {
    let mut runner = held_arp(ArpMode::ChordStrike);
    let a = ons(tile(&mut runner, 0, "grid_press", 0, 1));
    let b = ons(tile(&mut runner, 0, "grid_press", 0, 3));
    assert_eq!(b.len(), 2, "{b:?}");
    assert_eq!(b[0], a[0], "the key still down plays again");
    assert!(tile(&mut runner, 0, "grid_release", 0, 1).is_empty());
    let c = ons(tile(&mut runner, 0, "grid_press", 0, 5));
    assert_eq!(c.len(), 2, "{c:?}");
    assert!(!c.contains(&a[0]), "a released key leaves the list: {c:?}");
}

#[test]
fn a_held_arp_ignores_another_layers_notes_on_its_instrument() {
    let mut runner = held_arp(ArpMode::ChordStrike);
    let other = tile(&mut runner, 1, "grid_press", 0, 1);
    let own = ons(tile(&mut runner, 0, "grid_press", 0, 3));
    assert_eq!(own.len(), 1, "{own:?}");
    assert_ne!(own, other);
    assert_eq!(runner.link_arp.keys[0].len(), 1);
}

#[test]
fn the_arps_own_notes_never_join_its_list() {
    let mut runner = held_arp(ArpMode::OctaveSpread);
    runner.layers[0].link.arp.octave_spread = 1;
    tile(&mut runner, 0, "grid_press", 0, 1);
    for _ in 0..4 {
        runner
            .send(HostMessage::TransportPulseStep {
                pulses: runner.step_pulses_for_layer(0),
                source: SyncSource::Internal,
                at_ppqn_pulse: None,
                request_snapshot: Some(false),
            })
            .unwrap();
    }
    assert_eq!(runner.link_arp.keys[0].len(), 1);
    let replay = ons(tile(&mut runner, 0, "grid_press", 0, 3));
    assert_eq!(
        replay.len(),
        1,
        "the lowest of two keys, one step at a time: {replay:?}"
    );
    assert_eq!(runner.link_arp.keys[0].len(), 2);
}

#[test]
fn a_transpose_moves_every_key_the_arp_replays() {
    let mut runner = held_arp(ArpMode::ChordStrike);
    let first = ons(tile(&mut runner, 0, "grid_press", 0, 1));
    let note = first[0][1..].parse::<u8>().unwrap();
    runner.handle_play_transpose_grid_press(2, 3);
    let replay = ons(tile(&mut runner, 0, "grid_press", 0, 3));
    assert!(replay.contains(&format!("+{}", note + 2)), "{replay:?}");
    assert!(!replay.contains(&first[0]), "{replay:?}");
}

#[test]
fn a_drain_lets_go_of_the_arps_keys_and_stops_its_pattern() {
    let mut runner = held_arp(ArpMode::Up);
    for y in [1, 3, 5] {
        tile(&mut runner, 0, "grid_press", 0, y);
    }
    assert!(
        !runner.delayed_link_events[0].is_empty(),
        "the pattern waits"
    );
    runner.toggle_layer_trigger_gate(0);
    assert!(runner.delayed_link_events[0].is_empty());
    assert!(runner.link_arp.keys[0].is_empty());
    assert_eq!(runner.held_keys.held(0).len(), 0);
}

/// Turning the Arp on or off moves where the layer's note-offs go, so it drains the layer: no
/// held key is left that no note-off could reach.
#[test]
fn turning_the_arp_on_or_off_releases_the_layers_notes() {
    for (from, to) in [("none", "up"), ("up", "none")] {
        let mut runner = keys_layers(true);
        runner.menu.rebuild(runner.menu_config());
        runner
            .menu
            .set_enum_value_for_key("layers.0.link.arp.mode", from);
        runner
            .apply_or_schedule_menu_key("layers.0.link.arp.mode")
            .unwrap();
        runner
            .menu
            .set_enum_value_for_key("layers.0.link.arp.source", "held");
        runner
            .apply_or_schedule_menu_key("layers.0.link.arp.source")
            .unwrap();
        let on = ons(tile(&mut runner, 0, "grid_press", 0, 1));
        assert_eq!(on.len(), 1, "{from}: {on:?}");
        runner
            .menu
            .set_enum_value_for_key("layers.0.link.arp.mode", to);
        runner
            .apply_or_schedule_menu_key("layers.0.link.arp.mode")
            .unwrap();
        let drained = super::key_model::words(&runner.messages_without_snapshot().unwrap(), false);
        if from == "none" {
            assert_eq!(drained, [on[0].replace('+', "-")], "{from} to {to}");
        }
        assert_eq!(runner.held_keys.held_count(), 0, "{from} to {to}");
        assert!(runner.link_arp.keys[0].is_empty(), "{from} to {to}");
        tile(&mut runner, 0, "grid_release", 0, 1);
        assert_eq!(runner.held_keys.held_count(), 0, "{from} to {to}");
    }
}
