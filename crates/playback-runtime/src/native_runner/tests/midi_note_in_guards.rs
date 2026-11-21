use super::device_config_apply::{board_runner, save_setting_payload};
use super::midi_note_in::{
    audio, keyboard_runner, midi, note_off, playing_looper, recorded_events, send_note, tile_notes,
    NOTES_IN,
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
            [{ "tile": tile, "kind": "press" }],
            [{ "tile": tile, "kind": "release" }],
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

#[test]
fn a_keyboard_note_forwarded_on_its_own_channel_ignores_its_echo() {
    use crate::tests::midi_echo::{dispatch_note, midi_out_runtime};
    use crate::tests::support::FakeHost;
    use std::time::{Duration, Instant};

    for kind in [InstrumentType::Midi, InstrumentType::MidiDrum] {
        let mut runner = keyboard_runner("keys");
        runner.midi.output_id = Some("out".into());
        let instrument = &mut runner.instruments[0];
        instrument.kind = kind;
        instrument.midi_enabled = true;
        instrument.midi_channel = 1;
        instrument.midi_drum.settings.enabled = true;
        instrument.midi_drum.settings.channel = 1;
        let mut runtime = midi_out_runtime();
        let start = Instant::now();
        let mut host = FakeHost {
            clock: Some(start),
            ..FakeHost::default()
        };
        let at = |host: &mut FakeHost, ms| host.clock = Some(start + Duration::from_millis(ms));
        let sent = |host: &FakeHost| {
            host.midi_messages
                .iter()
                .filter(|bytes| bytes.len() == 3 && bytes[0] & 0xE0 == 0x80)
                .cloned()
                .collect::<Vec<_>>()
        };

        dispatch_note(&mut runtime, &mut runner, &mut host, [0x90, 60, 100]);
        assert_eq!(sent(&host), vec![vec![0x90, 60, 100]], "{kind:?}: forwards");
        at(&mut host, 10);
        dispatch_note(&mut runtime, &mut runner, &mut host, [0x90, 60, 100]);
        assert_eq!(sent(&host).len(), 1, "{kind:?}: its echo is dropped");

        at(&mut host, 20);
        dispatch_note(&mut runtime, &mut runner, &mut host, [0x80, 60, 0]);
        at(&mut host, 22);
        dispatch_note(&mut runtime, &mut runner, &mut host, [0x90, 60, 90]);
        at(&mut host, 25);
        dispatch_note(&mut runtime, &mut runner, &mut host, [0x80, 60, 0]);
        assert_eq!(
            sent(&host),
            vec![
                vec![0x90, 60, 100],
                vec![0x80, 60, 0],
                vec![0x90, 60, 90],
                vec![0x80, 60, 0]
            ],
            "{kind:?}: a key-up inside the window of the sent note-off still releases the key"
        );
        dispatch_note(&mut runtime, &mut runner, &mut host, [0x90, 60, 99]);
        assert_eq!(sent(&host).len(), 5, "{kind:?}: another velocity plays");
    }
}
