use super::*;
use platform_core::MusicalEvent;

fn keys_runner() -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "keys".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.transport.transport = RuntimeTransportState::Playing;
    runner
}

fn grid(runner: &mut NativeRunner, kind: &str, y: usize) -> Vec<(u8, Option<u32>)> {
    let messages = device_input(runner, json!({ "type": kind, "x": 0, "y": y }));
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

    let pressed = grid(&mut runner, "grid_press", 3);
    assert!(matches!(pressed[..], [(_, Some(200))]), "{pressed:?}");
}

#[test]
pub(crate) fn a_held_arp_on_a_one_shot_instrument_plays_the_lit_tiles() {
    let mut runner = keys_runner();
    runner.layers[0].link.arp.mode = ArpMode::ChordStrike;
    runner.layers[0].link.arp.source = ArpSource::Held;

    let first = grid(&mut runner, "grid_press", 1);
    assert_eq!(first.len(), 1, "{first:?}");
    let second = grid(&mut runner, "grid_press", 3);
    assert_eq!(second.len(), 2, "both lit tiles play: {second:?}");
    assert!(second.iter().any(|(note, _)| *note == first[0].0));

    grid(&mut runner, "grid_release", 1);
    let third = grid(&mut runner, "grid_press", 5);
    assert_eq!(third.len(), 2, "a dark tile stops playing: {third:?}");
    assert!(third.iter().all(|(note, _)| *note != first[0].0));
}
