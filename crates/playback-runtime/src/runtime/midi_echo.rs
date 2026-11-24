//! Note-ons this runtime sent to MIDI out, so a device that echoes its input cannot loop them
//! back in through MIDI In as keyboard notes.

use super::MidiNote;
use std::time::{Duration, Instant};

/// For this long after this runtime plays a note-on of its own (a layer's note, a Keys press or a
/// Send MIDI copy) on MIDI out, an incoming note-on on that channel and key is taken as its echo,
/// whatever its velocity, and dropped. Device echoes and DAW thru arrive well within it.
const PLAYED_WINDOW: Duration = Duration::from_millis(250);

/// A tap shorter than the thru latency comes back after its own key-up, so the passed-on note
/// can loop for good: the same keyboard note-on passed on this many times in a row, at one
/// steady interval under `LOOP_MAX_INTERVAL` with no other key coming in, is that loop, and that
/// copy is not sent.
const LOOP_REPEATS: u8 = 8;

/// A loop repeats at the thru's round trip, a few ms to a few tens of ms; a clocked source or a
/// player repeats a key no faster than about 32nds at 240 BPM (62 ms). Only identical repeats
/// closer together than this count toward the loop, so a steady drum machine is never cut.
const LOOP_MAX_INTERVAL: Duration = Duration::from_millis(40);

/// How far apart two intervals may be and still count as one steady interval.
const LOOP_INTERVAL_SLACK: Duration = Duration::from_millis(2);

/// Keyboard notes passed on to MIDI out are played by hand, so 64 hold many busy chords.
const RELAYED_NOTE_SLOTS: usize = 64;

const KEYS: usize = 16 * 128;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct SentMidiNotes {
    /// The wire instant of the latest note-on this runtime played itself, per channel and key.
    played: Box<[Option<Instant>]>,
    /// Each keyboard note passed on to MIDI out, with the instant it goes on the wire, until the
    /// key-up for it comes in.
    relayed: [Option<(MidiNote, Instant)>; RELAYED_NOTE_SLOTS],
    next: usize,
    /// The latest keyboard note-on passed on to MIDI out, and how steadily it repeats.
    repeat: Option<Repeat>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Repeat {
    note: MidiNote,
    at: Instant,
    interval: Option<Duration>,
    count: u8,
}

impl Default for SentMidiNotes {
    fn default() -> Self {
        Self {
            played: vec![None; KEYS].into_boxed_slice(),
            relayed: [None; RELAYED_NOTE_SLOTS],
            next: 0,
            repeat: None,
        }
    }
}

fn key(note: MidiNote) -> usize {
    usize::from(note.channel & 0x0F) * 128 + usize::from(note.note & 0x7F)
}

impl SentMidiNotes {
    /// Keeps `bytes` if it is a note-on, sent at `at`; `relayed` when it passes on a MIDI In
    /// note. Note-offs are never echoes: one that gets through releases only a key still held and
    /// cannot loop, while dropping one would hang a note the player really let go of.
    pub(super) fn remember(&mut self, bytes: &[u8], at: Instant, relayed: bool) {
        let Some(note) = MidiNote::parse(bytes).filter(|note| note.velocity > 0) else {
            return;
        };
        if relayed {
            self.relayed[self.next] = Some((note, at));
            self.next = (self.next + 1) % RELAYED_NOTE_SLOTS;
        } else {
            let played = &mut self.played[key(note)];
            *played = Some(played.map_or(at, |previous| previous.max(at)));
        }
    }

    /// True when `note` is the echo of a note-on sent to MIDI out: any note-on on a channel and
    /// key this runtime played itself within the played window (one scheduled for later counts
    /// too), or one identical to a passed-on keyboard note whose key is still down, which it
    /// forgets: a real repeat of a key needs its key-up first. An incoming note-off forgets the
    /// passed-on note-ons for that key, so the player's next press of it plays; it leaves the
    /// runtime's own notes alone, as a device echoes a retrigger's note-off just before its
    /// note-on. Another key coming in ends a run of passed-on repeats.
    pub(super) fn take_echo(&mut self, note: MidiNote, now: Instant) -> bool {
        let key_of = |note: MidiNote| (note.channel, note.note);
        if self
            .repeat
            .is_some_and(|repeat| key_of(repeat.note) != key_of(note))
        {
            self.repeat = None;
        }
        if note.velocity == 0 {
            for slot in &mut self.relayed {
                if slot
                    .is_some_and(|(sent, _)| (sent.channel, sent.note) == (note.channel, note.note))
                {
                    *slot = None;
                }
            }
            return false;
        }
        if self.played[key(note)]
            .is_some_and(|at| at.checked_add(PLAYED_WINDOW).is_none_or(|end| now <= end))
        {
            return true;
        }
        let Some(slot) = self
            .relayed
            .iter_mut()
            .find(|slot| slot.is_some_and(|(sent, at)| sent == note && at <= now))
        else {
            return false;
        };
        *slot = None;
        true
    }

    /// Counts the keyboard note-on `bytes` passed on at `at` as a repeat of the last one, and is
    /// true once it is the loop, which is then not sent.
    pub(super) fn relay_loops(&mut self, bytes: &[u8], at: Instant) -> bool {
        let Some(note) = MidiNote::parse(bytes).filter(|note| note.velocity > 0) else {
            return false;
        };
        let now = at;
        let fast = |last: &Repeat| now.saturating_duration_since(last.at) < LOOP_MAX_INTERVAL;
        let repeat = match self
            .repeat
            .filter(|repeat| repeat.note == note && fast(repeat))
        {
            Some(last) => {
                let gap = now.saturating_duration_since(last.at);
                let steady = last
                    .interval
                    .is_some_and(|interval| gap.abs_diff(interval) <= LOOP_INTERVAL_SLACK);
                Repeat {
                    note,
                    at: now,
                    interval: Some(gap),
                    count: if steady || last.interval.is_none() {
                        last.count.saturating_add(1)
                    } else {
                        2
                    },
                }
            }
            None => Repeat {
                note,
                at: now,
                interval: None,
                count: 1,
            },
        };
        if repeat.count >= LOOP_REPEATS {
            self.repeat = None;
            return true;
        }
        self.repeat = Some(repeat);
        false
    }
}
