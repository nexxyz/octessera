//! A note-off from one layer for a pitch another layer or Notes In holds on that instrument ends
//! it there, as that holder sounds it.

use super::{HeldNote, HeldNotes};
use crate::native_runner::modulation::RoutedMusicalEvents;
use crate::native_runner::NativeRunner;

impl NativeRunner {
    /// Ends `key` wherever another layer or Notes In holds it, as that holder sounds it.
    pub(super) fn end_held_note_elsewhere(&mut self, layer_index: usize, key: HeldNote) {
        let mut released = RoutedMusicalEvents::default();
        for other in (0..self.held_notes.len()).filter(|other| *other != layer_index) {
            let mut ended = RoutedMusicalEvents::default();
            if self.held_notes[other].end_held(key, &mut ended) {
                self.forget_layer_owned_route_note_offs(other, &ended);
                released.extend(ended);
            }
        }
        self.release_keyboard_note(key.channel, key.note, &mut released);
        self.ended_elsewhere.extend(released);
    }
}

impl HeldNotes {
    /// Another layer's note-off ended `key`: forgets who holds it here, so its tiles' Offs end
    /// nothing and a held Arp no longer replays it, and ends what it sounds as into `out`.
    pub(super) fn end_held(&mut self, key: HeldNote, out: &mut RoutedMusicalEvents) -> bool {
        let Some(entry) = self
            .notes
            .get_mut(&key)
            .filter(|entry| !entry.holders.is_empty())
        else {
            return false;
        };
        let holders = std::mem::take(&mut entry.holders);
        self.drained_tiles.extend(holders.tiles);
        entry.replay = None;
        for sounding in entry.sounding.drain(..) {
            sounding.release_into(out);
        }
        self.remove_if_idle(key);
        true
    }
}
