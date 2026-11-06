//! MIDI through the headless stack: internal notes copied by Send MIDI `all inst.` (with I4
//! made a Drum), their end on a Mix mute and on the switch back to `MIDI inst.`, a MIDI Drum's
//! stamped notes, and a MIDI CC In mapping swept from an input port, kept across Save Patch and
//! a restart.
use crate::device_menu::{confirm, open, press_back, select, set_number, set_until, tap};
use crate::support::{reload_differences, session_root};
use octessera_desktop_lib::headless::{HeadlessSession, PlayedMidi};
use playback_runtime::scenario::{button, grid, press_main, BAR};
use serde_json::Value;

/// Sampler hits go out as notes 36..43 for its sample slots 1..8.
const SAMPLER_SLOT_NOTES: std::ops::RangeInclusive<u8> = 36..=43;
/// GM notes of the Drum voices' Sounds.
const DRUM_GM_NOTES: [u8; 8] = [36, 37, 38, 39, 42, 45, 46, 50];
const DRUM_CHANNEL: u8 = 10;
/// Tiles the default L3 loop presses, as world-space `(x, y)`.
const L3_TILES: [(usize, usize); 2] = [(1, 6), (2, 5)];
const STAMPED_NOTES: [i32; 2] = [36, 38];

fn start(root: &std::path::Path) -> HeadlessSession {
    HeadlessSession::start(root.join("store"), root.join("media"))
        .unwrap_or_else(|error| panic!("midi: session failed to start: {error}"))
}

fn config(session: &HeadlessSession) -> Value {
    session.config_payload()["runtimeConfig"].clone()
}

/// MIDI Active on, then the test port picked under MIDI Host for MIDI Out and MIDI In; both save
/// System without Save System. Opening a port list scans this computer's ports; the test port
/// replaces that answer once it is in.
fn enable_midi(session: &mut HeadlessSession) {
    open(session, &["System", "MIDI"]);
    set_until(session, "MIDI Active", |session| {
        config(session)["midi"]["enabled"] == true
    });
    for direction in ["MIDI Out", "MIDI In"] {
        open(session, &["System", "MIDI", "MIDI Host", direction]);
        session.settle().unwrap();
        session.list_test_midi_ports().unwrap();
        select(session, "e2e port");
        tap(session, press_main());
    }
    let midi = config(session)["midi"].clone();
    assert_eq!(
        (&midi["outId"], &midi["inId"]),
        (&Value::from("999"), &Value::from("999"))
    );
    session.settle().unwrap();
}

fn send_midi(session: &mut HeadlessSession, value: &str) {
    open(session, &["System", "MIDI"]);
    set_until(session, "Send", |session| {
        config(session)["midi"]["sendInstruments"] == value
    });
}

fn play(session: &mut HeadlessSession) {
    if !session.is_playing() {
        tap(session, button("button_s", true));
        tap(session, button("button_s", false));
    }
}

fn render_pulses(session: &mut HeadlessSession, pulses: u64) {
    let target = session.transport_pulse() + pulses;
    while session.transport_pulse() < target {
        session.render(64).unwrap();
    }
}

/// Fn + right column row 0 opens Mix; row 0 of a column then toggles that instrument's mute.
fn toggle_mix_mute(session: &mut HeadlessSession, column: usize) {
    tap(session, button("button_fn", true));
    tap(session, grid(7, 0, true));
    tap(session, grid(7, 0, false));
    tap(session, button("button_fn", false));
    tap(session, grid(column, 0, true));
    tap(session, grid(column, 0, false));
}

/// A note-on (velocity above 0) or note-off as `(channel 1..16, note, velocity)`.
fn note_message(message: &PlayedMidi) -> Option<(bool, u8, u8, u8)> {
    let [status, note, velocity] = message.bytes[..] else {
        return None;
    };
    let channel = (status & 0x0F) + 1;
    match status & 0xF0 {
        0x90 if velocity > 0 => Some((true, channel, note, velocity)),
        0x90 | 0x80 => Some((false, channel, note, velocity)),
        _ => None,
    }
}

/// Copies still held after the first `upto` messages: `(channel, note)` per sounding note-on.
fn held(log: &[PlayedMidi], upto: usize) -> Vec<(u8, u8)> {
    let mut held: Vec<(u8, u8)> = Vec::new();
    for message in &log[..upto] {
        match note_message(message) {
            Some((true, channel, note, _)) => held.push((channel, note)),
            Some((false, channel, note, _)) => {
                if let Some(index) = held.iter().position(|key| *key == (channel, note)) {
                    held.remove(index);
                }
            }
            None => {}
        }
    }
    held
}

fn note_ons(log: &[PlayedMidi], channel: u8) -> Vec<&PlayedMidi> {
    log.iter()
        .filter(|message| matches!(note_message(message), Some((true, c, ..)) if c == channel))
        .collect()
}

fn note_offs_of(log: &[PlayedMidi], key: (u8, u8)) -> usize {
    log.iter()
        .filter(|message| matches!(note_message(message), Some((false, c, n, _)) if (c, n) == key))
        .count()
}

/// Frames from each note-on on `channel` to its note-off.
fn durations(log: &[PlayedMidi], channel: u8) -> Vec<(u8, usize)> {
    let mut open: Vec<(u8, usize)> = Vec::new();
    let mut lengths = Vec::new();
    for message in log {
        match note_message(message) {
            Some((true, c, note, _)) if c == channel => open.push((note, message.frame)),
            Some((false, c, note, _)) if c == channel => {
                if let Some(index) = open.iter().position(|(held, _)| *held == note) {
                    let (_, on) = open.remove(index);
                    lengths.push((note, message.frame - on));
                }
            }
            _ => {}
        }
    }
    lengths
}

fn assert_fixed_length(session: &HeadlessSession, log: &[PlayedMidi], channel: u8, what: &str) {
    let expected = session.sample_rate() as usize / 10;
    let lengths = durations(log, channel);
    assert!(!lengths.is_empty(), "{what}: no notes on channel {channel}");
    for (note, frames) in lengths {
        assert!(
            frames.abs_diff(expected) <= 128,
            "{what}: note {note} lasted {frames} frames, not 100 ms"
        );
    }
}

/// Copies held at `upto` each end with exactly one note-off afterwards, and none of `channels`
/// starts a new note once those note-offs went out.
fn assert_copies_end_once(log: &[PlayedMidi], upto: usize, channels: &[u8], what: &str) {
    let ended = held(log, upto);
    let ended: Vec<(u8, u8)> = ended
        .into_iter()
        .filter(|(channel, _)| channels.contains(channel))
        .collect();
    assert!(!ended.is_empty(), "{what}: no copies were held");
    let after = &log[upto..];
    for key in &ended {
        assert_eq!(
            note_offs_of(after, *key),
            ended.iter().filter(|held| *held == key).count(),
            "{what}: channel {} note {} should end once",
            key.0,
            key.1
        );
    }
    for channel in channels {
        assert!(
            note_ons(after, *channel).is_empty(),
            "{what}: channel {channel} still sends notes"
        );
    }
}

/// Assign from the slot's `group`, then Shift + a bottom-row tile: stamps the row L4 plays.
fn stamp_bottom_row(session: &mut HeadlessSession, group: &str) {
    select(session, group);
    tap(session, press_main());
    select(session, "Assign");
    tap(session, press_main());
    tap(session, button("button_shift", true));
    tap(session, grid(1, 0, true));
    tap(session, grid(1, 0, false));
    tap(session, button("button_shift", false));
    press_back(session);
}

#[test]
fn send_midi_all_copies_internal_notes_until_a_mute_or_the_switch_back() {
    let root = session_root("midi-copies");
    let _ = std::fs::remove_dir_all(&root);
    let mut session = start(&root);
    enable_midi(&mut session);
    send_midi(&mut session, "all");
    open(&mut session, &["Shape", "Instruments", "I4"]);
    set_until(&mut session, "Type", |session| {
        config(session)["instruments"][3]["type"] == "drum"
    });
    stamp_bottom_row(&mut session, "Drum");
    play(&mut session);
    render_pulses(&mut session, 4 * BAR);

    let log = session.midi_out().to_vec();
    assert!(
        !note_ons(&log, 1).is_empty(),
        "the I1 synth sends copies on channel 1"
    );
    let open_synth = held(&log, log.len())
        .into_iter()
        .filter(|(channel, _)| *channel == 1)
        .count();
    assert_eq!(
        durations(&log, 1).len() + open_synth,
        note_ons(&log, 1).len(),
        "every synth copy that ended has its note-off"
    );
    let sampler = note_ons(&log, 2);
    assert!(
        !sampler.is_empty()
            && sampler
                .iter()
                .all(|message| SAMPLER_SLOT_NOTES.contains(&message.bytes[1])),
        "I2 Sampler hits send the notes of their sample slots on channel 2"
    );
    assert_fixed_length(&session, &log, 2, "Sampler copies");
    let drum = note_ons(&log, 4);
    assert!(
        !drum.is_empty()
            && drum
                .iter()
                .all(|message| DRUM_GM_NOTES.contains(&message.bytes[1])),
        "I4 Drum hits send their voices' GM notes on channel 4"
    );
    assert_fixed_length(&session, &log, 4, "Drum copies");

    let before_mute = session.midi_out().len();
    toggle_mix_mute(&mut session, 0);
    render_pulses(&mut session, 2 * BAR);
    assert_copies_end_once(session.midi_out(), before_mute, &[1], "Mix mute of I1");

    let before_switch = session.midi_out().len();
    send_midi(&mut session, "midi");
    render_pulses(&mut session, 2 * BAR);
    assert_copies_end_once(
        session.midi_out(),
        before_switch,
        &[2, 3, 4],
        "Send MIDI back to MIDI inst.",
    );
}

/// I3 (the L3 loop's Plucked) as an enabled MIDI Drum with one stamp per looped tile.
fn stamp_midi_drum(session: &mut HeadlessSession) {
    open(session, &["Shape", "Instruments", "I3"]);
    set_until(session, "Type", |session| {
        config(session)["instruments"][2]["type"] == "midi_drum"
    });
    select(session, "MIDI Drum");
    tap(session, press_main());
    set_until(session, "Enabled", |session| {
        config(session)["instruments"][2]["midiDrum"]["enabled"] == true
    });
    let drum_note = |session: &HeadlessSession| {
        config(session)["instruments"][2]["midiDrum"]["tileNote"]
            .as_i64()
            .unwrap() as i32
    };
    for ((x, y), note) in L3_TILES.into_iter().zip(STAMPED_NOTES) {
        set_number(session, "Note", drum_note, note);
        select(session, "Assign");
        tap(session, press_main());
        tap(session, grid(x, y, true));
        tap(session, grid(x, y, false));
        press_back(session);
    }
}

/// Link > MIDI CC In > CC1 mapped to I1's filter cutoff through the shared picker.
fn map_cc1_to_cutoff(session: &mut HeadlessSession) {
    open(session, &["Link", "MIDI CC In", "CC1", "Target"]);
    for label in ["Shape", "Instruments", "I1", "Synth", "Filter", "Cutoff"] {
        select(session, label);
        tap(session, press_main());
    }
}

/// `sqrt(E[d²] / E[x²])` of the mono mix between two frames: rises with the spectral centroid.
fn brightness(session: &HeadlessSession, from: usize, to: usize) -> f64 {
    let audio = session.audio();
    let mono = |frame: usize| f64::from(audio[frame * 2] + audio[frame * 2 + 1]) * 0.5;
    let (mut level, mut slope) = (0.0, 0.0);
    for frame in from + 1..to {
        level += mono(frame).powi(2);
        slope += (mono(frame) - mono(frame - 1)).powi(2);
    }
    (slope / level.max(1e-18)).sqrt()
}

fn sweep_brightness(session: &mut HeadlessSession, value: u8) -> f64 {
    session.midi_in(&[0xB0, 1, value]).unwrap();
    render_pulses(session, BAR);
    let from = session.audio().len() / 2;
    render_pulses(session, 2 * BAR);
    brightness(session, from, session.audio().len() / 2)
}

fn assert_drum_stamps_play(session: &HeadlessSession, from: usize, what: &str) {
    let log = &session.midi_out()[from..];
    let mut notes: Vec<u8> = note_ons(log, DRUM_CHANNEL)
        .iter()
        .map(|message| message.bytes[1])
        .collect();
    notes.sort_unstable();
    notes.dedup();
    assert_eq!(
        notes,
        STAMPED_NOTES.map(|note| note as u8).to_vec(),
        "{what}: the stamped tiles send their notes on channel {DRUM_CHANNEL}"
    );
    assert!(
        note_ons(log, DRUM_CHANNEL)
            .iter()
            .all(|message| message.bytes[2] == 100),
        "{what}: stamps keep Tile Vel 100"
    );
    assert_fixed_length(session, log, DRUM_CHANNEL, what);
}

#[test]
fn midi_drum_stamps_and_a_cc_in_mapping_play_and_survive_a_restart() {
    let root = session_root("midi-drum-cc");
    let _ = std::fs::remove_dir_all(&root);
    let mut first = start(&root);
    enable_midi(&mut first);
    stamp_midi_drum(&mut first);
    map_cc1_to_cutoff(&mut first);
    toggle_mix_mute(&mut first, 1);
    tap(&mut first, grid(3, 0, true));
    tap(&mut first, grid(3, 0, false));
    play(&mut first);
    let take = first.midi_out().len();
    let dark = sweep_brightness(&mut first, 0);
    let bright = sweep_brightness(&mut first, 127);
    assert!(
        bright > dark * 1.5,
        "CC1 0 -> 127 moves I1's cutoff: brightness {dark:.4} -> {bright:.4}"
    );
    assert_drum_stamps_play(&first, take, "first take");

    tap(&mut first, button("button_s", true));
    tap(&mut first, button("button_s", false));
    open(&mut first, &["System", "Saves", "Default", "Save Patch"]);
    confirm(&mut first);
    first.settle().unwrap();
    let saved = first.config_payload();
    drop(first);

    let mut second = start(&root);
    let changed = reload_differences(&saved, &second.config_payload());
    assert!(changed.is_empty(), "reloaded differently: {changed:#?}");
    let reloaded = config(&second);
    assert_eq!(
        (&reloaded["midi"]["outId"], &reloaded["midi"]["inId"]),
        (&Value::from("999"), &Value::from("999"))
    );
    assert_eq!(
        reloaded["midiCcMappings"][0]["target"]["key"],
        "instruments.0.synth.filter.cutoffHz"
    );
    assert_eq!(
        reloaded["instruments"][2]["midiDrum"]["assignments"]
            .as_array()
            .map(Vec::len),
        Some(2)
    );
    play(&mut second);
    render_pulses(&mut second, 4 * BAR);
    assert_drum_stamps_play(&second, 0, "after the restart");
}
