use super::in_place_menu_actions::press_key;
use super::midi_note_in::{audio, keyboard_runner, playing_looper, recorded_events, tile_notes};
use super::*;
use crate::{MidiControlChange, MidiNote};

fn snapshots(messages: &[RunnerMessage]) -> usize {
    messages
        .iter()
        .filter(|message| matches!(message, RunnerMessage::Snapshot { .. }))
        .count()
}

fn send_music_first_cc(runner: &mut NativeRunner, value: u8) -> Vec<RunnerMessage> {
    runner
        .send_music_first(
            MidiControlChange::parse(&[0xB0, 20, value])
                .expect("a Control Change")
                .into(),
        )
        .unwrap()
}

#[test]
fn a_spent_autosave_deadline_costs_one_snapshot_not_one_per_later_midi_message() {
    let note = tile_notes()[GRID_WIDTH * 3 + 5];
    let mut runner = playing_looper();
    runner.auto_save_default = true;
    let note_on = MidiNote::parse(&[0x90, note, 100]).expect("a note");
    runner.send_music_first(note_on.into()).unwrap();
    assert!(!recorded_events(&runner, 0).is_empty());
    runner.make_deferred_work_due_now();

    let first = send_music_first_cc(&mut runner, 0);
    let later = (1..=8)
        .map(|value| snapshots(&send_music_first_cc(&mut runner, value)))
        .sum::<usize>();

    assert!(snapshots(&first) <= 1);
    assert_eq!(later, 0, "every CC after the deadline drew a full snapshot");
    let intent = runner
        .persistence_intent_at(std::time::Instant::now())
        .expect("the Looper recording still autosaves");
    assert!(intent.default_eligible());
    assert_eq!(Some(intent.revision()), runner.dirty_revision);
}

fn send_music_first_note(runner: &mut NativeRunner, bytes: [u8; 3]) -> Vec<RunnerMessage> {
    runner
        .send_music_first(MidiNote::parse(&bytes).expect("a note").into())
        .unwrap()
}

#[test]
fn playing_midi_notes_and_ccs_leave_the_event_dot_to_the_next_display_tick() {
    let mut runner = keyboard_runner("keys");
    runner.transport.transport = RuntimeTransportState::Playing;
    runner.capture_display_scene().unwrap();

    let mut played = Vec::new();
    for note in [60, 64, 67] {
        let pressed = send_music_first_note(&mut runner, [0x90, note, 100]);
        let released = send_music_first_note(&mut runner, [0x80, note, 0]);
        let changed = send_music_first_cc(&mut runner, note);
        for messages in [&pressed, &released, &changed] {
            assert_eq!(
                snapshots(messages),
                0,
                "a playing MIDI message drew a snapshot"
            );
        }
        played.extend(audio(&pressed));
    }

    assert_eq!(played.len(), 3, "every note still sounds: {played:?}");
    assert!(runner.display_scene_pending());
    assert!(runner
        .capture_display_scene()
        .unwrap()
        .into_snapshot()
        .is_object());
}

#[test]
fn stopped_midi_notes_still_redraw_the_display_with_each_note() {
    let mut runner = keyboard_runner("keys");
    runner.capture_display_scene().unwrap();

    let pressed = send_music_first_note(&mut runner, [0x90, 60, 100]);

    assert_eq!(audio(&pressed).len(), 1);
    assert_eq!(snapshots(&pressed), 1);
}

#[test]
fn a_playing_cc_learn_shows_its_toast_on_the_next_display_tick() {
    let mut runner = keyboard_runner("keys");
    press_key(&mut runner, "midiCc.2.learn");
    runner.transport.transport = RuntimeTransportState::Playing;
    runner.capture_display_scene().unwrap();

    let learned = send_music_first_cc(&mut runner, 99);

    assert_eq!(snapshots(&learned), 0);
    assert_eq!(runner.midi_cc_mappings[2].cc, 20);
    assert!(runner.display_scene_pending());
    assert_eq!(
        runner.display.toast.as_ref().unwrap().message,
        "CC3: learned CC 20 ch 1"
    );
}
