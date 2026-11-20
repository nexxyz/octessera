use super::device_config_apply::{board_runner, save_setting_payload};
use super::midi_note_in::{
    audio, keyboard_runner, midi, note_off, note_on, playing_looper, recorded_events, send_note,
    tile_notes, NOTES_IN,
};
use super::*;
use crate::native_runner::midi_drum::NativeMidiDrumTile;
use crate::RuntimePlatformRequest;

fn load_system(runner: &mut NativeRunner, payload: Value) {
    let request = RuntimePlatformRequest::new(
        RuntimePlatformEffect::StoreLoadSystem,
        "load-system".into(),
        None,
    );
    runner.register_platform_request(&request);
    runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::LoadSystemResult {
                payload: Some(payload),
            }
            .with_identity(request.request_id, request.revision),
        })
        .unwrap();
}

#[test]
fn a_key_held_while_a_system_load_turns_notes_in_off_still_releases() {
    let notes_in_off = save_setting_payload(&mut board_runner(false), NOTES_IN, -1, "save");
    let mut runner = keyboard_runner("keys");
    send_note(&mut runner, [0x90, 60, 100]);

    load_system(&mut runner, notes_in_off);
    assert!(!runner.midi.notes_in);
    let messages = send_note(&mut runner, [0x80, 60, 0]);

    assert!(audio(&messages).contains(&note_off(0, 60)), "{messages:?}");
    let messages = send_note(&mut runner, [0x90, 60, 100]);
    assert!(audio(&messages).is_empty(), "new notes stay ignored");
}

#[test]
fn a_sampler_slot_plays_only_notes_36_to_43() {
    let mut runner = keyboard_runner("keys");
    runner.instruments[0].kind = InstrumentType::Sampler;

    for outside in [35, 44, 100] {
        let messages = send_note(&mut runner, [0x90, outside, 100]);
        assert!(audio(&messages).is_empty(), "note {outside}: {messages:?}");
        let messages = send_note(&mut runner, [0x80, outside, 0]);
        assert!(audio(&messages).is_empty(), "note {outside}: {messages:?}");
    }
    for inside in [36, 43] {
        let messages = send_note(&mut runner, [0x90, inside, 100]);
        assert_eq!(audio(&messages).len(), 1, "note {inside}");
    }
}

#[test]
fn a_midi_drum_slot_records_its_lowest_tile_stamped_with_the_note() {
    let mut runner = playing_looper();
    runner.instruments[0].kind = InstrumentType::MidiDrum;
    runner.instruments[0].midi_drum.settings.enabled = true;
    let stamp = |x, y, note| NativeMidiDrumTile {
        x,
        y,
        note,
        velocity: 100,
    };
    runner.instruments[0].midi_drum.assignments =
        vec![stamp(4, 2, 50), stamp(3, 1, 50), stamp(0, 0, 36)];

    let messages = send_note(&mut runner, [0x90, 50, 100]);
    send_note(&mut runner, [0x80, 50, 0]);

    assert!(!midi(&messages).is_empty(), "the note is forwarded");
    let tile = GRID_WIDTH + 3;
    assert_eq!(
        runner.layers[0].engine.serialized_state().unwrap()["steps"],
        json!([
            [{ "tile": tile, "kind": "release" }],
            [{ "tile": tile, "kind": "press" }],
        ])
    );

    let mut runner = playing_looper();
    runner.instruments[0].kind = InstrumentType::MidiDrum;
    runner.instruments[0].midi_drum.settings.enabled = true;
    let unstamped = tile_notes()[0];
    runner.instruments[0].midi_drum.assignments = vec![stamp(0, 0, unstamped.wrapping_add(1))];
    send_note(&mut runner, [0x90, unstamped, 100]);
    assert!(
        recorded_events(&runner, 0).is_empty(),
        "a note no tile is stamped with is not recorded, even if the scale would map it"
    );
}

fn recorded_press_tile(runner: &NativeRunner) -> Option<u64> {
    recorded_events(runner, 0)
        .iter()
        .find(|event| event["kind"] == "press")
        .and_then(|event| event["tile"].as_u64())
}

#[test]
fn recording_undoes_the_play_transpose_the_layer_plays_with() {
    let notes = tile_notes();
    let tile = GRID_WIDTH * 3 + 5;
    let note = notes[tile];
    let lowest = notes.iter().position(|played| *played == note).unwrap() as u64;

    for (enabled, selected, played) in [
        (true, true, note + 2),
        (false, true, note),
        (true, false, note),
    ] {
        let mut runner = playing_looper();
        runner.transpose.offsets[0] = 2;
        runner.transpose.enabled[0] = enabled;
        runner.transpose.selected[0] = selected;
        assert_eq!(
            runner.play_transpose_offset_for_layer(0),
            if enabled && selected { 2 } else { 0 }
        );

        send_note(&mut runner, [0x90, played, 100]);

        assert_eq!(
            recorded_press_tile(&runner),
            Some(lowest),
            "enabled {enabled}, selected {selected}"
        );
    }
}

/// I1 as a MIDI Inst. or MIDI Drum on channel 1, the slot's own default, with the keyboard on
/// channel 1 too.
fn echoing_runner(kind: InstrumentType, note: u8) -> (NativeRunner, std::time::Instant) {
    let mut runner = playing_looper();
    let instrument = &mut runner.instruments[0];
    instrument.kind = kind;
    instrument.midi_enabled = true;
    instrument.midi_channel = 1;
    instrument.midi_drum.settings.enabled = true;
    instrument.midi_drum.settings.channel = 1;
    instrument.midi_drum.assignments = vec![NativeMidiDrumTile {
        x: 0,
        y: 0,
        note,
        velocity: 100,
    }];
    let start = std::time::Instant::now();
    runner.display.transients.set_test_now(start);
    (runner, start)
}

fn at(runner: &mut NativeRunner, start: std::time::Instant, ms: u64) {
    runner
        .display
        .transients
        .set_test_now(start + std::time::Duration::from_millis(ms));
}

#[test]
fn a_keyboard_note_forwarded_on_its_own_channel_ignores_its_echo() {
    let note = tile_notes()[0];
    for kind in [InstrumentType::Midi, InstrumentType::MidiDrum] {
        let (mut runner, start) = echoing_runner(kind, note);

        let messages = send_note(&mut runner, [0x90, note, 100]);
        assert_eq!(
            midi(&messages),
            vec![note_on(0, note, 100, None)],
            "{kind:?}"
        );
        at(&mut runner, start, 10);
        let echo = send_note(&mut runner, [0x90, note, 100]);
        assert!(
            midi(&echo).is_empty() && audio(&echo).is_empty(),
            "{kind:?}"
        );
        assert_eq!(
            recorded_events(&runner, 0).len(),
            1,
            "{kind:?}: echo not recorded"
        );

        at(&mut runner, start, 20);
        let messages = send_note(&mut runner, [0x80, note, 0]);
        assert_eq!(midi(&messages), vec![note_off(0, note)], "{kind:?}");
        at(&mut runner, start, 22);
        send_note(&mut runner, [0x90, note, 90]);
        at(&mut runner, start, 25);
        let echo = send_note(&mut runner, [0x80, note, 0]);
        assert!(
            midi(&echo).is_empty(),
            "{kind:?}: the echoed note-off leaves the key pressed again held"
        );
        let messages = send_note(&mut runner, [0x80, note, 0]);
        assert_eq!(midi(&messages), vec![note_off(0, note)], "{kind:?}");
    }
}

#[test]
fn the_same_note_after_the_echo_window_or_at_another_velocity_plays() {
    let note = tile_notes()[0];
    let (mut runner, start) = echoing_runner(InstrumentType::Midi, note);
    send_note(&mut runner, [0x90, note, 100]);
    send_note(&mut runner, [0x80, note, 0]);

    at(&mut runner, start, 5);
    let messages = send_note(&mut runner, [0x90, note, 99]);
    assert_eq!(midi(&messages), vec![note_on(0, note, 99, None)]);
    send_note(&mut runner, [0x80, note, 0]);

    let (mut runner, start) = echoing_runner(InstrumentType::Midi, note);
    send_note(&mut runner, [0x90, note, 100]);
    send_note(&mut runner, [0x80, note, 0]);
    at(&mut runner, start, 31);
    let messages = send_note(&mut runner, [0x90, note, 100]);
    assert_eq!(midi(&messages), vec![note_on(0, note, 100, None)]);
}
