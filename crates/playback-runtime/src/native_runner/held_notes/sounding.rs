//! What each held note sounds as on its instrument: the transposed pitch, the MIDI channel it
//! went out on, or the Sampler tile it plays.

use super::{HeldNote, HeldNotes};
use crate::native_runner::modulation::sampler::TransposedHeldNote;
use crate::native_runner::modulation::RoutedMusicalEvents;

impl HeldNotes {
    /// Remembers what the held note-on of `channel` and `note` sounds as.
    pub(in crate::native_runner) fn record_sounding(
        &mut self,
        channel: u8,
        note: u8,
        sounding: TransposedHeldNote,
    ) {
        self.notes
            .entry(HeldNote { channel, note })
            .or_default()
            .sounding
            .push(sounding);
    }

    /// Takes what `channel` and `note` sounds as on Sampler `tile`.
    pub(in crate::native_runner) fn take_sounding_tile(
        &mut self,
        channel: u8,
        note: u8,
        tile: u8,
    ) -> Option<TransposedHeldNote> {
        let key = HeldNote { channel, note };
        let sounding = &mut self.notes.get_mut(&key)?.sounding;
        let index = sounding.iter().position(|held| {
            held.sampler_tile
                .is_some_and(|(_, held_tile)| held_tile == tile)
        })?;
        let taken = sounding.remove(index);
        self.remove_if_idle(key);
        Some(taken)
    }

    /// Takes the latest sound of `channel` and `note`.
    pub(in crate::native_runner) fn take_sounding(
        &mut self,
        channel: u8,
        note: u8,
    ) -> Option<TransposedHeldNote> {
        let key = HeldNote { channel, note };
        let taken = self.notes.get_mut(&key)?.sounding.pop();
        self.remove_if_idle(key);
        taken
    }

    /// Ends the sound of every held note, or of those on `channel`, into `out`; who holds them
    /// stays.
    pub(in crate::native_runner) fn release_sounding(
        &mut self,
        channel: Option<u8>,
        out: &mut RoutedMusicalEvents,
    ) {
        self.notes.retain(|key, entry| {
            if channel.is_none_or(|channel| channel == key.channel) {
                for sounding in entry.sounding.drain(..) {
                    sounding.release_into(out);
                }
            }
            !entry.holders.is_empty() || !entry.sounding.is_empty()
        });
    }

    /// Forgets every sound without ending it; who holds the notes stays.
    pub(in crate::native_runner) fn clear_sounding(&mut self) {
        self.notes.retain(|_, entry| {
            entry.sounding.clear();
            !entry.holders.is_empty()
        });
    }

    /// True while Sampler slot `slot` sounds `tile` on `voice` for a held note.
    pub(in crate::native_runner) fn sounds_tile(&self, slot: u8, voice: u8, tile: u8) -> bool {
        self.notes.values().any(|entry| {
            entry
                .sounding
                .iter()
                .any(|held| held.routed_channel == slot && held.sampler_tile == Some((voice, tile)))
        })
    }

    #[cfg(test)]
    pub(in crate::native_runner) fn sounding_keys(&self) -> Vec<(u8, u8)> {
        self.notes
            .iter()
            .filter(|(_, entry)| !entry.sounding.is_empty())
            .map(|(key, _)| (key.channel, key.note))
            .collect()
    }
}
