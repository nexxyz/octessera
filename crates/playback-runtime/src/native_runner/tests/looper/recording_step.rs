//! Grid presses and keyboard notes land on the same Looper step: the nearest one while
//! playing, the one the playhead waits on while stopped.

use super::super::midi_note_in::{playing_looper, send_note, tile_notes};
use super::*;

const TILE: (usize, usize) = (2, 3);

fn pulses(runner: &mut NativeRunner, pulses: u32) -> Vec<RunnerMessage> {
    runner
        .send(HostMessage::TransportPulseStep {
            pulses,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap()
}

fn note_ons(messages: &[RunnerMessage]) -> usize {
    messages
        .iter()
        .map(|message| match message {
            RunnerMessage::MusicalEvents { events } => events
                .iter()
                .filter(|event| matches!(event, platform_core::MusicalEvent::NoteOn { .. }))
                .count(),
            _ => 0,
        })
        .sum()
}

fn grid(runner: &mut NativeRunner, kind: &str) -> Vec<RunnerMessage> {
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": kind, "x": TILE.0, "y": TILE.1 }),
            request_snapshot: None,
        })
        .unwrap()
}

/// Plays a short press and release on the grid or the keyboard and returns the note-ons it
/// sounded live.
fn tap(runner: &mut NativeRunner, keyboard: bool, note: u8) -> usize {
    if keyboard {
        let played = note_ons(&send_note(runner, [0x90, note, 100]));
        send_note(runner, [0x80, note, 0]);
        played
    } else {
        let played = note_ons(&grid(runner, "grid_press"));
        grid(runner, "grid_release");
        played
    }
}

fn steps(runner: &NativeRunner) -> Value {
    runner.active_engine().serialized_state().unwrap()["steps"].clone()
}

fn tile_index(keyboard: bool, notes: &[u8]) -> usize {
    if keyboard {
        notes.iter().position(|played| *played == notes[0]).unwrap()
    } else {
        platform_core::grid_index(TILE.0, TILE.1)
    }
}

#[test]
fn a_tap_early_in_a_step_lands_on_that_step_and_replays_next_pass() {
    let notes = tile_notes();
    for keyboard in [false, true] {
        let mut runner = playing_looper();
        let tile = tile_index(keyboard, &notes);
        pulses(&mut runner, 12);
        pulses(&mut runner, 5);

        assert_eq!(tap(&mut runner, keyboard, notes[0]), 1, "sounds live");

        assert_eq!(
            steps(&runner),
            json!([
                [{ "tile": tile, "kind": "press" }],
                [{ "tile": tile, "kind": "release" }],
            ]),
            "keyboard {keyboard}: press on the sounding step, release after it"
        );
        assert_eq!(note_ons(&pulses(&mut runner, 7)), 0, "keyboard {keyboard}");
        assert_eq!(note_ons(&pulses(&mut runner, 12)), 1, "keyboard {keyboard}");
    }
}

#[test]
fn a_tap_late_in_a_step_lands_on_the_next_step_without_a_double_hit() {
    let notes = tile_notes();
    for keyboard in [false, true] {
        let mut runner = playing_looper();
        let tile = tile_index(keyboard, &notes);
        pulses(&mut runner, 12);
        pulses(&mut runner, 6);

        assert_eq!(tap(&mut runner, keyboard, notes[0]), 1, "sounds live");

        assert_eq!(
            steps(&runner),
            json!([
                [{ "tile": tile, "kind": "release" }],
                [{ "tile": tile, "kind": "press" }],
            ]),
            "keyboard {keyboard}: press on the next step, release after it"
        );
        assert_eq!(
            note_ons(&pulses(&mut runner, 6)),
            0,
            "keyboard {keyboard}: the next step skips what just sounded"
        );
        assert_eq!(note_ons(&pulses(&mut runner, 12)), 0, "keyboard {keyboard}");
        assert_eq!(
            note_ons(&pulses(&mut runner, 12)),
            1,
            "keyboard {keyboard}: the next pass plays it"
        );
    }
}

fn single_step(runner: &mut NativeRunner) {
    for input in [
        json!({ "type": "button_fn", "pressed": true }),
        json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
        json!({ "type": "button_fn", "pressed": false }),
    ] {
        runner
            .send(HostMessage::DeviceInput {
                input,
                request_snapshot: None,
            })
            .unwrap();
    }
}

#[test]
fn a_tap_while_stopped_lands_on_the_step_the_playhead_waits_on() {
    let notes = tile_notes();
    for keyboard in [false, true] {
        let mut runner = playing_looper();
        runner.send(HostMessage::TransportStop).unwrap();
        single_step(&mut runner);
        assert_eq!(looper_mode_and_step(&runner).1, 1);

        tap(&mut runner, keyboard, notes[0]);

        let tile = tile_index(keyboard, &notes);
        assert_eq!(
            steps(&runner),
            json!([
                [{ "tile": tile, "kind": "release" }],
                [{ "tile": tile, "kind": "press" }],
            ]),
            "keyboard {keyboard}: on step 2, not step 1"
        );
    }
}
