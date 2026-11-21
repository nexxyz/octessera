//! Note-ons this runtime sent to MIDI out, so a device that echoes its input cannot loop them
//! back in through MIDI In as keyboard notes.

use super::MidiNote;
use std::time::{Duration, Instant};

/// How long after a note-on leaves MIDI out an identical incoming note-on counts as its echo. A
/// match needs the same velocity too, so a slow DAW thru still matches and a player rarely does.
const ECHO_WINDOW: Duration = Duration::from_millis(150);

/// Every note-on sent lives here from being scheduled until the echo window after it goes out.
/// Eight slots with four-note chords are 32 note-ons per step, so 64 entries hold two full busy
/// steps. Faster steps can overwrite a live entry; that note's echo then plays once, and its own
/// forward is remembered, so it cannot loop.
const SENT_NOTE_SLOTS: usize = 64;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct SentMidiNotes {
    /// Each note-on with the instant the MIDI sender puts it on the wire.
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
    /// Keeps `bytes` if it is a note-on, sent at `at`. Note-offs are never echoes: one that gets
    /// through releases only a key still held and cannot loop, while dropping one would hang a
    /// note the player really let go of.
    pub(super) fn remember(&mut self, bytes: &[u8], at: Instant) {
        let Some(note) = MidiNote::parse(bytes).filter(|note| note.velocity > 0) else {
            return;
        };
        self.notes[self.next] = Some((note, at));
        self.next = (self.next + 1) % SENT_NOTE_SLOTS;
    }

    /// True, and forgets it, when `note` repeats a note-on sent at most the echo window before
    /// `now`. A note scheduled for later has not gone out yet, so nothing can echo it. An incoming
    /// note-off forgets the note-ons sent for that key: a device echoes a note-on before its
    /// note-off, so whatever is still remembered then was never echoed, and the player's next
    /// press of that key must not be taken for its echo.
    pub(super) fn take_echo(&mut self, note: MidiNote, now: Instant) -> bool {
        if note.velocity == 0 {
            for slot in &mut self.notes {
                if slot
                    .is_some_and(|(sent, _)| (sent.channel, sent.note) == (note.channel, note.note))
                {
                    *slot = None;
                }
            }
            return false;
        }
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
