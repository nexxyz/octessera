use super::*;
use platform_core::{MusicalEvent, TriggerAction};

fn keys_runner() -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "keys".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.transport.transport = RuntimeTransportState::Playing;
    runner
}

fn held_arp_runner() -> NativeRunner {
    let mut runner = keys_runner();
    runner.layers[0].link.arp.mode = ArpMode::ChordStrike;
    runner.layers[0].link.arp.source = ArpSource::Held;
    runner
}

fn grid(runner: &mut NativeRunner, kind: &str, x: usize, y: usize) -> Vec<(u8, Option<u32>)> {
    let messages = device_input(runner, json!({ "type": kind, "x": x, "y": y }));
    routed_events(&messages, false)
        .into_iter()
        .filter_map(|event| match event {
            MusicalEvent::NoteOn {
                note, duration_ms, ..
            } => Some((note, duration_ms)),
            _ => None,
        })
        .collect()
}

fn pitches(notes: &[(u8, Option<u32>)]) -> Vec<u8> {
    let mut pitches: Vec<u8> = notes.iter().map(|(note, _)| *note).collect();
    pitches.sort_unstable();
    pitches
}

#[test]
pub(crate) fn a_one_shot_note_lasts_the_one_shot_len_its_link_menu_row_shows() {
    let mut runner = keys_runner();
    assert!(runner.menu.focus_item_key("layers.0.link.oneShotLengthMs"));
    runner.menu.state.editing = true;
    device_input(
        &mut runner,
        json!({ "type": "encoder_turn", "delta": 2, "id": "main" }),
    );
    runner.menu.state.editing = false;
    assert_eq!(runner.layers[0].link.one_shot_length_ms, 200);

    let pressed = grid(&mut runner, "grid_press", 0, 3);
    assert!(matches!(pressed[..], [(_, Some(200))]), "{pressed:?}");
}

#[test]
pub(crate) fn a_held_arp_on_a_one_shot_instrument_plays_the_lit_tiles() {
    let mut runner = held_arp_runner();

    let first = grid(&mut runner, "grid_press", 0, 1);
    assert_eq!(first.len(), 1, "{first:?}");
    let second = grid(&mut runner, "grid_press", 0, 3);
    assert_eq!(second.len(), 2, "both lit tiles play: {second:?}");
    assert!(second.iter().any(|(note, _)| *note == first[0].0));

    grid(&mut runner, "grid_release", 0, 1);
    let third = grid(&mut runner, "grid_press", 0, 5);
    assert_eq!(third.len(), 2, "a dark tile stops playing: {third:?}");
    assert!(third.iter().all(|(note, _)| *note != first[0].0));
}

#[test]
pub(crate) fn a_dark_tile_leaves_the_held_arp_even_when_its_off_plays_nothing() {
    let mut runner = held_arp_runner();
    runner.layers[0].link.deactivate_action = TriggerAction::None;
    runner.refresh_active_mapping_config();

    let first = grid(&mut runner, "grid_press", 0, 1);
    grid(&mut runner, "grid_release", 0, 1);
    let second = grid(&mut runner, "grid_press", 0, 3);
    assert_eq!(second.len(), 1, "{second:?}");
    assert_ne!(second[0].0, first[0].0);
}

#[test]
pub(crate) fn a_pitch_stays_held_while_another_lit_tile_still_plays_it() {
    let mut runner = held_arp_runner();

    let shared = grid(&mut runner, "grid_press", 0, 3);
    let both = grid(&mut runner, "grid_press", 2, 3);
    assert_eq!(pitches(&both), pitches(&shared), "one voice per pitch");
    grid(&mut runner, "grid_release", 0, 3);
    let after = grid(&mut runner, "grid_press", 0, 5);
    assert_eq!(after.len(), 2, "{after:?}");
    assert!(after.iter().any(|(note, _)| *note == shared[0].0));
}

#[test]
pub(crate) fn muting_the_layer_or_stopping_forgets_its_lit_tiles() {
    let mut runner = held_arp_runner();
    grid(&mut runner, "grid_press", 0, 1);
    assert_eq!(runner.link_arp.held_notes[0].len(), 1);
    runner.toggle_layer_trigger_gate(0);
    assert!(runner.link_arp.held_notes[0].is_empty());

    runner.toggle_layer_trigger_gate(0);
    grid(&mut runner, "grid_press", 0, 3);
    assert_eq!(runner.link_arp.held_notes[0].len(), 1);
    runner.reset_transport_position();
    assert!(runner.link_arp.held_notes[0].is_empty());
}
