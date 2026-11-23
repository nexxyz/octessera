//! The instrument keyboards as sources play them: a note-on presses a key (the note as played,
//! before Play Transpose), a note-off releases it from any source, and a key nothing releases
//! keeps sounding.

use super::*;
use crate::runtime::MidiNote;
use platform_core::{MusicalEvent, TileTriggerIntent, TileTriggerKind};

/// Keys on layers 0 and 1, both playing instrument 1 with On note on and Off note off, the
/// instrument in Hold mode when `hold`, and the transport playing.
pub(super) fn keys_layers(hold: bool) -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "keys".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    if hold {
        runner
            .edit_menu_key("instruments.0.noteBehavior", 1)
            .unwrap();
    }
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

/// The notes in `messages` on the internal (`midi` false) or MIDI lane, as `+note` and `-note`.
pub(super) fn words(messages: &[RunnerMessage], midi: bool) -> Vec<String> {
    routed_events(messages, midi)
        .into_iter()
        .filter_map(|event| match event {
            MusicalEvent::NoteOn { note, .. } => Some(format!("+{note}")),
            MusicalEvent::NoteOff { note, .. } => Some(format!("-{note}")),
            MusicalEvent::Cc { .. } => None,
        })
        .collect()
}

/// What a press or release of tile `x`, `y` on `layer` plays on the internal lane.
pub(super) fn tile(
    runner: &mut NativeRunner,
    layer: usize,
    kind: &str,
    x: usize,
    y: usize,
) -> Vec<String> {
    runner.select_active_layer(layer).unwrap();
    words(
        &device_input(runner, json!({ "type": kind, "x": x, "y": y })),
        false,
    )
}

/// Plays a note-on (`velocity` > 0) or note-off of key `note` from layer 0 as its step would.
fn play(runner: &mut NativeRunner, note: u8, velocity: u8, midi: bool) -> Vec<String> {
    let kind = if velocity > 0 {
        TileTriggerKind::Activate
    } else {
        TileTriggerKind::Deactivate
    };
    let event = if velocity > 0 {
        MusicalEvent::NoteOn {
            channel: 0,
            note,
            velocity,
            duration_ms: None,
        }
    } else {
        MusicalEvent::NoteOff { channel: 0, note }
    };
    let intent = TileTriggerIntent {
        x: 0,
        y: 0,
        degree: 0,
        kind,
    };
    let model = runner.active_engine().model().unwrap();
    let messages = runner
        .messages_with_layer_input_result(
            0,
            platform_core::NativeInputResult {
                events: vec![event],
                emitted_events: vec![],
                mapped_intents: vec![],
                event_intents: vec![Some(intent)],
                model,
            },
        )
        .unwrap();
    words(&messages, midi)
}

fn midi_slot(runner: &mut NativeRunner) {
    runner.test_set_midi_enabled(true);
    let slot = &mut runner.instruments[0];
    slot.kind = InstrumentType::Midi;
    slot.midi_enabled = true;
    slot.midi_channel = 3;
}

#[test]
fn a_held_key_pressed_again_strikes_again_on_a_hold_instrument() {
    let mut runner = keys_layers(true);
    let on = tile(&mut runner, 0, "grid_press", 0, 2);
    let [on] = &on[..] else { panic!("{on:?}") };
    let off = on.replace('+', "-");

    assert_eq!(
        tile(&mut runner, 0, "grid_press", 5, 2),
        [off.clone(), on.clone()]
    );
    assert_eq!(tile(&mut runner, 0, "grid_release", 0, 2), [off]);
    assert!(tile(&mut runner, 0, "grid_release", 5, 2).is_empty());
}

#[test]
fn a_key_nothing_releases_keeps_sounding() {
    let mut runner = keys_layers(true);
    runner.layers[0].link.deactivate_action = TriggerAction::None;
    runner.refresh_active_mapping_config();
    tile(&mut runner, 0, "grid_press", 0, 2);
    assert!(tile(&mut runner, 0, "grid_release", 0, 2).is_empty());
    for _ in 0..96 {
        let step = runner
            .send(HostMessage::TransportPulseStep {
                pulses: 1,
                source: SyncSource::Internal,
                at_ppqn_pulse: None,
                request_snapshot: Some(false),
            })
            .unwrap();
        assert!(words(&step, false).is_empty());
    }
    assert_eq!(runner.held_keys.held_count(), 1);
}

#[test]
fn a_transposed_key_ends_by_the_key_as_played_and_only_that_key() {
    let mut runner = keys_layers(true);
    let on = tile(&mut runner, 0, "grid_press", 0, 2);
    tile(&mut runner, 0, "grid_press", 0, 4);
    runner.handle_play_transpose_grid_press(2, 3);
    assert_eq!(runner.play_transpose_offset_for_layer(0), 2);
    let messages = runner.messages_without_snapshot().unwrap();
    assert!(
        words(&messages, false).is_empty(),
        "a transpose ends nothing"
    );

    assert_eq!(
        tile(&mut runner, 0, "grid_release", 0, 2),
        [on[0].replace('+', "-")]
    );
    assert_eq!(runner.held_keys.held_count(), 1);
}

#[test]
fn a_key_that_sounds_like_a_held_one_shares_its_voice_until_either_releases() {
    for midi in [false, true] {
        let mut runner = keys_layers(true);
        if midi {
            midi_slot(&mut runner);
        }
        assert_eq!(play(&mut runner, 60, 100, midi), ["+60"]);
        runner.transpose.offsets[0] = 2;
        assert_eq!(
            play(&mut runner, 58, 100, midi),
            ["-60", "+60"],
            "midi {midi}"
        );
        assert_eq!(play(&mut runner, 58, 0, midi), ["-60"], "midi {midi}");
        assert!(play(&mut runner, 60, 0, midi).is_empty(), "midi {midi}");
    }
}

#[test]
fn two_keys_clamped_to_one_pitch_share_it() {
    let mut runner = keys_layers(true);
    runner.transpose.offsets[0] = 2;
    assert_eq!(play(&mut runner, 126, 100, false), ["+127"]);
    assert_eq!(play(&mut runner, 127, 100, false), ["-127", "+127"]);
    assert_eq!(play(&mut runner, 127, 0, false), ["-127"]);
    assert!(play(&mut runner, 126, 0, false).is_empty());
}

#[test]
fn a_notes_in_note_off_ends_a_layers_key() {
    let mut runner = keys_layers(true);
    runner.test_set_midi_enabled(true);
    runner.midi.notes_in = true;
    let on = tile(&mut runner, 0, "grid_press", 0, 2);
    let note = on[0][1..].parse::<u8>().unwrap();
    let released = runner
        .send_midi_note(MidiNote {
            channel: 0,
            note,
            velocity: 0,
        })
        .unwrap();
    assert_eq!(words(&released, false), [format!("-{note}")]);
    assert!(tile(&mut runner, 0, "grid_release", 0, 2).is_empty());
}

#[test]
fn a_mapped_note_off_cuts_a_one_shot_note_and_ends_nothing_after_its_length() {
    let mut runner = keys_layers(false);
    let pulse = |runner: &mut NativeRunner, pulses: u32| {
        runner
            .send(HostMessage::TransportPulseStep {
                pulses,
                source: SyncSource::Internal,
                at_ppqn_pulse: None,
                request_snapshot: Some(false),
            })
            .unwrap();
    };
    let on = tile(&mut runner, 0, "grid_press", 0, 2);
    pulse(&mut runner, 3);
    assert_eq!(
        tile(&mut runner, 0, "grid_release", 0, 2),
        [on[0].replace('+', "-")]
    );

    tile(&mut runner, 0, "grid_press", 0, 2);
    pulse(&mut runner, 8);
    assert!(
        tile(&mut runner, 0, "grid_release", 0, 2).is_empty(),
        "150 ms are over after 8 pulses at 120 BPM"
    );
}

#[test]
fn presses_of_one_key_on_one_tick_merge_into_the_loudest_or_the_first() {
    let mut runner = keys_layers(false);
    runner.layers[1].link.one_shot_length_ms = 300;
    let press = |runner: &mut NativeRunner, layer: usize, velocity: u8| {
        runner
            .route_events_with_link_timing(
                layer,
                super::super::algorithm::LinkRoutingInput {
                    events: vec![MusicalEvent::NoteOn {
                        channel: 0,
                        note: 60,
                        velocity,
                        duration_ms: Some(if layer == 0 { 150 } else { 300 }),
                    }],
                    event_intents: &[Some(TileTriggerIntent {
                        x: 0,
                        y: 0,
                        degree: 0,
                        kind: TileTriggerKind::Activate,
                    })],
                    transpose_offset: 0,
                },
            )
            .unwrap()
    };
    for (velocities, kept) in [([80, 100], (100, 300)), ([90, 90], (90, 150))] {
        let mut tick = press(&mut runner, 0, velocities[0]);
        tick.extend(press(&mut runner, 1, velocities[1]));
        runner.sound_keys(&mut tick);
        assert_eq!(
            tick.audio,
            [MusicalEvent::NoteOn {
                channel: 0,
                note: 60,
                velocity: kept.0,
                duration_ms: Some(kept.1),
            }],
            "{velocities:?}"
        );
    }
}
