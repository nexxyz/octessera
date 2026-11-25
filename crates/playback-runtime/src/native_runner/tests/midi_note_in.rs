use super::device_config_apply::{board_runner, reload_system_payload, save_setting_payload};
use super::in_place_menu_actions::{confirm_key, press_key};
use super::looper::looper_mode_and_step;
use super::*;
use crate::native_help::resolve_native_help;
use crate::{MidiControlChange, MidiNote};

pub(super) const NOTES_IN: &str = "midi.notesIn";
pub(super) const MIDI_SLOT: usize = 2;
pub(super) const MIDI_OUT_CHANNEL: u8 = 5;

pub(super) fn keyboard_runner(behavior: &str) -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: behavior.into(),
        behavior_config: if behavior == "looper" {
            json!({ "mode": "overdub", "lengthSteps": 2 })
        } else {
            Value::Null
        },
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.test_set_midi_enabled(true);
    runner.instruments[0].note_behavior = NoteBehavior::Hold;
    runner.instruments[MIDI_SLOT].kind = InstrumentType::Midi;
    runner.instruments[MIDI_SLOT].midi_enabled = true;
    runner.instruments[MIDI_SLOT].midi_channel = MIDI_OUT_CHANNEL + 1;
    runner.menu.rebuild(runner.menu_config());
    runner.sync_engine_runtime_config();
    runner
}

pub(super) fn send_note(runner: &mut NativeRunner, bytes: [u8; 3]) -> Vec<RunnerMessage> {
    runner
        .send(MidiNote::parse(&bytes).expect("a note").into())
        .unwrap()
}

pub(super) fn audio(messages: &[RunnerMessage]) -> Vec<MusicalEvent> {
    messages
        .iter()
        .filter_map(|message| match message {
            RunnerMessage::MusicalEvents { events } => Some(events.clone()),
            _ => None,
        })
        .flatten()
        .collect()
}

pub(super) fn midi(messages: &[RunnerMessage]) -> Vec<MusicalEvent> {
    messages
        .iter()
        .filter_map(|message| match message {
            RunnerMessage::MidiEvents { events } => Some(events.clone()),
            _ => None,
        })
        .flatten()
        .filter(|event| !matches!(event, MusicalEvent::Cc { .. }))
        .collect()
}

pub(super) fn note_on(
    channel: u8,
    note: u8,
    velocity: u8,
    duration_ms: Option<u32>,
) -> MusicalEvent {
    MusicalEvent::NoteOn {
        channel,
        note,
        velocity,
        duration_ms,
    }
}

pub(super) fn note_off(channel: u8, note: u8) -> MusicalEvent {
    MusicalEvent::NoteOff { channel, note }
}

/// Holds C4 on I1 (synth) and D4 on I3 (MIDI Inst. on channel 6).
fn hold_two_notes(runner: &mut NativeRunner) {
    send_note(runner, [0x90, 60, 100]);
    send_note(runner, [0x92, 62, 90]);
}

fn assert_both_released(messages: &[RunnerMessage]) {
    assert!(audio(messages).contains(&note_off(0, 60)), "{messages:?}");
    assert!(
        midi(messages).contains(&note_off(MIDI_OUT_CHANNEL, 62)),
        "{messages:?}"
    );
}

fn assert_key_releases_stay_silent(runner: &mut NativeRunner) {
    let messages = send_note(runner, [0x80, 60, 0]);
    assert!(audio(&messages).is_empty());
    let messages = send_note(runner, [0x82, 62, 0]);
    assert!(midi(&messages).is_empty());
}

#[test]
fn channel_one_plays_slot_one_live_with_its_velocity_and_channel_nine_is_ignored() {
    let mut runner = keyboard_runner("keys");

    let messages = send_note(&mut runner, [0x90, 60, 77]);
    assert_eq!(audio(&messages), vec![note_on(0, 60, 77, None)]);
    let messages = send_note(&mut runner, [0x80, 60, 40]);
    assert_eq!(audio(&messages), vec![note_off(0, 60)]);
    let messages = send_note(&mut runner, [0x90, 61, 0]);
    assert!(
        audio(&messages).is_empty(),
        "velocity 0 without a held note"
    );

    let messages = send_note(&mut runner, [0x91, 64, 20]);
    assert_eq!(
        audio(&messages),
        vec![note_on(1, 64, 20, None)],
        "a one-shot slot plays while the key is held"
    );
    let messages = send_note(&mut runner, [0x81, 64, 0]);
    assert_eq!(audio(&messages), vec![note_off(1, 64)]);

    for status in [0x98, 0x9F] {
        let messages = send_note(&mut runner, [status, 60, 100]);
        assert!(audio(&messages).is_empty() && midi(&messages).is_empty());
    }
}

#[test]
fn a_midi_instrument_slot_forwards_the_note_to_its_own_channel() {
    let mut runner = keyboard_runner("keys");

    let messages = send_note(&mut runner, [0x92, 62, 90]);
    assert!(audio(&messages).is_empty());
    assert_eq!(
        midi(&messages),
        vec![note_on(MIDI_OUT_CHANNEL, 62, 90, None)]
    );
    let messages = send_note(&mut runner, [0x82, 62, 0]);
    assert_eq!(midi(&messages), vec![note_off(MIDI_OUT_CHANNEL, 62)]);

    runner.instruments[MIDI_SLOT].midi_enabled = false;
    let messages = send_note(&mut runner, [0x92, 62, 90]);
    assert!(
        midi(&messages).is_empty(),
        "a disabled MIDI Inst. stays quiet"
    );
}

#[test]
fn a_drum_slot_plays_the_voice_with_that_general_midi_note() {
    let mut runner = keyboard_runner("keys");
    runner.instruments[3].kind = InstrumentType::Drum;
    let voice = 1;
    let gm_note = drum_config::drum_voice_midi_note(&runner.instruments[3].drum_config, voice);

    let messages = send_note(&mut runner, [0x93, gm_note, 99]);
    let hits = messages
        .iter()
        .filter_map(|message| match message {
            RunnerMessage::TileHits { hits } => Some(hits.clone()),
            _ => None,
        })
        .flatten()
        .collect::<Vec<_>>();
    assert_eq!(hits.len(), 1);
    assert_eq!(
        (hits[0].instrument_slot, hits[0].voice, hits[0].velocity),
        (3, voice, 99)
    );
}

#[test]
fn stop_releases_held_keyboard_notes_including_forwarded_midi_ones() {
    let mut runner = keyboard_runner("keys");
    runner.transport.transport = RuntimeTransportState::Playing;
    hold_two_notes(&mut runner);

    let messages = runner.send(HostMessage::TransportStop).unwrap();

    assert_both_released(&messages);
    assert_key_releases_stay_silent(&mut runner);
}

#[test]
fn panic_releases_held_keyboard_notes() {
    let mut runner = keyboard_runner("keys");
    hold_two_notes(&mut runner);

    let messages = confirm_key(&mut runner, "midi.panic");

    assert_both_released(&messages);
    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects.contains(&RuntimePlatformEffect::MidiPanic)
    )));
    assert_key_releases_stay_silent(&mut runner);
}

#[test]
fn a_patch_load_releases_held_keyboard_notes() {
    let mut runner = keyboard_runner("keys");
    hold_two_notes(&mut runner);
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["transport"]["bpm"] = json!(97);

    runner.apply_config_payload(payload).unwrap();
    let messages = runner.messages_without_snapshot().unwrap();

    assert_both_released(&messages);
    assert_key_releases_stay_silent(&mut runner);
}

#[test]
fn loading_the_identical_patch_releases_held_keyboard_notes_once() {
    let mut runner = keyboard_runner("keys");
    let load = |runner: &mut NativeRunner| {
        let patch = split_local_system_patch_documents(&runner.config_payload())
            .unwrap()
            .patch;
        runner.apply_patch_payload_preserving_device(patch).unwrap();
    };
    runner.config_revision = 0;
    hold_two_notes(&mut runner);

    load(&mut runner);
    let messages = runner.messages_without_snapshot().unwrap();

    assert_both_released(&messages);
    assert_eq!(midi(&messages).len(), 1, "{:?}", midi(&messages));
    assert_eq!(runner.held_keys.held_count(), 0);
    assert_key_releases_stay_silent(&mut runner);
}

#[test]
fn a_patch_load_ends_each_held_midi_instrument_note_exactly_once() {
    let mut runner = keyboard_runner("keys");
    let held = [60, 64, 67];
    for note in held {
        send_note(&mut runner, [0x92, note, 90]);
    }
    let payload = runner.config_payload();

    runner.apply_config_payload(payload).unwrap();
    let mut messages = runner.messages_without_snapshot().unwrap();
    messages.extend(runner.send(HostMessage::TransportStop).unwrap());

    let offs = midi(&messages);
    for note in held {
        let ends = offs
            .iter()
            .filter(|event| **event == note_off(MIDI_OUT_CHANNEL, note))
            .count();
        assert_eq!(ends, 1, "note {note}: {offs:?}");
    }
    assert_eq!(offs.len(), held.len(), "{offs:?}");
    for note in held {
        let messages = send_note(&mut runner, [0x82, note, 0]);
        assert!(midi(&messages).is_empty(), "note {note} was left held");
    }
}

#[test]
fn notes_in_off_ignores_notes_and_switching_it_off_releases_held_ones() {
    let mut runner = keyboard_runner("keys");
    assert_eq!(runner.menu.value_for_key(NOTES_IN).as_deref(), Some("true"));
    hold_two_notes(&mut runner);

    assert!(runner.menu.set_bool_value_for_key(NOTES_IN, false));
    runner.apply_or_schedule_menu_key(NOTES_IN).unwrap();
    assert!(!runner.midi.notes_in);
    let messages = runner.messages_without_snapshot().unwrap();
    assert_both_released(&messages);

    let messages = send_note(&mut runner, [0x90, 60, 100]);
    assert!(audio(&messages).is_empty());
    let messages = send_note(&mut runner, [0x92, 62, 100]);
    assert!(midi(&messages).is_empty());

    runner.midi.notes_in = true;
    runner.test_set_midi_enabled(false);
    let messages = send_note(&mut runner, [0x90, 60, 100]);
    assert!(
        audio(&messages).is_empty(),
        "MIDI Active off ignores notes too"
    );
}

#[test]
fn notes_leave_cc_learn_waiting_for_its_control_change() {
    let mut runner = keyboard_runner("keys");
    press_key(&mut runner, "midiCc.0.learn");

    send_note(&mut runner, [0x90, 60, 100]);
    assert!(runner.midi_cc_learn.is_some());

    runner
        .send(MidiControlChange::parse(&[0xB1, 71, 5]).unwrap().into())
        .unwrap();
    assert!(runner.midi_cc_learn.is_none());
    assert_eq!(
        (
            runner.midi_cc_mappings[0].cc,
            runner.midi_cc_mappings[0].channel
        ),
        (71, 2)
    );
}

/// The notes a press of each tile plays on a Keys layer with the same Link, lowest index
/// first.
pub(super) fn tile_notes() -> Vec<u8> {
    let mut runner = keyboard_runner("keys");
    (0..GRID_WIDTH * GRID_HEIGHT)
        .map(|index| {
            let (x, y) = (index % GRID_WIDTH, index / GRID_WIDTH);
            let pressed = runner
                .send(HostMessage::DeviceInput {
                    input: json!({ "type": "grid_press", "x": x, "y": y }),
                    request_snapshot: None,
                })
                .unwrap();
            runner
                .send(HostMessage::DeviceInput {
                    input: json!({ "type": "grid_release", "x": x, "y": y }),
                    request_snapshot: None,
                })
                .unwrap();
            audio(&pressed)
                .into_iter()
                .find_map(|event| match event {
                    MusicalEvent::NoteOn { note, .. } => Some(note),
                    _ => None,
                })
                .expect("every tile plays a note")
        })
        .collect()
}

pub(super) fn recorded_events(runner: &NativeRunner, layer: usize) -> Vec<Value> {
    let state = runner.layers[layer].engine.serialized_state().unwrap();
    state["steps"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|step| step.as_array().unwrap().clone())
        .collect()
}

pub(super) fn playing_looper() -> NativeRunner {
    let mut runner = keyboard_runner("looper");
    press_play(&mut runner);
    runner
}

/// Stops, then presses Play, and returns what Play sent.
pub(super) fn press_play(runner: &mut NativeRunner) -> Vec<RunnerMessage> {
    runner.send(HostMessage::TransportStop).unwrap();
    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_s", "pressed": true }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(runner.transport.transport, RuntimeTransportState::Playing);
    messages
}

#[test]
fn an_overdubbing_looper_for_the_slot_records_the_lowest_tile_that_plays_the_note() {
    let notes = tile_notes();
    let tile = GRID_WIDTH * 3 + 5;
    let note = notes[tile];
    let lowest = notes.iter().position(|played| *played == note).unwrap();
    let mut runner = playing_looper();

    let pressed = send_note(&mut runner, [0x90, note, 100]);
    let released = send_note(&mut runner, [0x80, note, 0]);

    assert_eq!(audio(&pressed), vec![note_on(0, note, 100, None)]);
    assert_eq!(audio(&released), vec![note_off(0, note)]);
    assert_eq!(
        runner.layers[0].engine.serialized_state().unwrap()["steps"],
        json!([
            [{ "tile": lowest, "kind": "press" }],
            [{ "tile": lowest, "kind": "release" }],
        ]),
        "right after Play the press lands on step 1 and its release on step 2"
    );
    let replayed = (0..16)
        .flat_map(|_| {
            let messages = runner
                .send(HostMessage::TransportPulseStep {
                    pulses: 12,
                    source: SyncSource::Internal,
                    at_ppqn_pulse: None,
                    request_snapshot: Some(false),
                })
                .unwrap();
            audio(&messages)
        })
        .collect::<Vec<_>>();
    assert!(
        replayed.iter().any(
            |event| matches!(event, MusicalEvent::NoteOn { note: played, .. } if *played == note)
        ),
        "the loop plays the recorded tile on a later pass: {replayed:?}"
    );
}

#[test]
fn a_looper_that_is_not_recording_or_plays_another_slot_records_nothing() {
    let note = tile_notes()[0];

    let mut runner = playing_looper();
    press_key(&mut runner, "layers.0.build.behaviorConfig.toggleMode");
    assert_eq!(looper_mode_and_step(&runner).0, "play");
    send_note(&mut runner, [0x90, note, 100]);
    send_note(&mut runner, [0x80, note, 0]);
    assert!(recorded_events(&runner, 0).is_empty(), "punched out");

    let mut runner = playing_looper();
    let messages = send_note(&mut runner, [0x91, note, 100]);
    assert_eq!(audio(&messages).len(), 1, "I2 still sounds");
    assert!(recorded_events(&runner, 0).is_empty(), "the layer plays I1");
}

#[test]
fn a_note_no_tile_plays_still_sounds_but_is_not_recorded() {
    let notes = tile_notes();
    let unmapped = (0..=127).find(|note| !notes.contains(note)).unwrap();
    let mut runner = playing_looper();

    let messages = send_note(&mut runner, [0x90, unmapped, 100]);

    assert_eq!(audio(&messages), vec![note_on(0, unmapped, 100, None)]);
    assert!(recorded_events(&runner, 0).is_empty());
}

#[test]
fn notes_in_row_shows_its_help_and_survives_a_system_save() {
    let mut runner = board_runner(false);
    assert!(runner.menu.focus_item_key(NOTES_IN));
    assert_eq!(runner.menu.current_label(), Some("Notes In"));
    let help = resolve_native_help(&runner.menu.current_help_target().unwrap()).unwrap();
    assert_eq!(help.title, "Notes In");

    let saved = save_setting_payload(&mut runner, NOTES_IN, -1, "notes-in-save");
    assert_eq!(saved["runtimeConfig"]["midi"]["notesIn"], false);

    let reloaded = reload_system_payload(saved);
    assert!(!reloaded.midi.notes_in);
    assert_eq!(
        reloaded.menu.value_for_key(NOTES_IN).as_deref(),
        Some("false")
    );
}

#[test]
fn a_drum_slot_records_its_lowest_tile_stamped_with_the_voice() {
    let mut runner = playing_looper();
    runner.instruments[0].kind = InstrumentType::Drum;
    runner.instruments[0].drum_config["assignments"] = json!([
        { "x": 4, "y": 2, "voice": 1 },
        { "x": 6, "y": 0, "voice": 1 },
        { "x": 0, "y": 0, "voice": 0 },
    ]);
    let gm_note = drum_config::drum_voice_midi_note(&runner.instruments[0].drum_config, 1);

    send_note(&mut runner, [0x90, gm_note, 100]);

    assert_eq!(
        runner.layers[0].engine.serialized_state().unwrap()["steps"],
        json!([
            [{ "tile": 6, "kind": "press" }],
            [{ "tile": 6, "kind": "release" }],
        ]),
        "a drum hit records its press and release at once"
    );
}
