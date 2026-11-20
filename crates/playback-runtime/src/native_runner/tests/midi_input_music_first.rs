use super::midi_note_in::{playing_looper, recorded_events, tile_notes};
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
