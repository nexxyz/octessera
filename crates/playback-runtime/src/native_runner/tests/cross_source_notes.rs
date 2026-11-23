use super::*;
use platform_core::MusicalEvent;

/// Keys on layers 0 and 1, both playing instrument 1 in Hold mode with On note on and Off note
/// off, and the transport playing.
fn two_keys_layers() -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "keys".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner
        .edit_menu_key("instruments.0.noteBehavior", 1)
        .unwrap();
    runner
        .replace_layer_engine_with_config(1, NativeBehavior::Keys, Value::Null, None)
        .unwrap();
    for link in runner
        .layers
        .iter_mut()
        .take(2)
        .map(|layer| &mut layer.link)
    {
        link.event_enabled = true;
        link.activate_slot = 0;
        link.deactivate_slot = 0;
        link.activate_action = TriggerAction::NoteOn;
        link.deactivate_action = TriggerAction::NoteOff;
    }
    runner.transport.transport = RuntimeTransportState::Playing;
    runner
}

/// What a press or release on `layer` sends on the internal (`midi` false) or MIDI lane, as
/// `+note` and `-note` words.
fn tile(runner: &mut NativeRunner, layer: usize, kind: &str, midi: bool) -> Vec<String> {
    runner.select_active_layer(layer).unwrap();
    let messages = device_input(runner, json!({ "type": kind, "x": 0, "y": 2 }));
    words(&messages, midi)
}

fn words(messages: &[RunnerMessage], midi: bool) -> Vec<String> {
    routed_events(messages, midi)
        .into_iter()
        .filter_map(|event| match event {
            MusicalEvent::NoteOn { note, .. } => Some(format!("+{note}")),
            MusicalEvent::NoteOff { note, .. } => Some(format!("-{note}")),
            MusicalEvent::Cc { .. } => None,
        })
        .collect()
}

#[test]
fn a_layer_that_ended_a_shared_pitch_leaves_no_stale_hold_on_the_other() {
    let mut runner = two_keys_layers();
    let on = tile(&mut runner, 0, "grid_press", false);
    assert_eq!(on.len(), 1, "{on:?}");
    assert!(tile(&mut runner, 1, "grid_press", false).len() <= 1);

    let off = tile(&mut runner, 0, "grid_release", false);
    assert_eq!(off, [on[0].replace('+', "-")]);
    tile(&mut runner, 1, "grid_release", false);

    assert_eq!(tile(&mut runner, 1, "grid_press", false), on);
}

#[test]
fn a_layer_note_off_ends_the_note_another_layer_plays_through_play_transpose() {
    let mut runner = two_keys_layers();
    runner.layers[0].link.activate_action = TriggerAction::None;
    runner.transpose.offsets[1] = 2;
    runner.transpose.enabled[1] = true;
    runner.transpose.selected[1] = true;
    let on = tile(&mut runner, 1, "grid_press", false);
    assert_eq!(on.len(), 1, "{on:?}");

    tile(&mut runner, 0, "grid_press", false);
    let off = tile(&mut runner, 0, "grid_release", false);

    assert_eq!(off, [on[0].replace('+', "-")], "the transposed sound ends");
    assert!(runner.held_notes[1].is_empty());
}

#[test]
fn a_layer_note_off_ends_a_midi_slot_note_on_its_own_channel() {
    let mut runner = two_keys_layers();
    runner.instruments[0].kind = InstrumentType::Midi;
    runner.instruments[0].midi_enabled = true;
    runner.instruments[0].midi_channel = 5;
    runner.test_set_midi_enabled(true);
    runner.layers[0].link.activate_action = TriggerAction::None;
    runner.select_active_layer(1).unwrap();
    let pressed = device_input(&mut runner, json!({ "type": "grid_press", "x": 0, "y": 2 }));
    assert_eq!(words(&pressed, true).len(), 1);

    tile(&mut runner, 0, "grid_press", true);
    runner.select_active_layer(0).unwrap();
    let released = device_input(
        &mut runner,
        json!({ "type": "grid_release", "x": 0, "y": 2 }),
    );

    let offs = routed_events(&released, true)
        .into_iter()
        .filter_map(|event| match event {
            MusicalEvent::NoteOff { channel, .. } => Some(channel),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(offs, [4]);
}

#[test]
fn a_layer_note_off_ends_a_notes_in_key_and_a_key_ends_nothing_twice() {
    let mut runner = two_keys_layers();
    runner.test_set_midi_enabled(true);
    runner.midi.notes_in = true;
    runner.layers[0].link.activate_action = TriggerAction::None;
    let pitch = {
        let mut probe = two_keys_layers();
        let on = tile(&mut probe, 0, "grid_press", false);
        on[0].trim_start_matches('+').parse::<u8>().unwrap()
    };
    let key = runner
        .send(HostMessage::MidiNote {
            channel: 0,
            note: pitch,
            velocity: 100,
        })
        .unwrap();
    assert_eq!(words(&key, false), [format!("+{pitch}")]);

    tile(&mut runner, 0, "grid_press", false);
    let off = tile(&mut runner, 0, "grid_release", false);
    assert_eq!(off, [format!("-{pitch}")]);

    let key_up = runner
        .send(HostMessage::MidiNote {
            channel: 0,
            note: pitch,
            velocity: 0,
        })
        .unwrap();
    assert!(words(&key_up, false).is_empty(), "{key_up:?}");
}

#[test]
fn switching_scanning_on_ends_the_notes_the_grid_held() {
    let mut runner = two_keys_layers();
    let on = tile(&mut runner, 0, "grid_press", false);
    runner.menu.rebuild(runner.menu_config());
    runner.edit_menu_key("layers.0.link.scanMode", 1).unwrap();
    let ended = words(&runner.messages_without_snapshot().unwrap(), false);
    assert_eq!(ended, [on[0].replace('+', "-")]);
    assert!(runner.held_notes[0].is_empty());
}

/// Layer 1 holds a Sampler tile's sample; layer 0's Off on another tile of the same pitch finds
/// that voice through the held pitch and releases layer 1's tile.
#[test]
fn a_layer_note_off_releases_another_layers_sampler_tile() {
    let mut runner = two_keys_layers();
    runner.instruments[0].kind = InstrumentType::Sampler;
    runner.instruments[0].sample_assignments = vec![NativeSampleAssignment {
        x: 0,
        y: 2,
        sample_slot: 2,
        level: None,
        tune_semis: 0,
        tune_cents: 0,
    }];
    runner.layers[0].link.activate_action = TriggerAction::None;
    runner.select_active_layer(1).unwrap();
    let pressed = device_input(&mut runner, json!({ "type": "grid_press", "x": 0, "y": 2 }));
    let started = tile_hits(&pressed);
    assert_eq!(started.len(), 1, "{pressed:?}");

    runner.select_active_layer(0).unwrap();
    device_input(&mut runner, json!({ "type": "grid_press", "x": 0, "y": 2 }));
    let released = device_input(
        &mut runner,
        json!({ "type": "grid_release", "x": 0, "y": 2 }),
    );

    let releases = tile_hits(&released);
    assert_eq!(releases.len(), 1, "{released:?}");
    assert_eq!(
        (releases[0].tile, releases[0].velocity),
        (started[0].tile, 0)
    );
    assert!(runner.held_notes[1].is_empty());
}

fn tile_hits(messages: &[RunnerMessage]) -> Vec<crate::protocol::TileHit> {
    messages
        .iter()
        .flat_map(|message| match message {
            RunnerMessage::TileHits { hits } => hits.clone(),
            _ => Vec::new(),
        })
        .collect()
}
