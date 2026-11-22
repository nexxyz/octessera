//! Note-ons this runtime sent to MIDI out, so a device that echoes its input cannot loop them
//! back in through MIDI In as keyboard notes.

use super::MidiNote;
use std::time::{Duration, Instant};

/// For about this long after this runtime plays a note-on of its own (a layer's note, a Keys
/// press or a Send MIDI copy) on MIDI out, an incoming note-on on that channel and key is taken as
/// its echo, whatever its velocity, and dropped. No device echo or DAW thru is slower.
const PLAYED_WINDOW: Duration = Duration::from_secs(1);

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
}

impl Default for SentMidiNotes {
    fn default() -> Self {
        Self {
            played: vec![None; KEYS].into_boxed_slice(),
            relayed: [None; RELAYED_NOTE_SLOTS],
            next: 0,
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
    /// note-on.
    pub(super) fn take_echo(&mut self, note: MidiNote, now: Instant) -> bool {
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
}
