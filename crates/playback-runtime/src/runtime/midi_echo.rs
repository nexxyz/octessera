//! Notes this runtime sent to MIDI out, so a device that echoes its input cannot loop them back
//! in through MIDI In as keyboard notes.

use super::MidiNote;
use std::time::{Duration, Instant};

/// How long after a note leaves MIDI out an identical incoming note counts as its echo.
const ECHO_WINDOW: Duration = Duration::from_millis(30);

/// Every note-on and note-off sent lives here from being scheduled until the echo window after
/// it goes out. Eight slots with four-note chords, note-ons and note-offs, are 64 events per step; a
/// sixteenth at 120 BPM is 125 ms, so 64 entries hold a full busy step.
const SENT_NOTE_SLOTS: usize = 64;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct SentMidiNotes {
    /// Each note with the instant the MIDI sender puts it on the wire; velocity 0 for note-offs.
    notes: [Option<(MidiNote, Instant)>; SENT_NOTE_SLOTS],
    next: usize,
}

impl Default for SentMidiNotes {
    fn default() -> Self {
        Self {
            notes: [None; SENT_NOTE_SLOTS],
            next: 0,
        }
    }
}

impl SentMidiNotes {
    /// Keeps `bytes` if it is a note, sent at `at`.
    pub(super) fn remember(&mut self, bytes: &[u8], at: Instant) {
        let Some(note) = MidiNote::parse(bytes) else {
            return;
        };
        self.notes[self.next] = Some((note, at));
        self.next = (self.next + 1) % SENT_NOTE_SLOTS;
    }

    /// True, and forgets it, when `note` repeats one sent at most the echo window before `now`.
    /// A note scheduled for later has not gone out yet, so nothing can echo it.
    pub(super) fn take_echo(&mut self, note: MidiNote, now: Instant) -> bool {
        let Some(slot) = self.notes.iter_mut().find(|slot| {
            slot.is_some_and(|(sent, at)| {
                sent == note && at <= now && now.duration_since(at) <= ECHO_WINDOW
            })
        }) else {
            return false;
        };
        *slot = None;
        true
    }
}
