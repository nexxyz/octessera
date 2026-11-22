//! What a held Arp holds: the layer's held notes, which it replays through the current Play
//! Transpose, and a One-Shot note for as long as the tile that played it stays lit.

use super::modulation::sampler::replayed_note_on;
use super::modulation::RoutedMusicalEvents;
use super::{LinkArpHeldNote, NativeInstrumentSlot, NativeRunner, GRID_WIDTH};
use platform_core::{MusicalEvent, TileTriggerIntent};

impl NativeRunner {
    /// Forgets the One-Shot notes of tiles the layer no longer shows lit, whatever its Off Trig.
    pub(super) fn release_dark_link_arp_tiles(&mut self, layer_index: usize, lit_tiles: &[bool]) {
        if let Some(held) = self.link_arp.held_notes.get_mut(layer_index) {
            held.retain(|note| lit_tiles.get(note.tile).copied() == Some(true));
        }
    }

    /// Adds the One-Shot notes `lit` by a batch's tiles.
    pub(super) fn update_link_arp_lit_notes(
        &mut self,
        layer_index: usize,
        lit: Vec<LinkArpHeldNote>,
    ) {
        let Some(held) = self.link_arp.held_notes.get_mut(layer_index) else {
            return;
        };
        for note in lit {
            held.retain(|held| {
                (held.tile, held.audio, held.channel) != (note.tile, note.audio, note.channel)
            });
            held.push(note);
        }
    }

    /// The held set as note-ons, one per pitch even when two lit tiles share it: the layer's held
    /// notes through the current Play Transpose, then its lit One-Shot notes.
    pub(super) fn held_notes_as_routed(
        &self,
        layer_index: usize,
        instruments: &[NativeInstrumentSlot],
        transpose_offset: i8,
    ) -> RoutedMusicalEvents {
        let replays = self
            .held_notes
            .get(layer_index)
            .map(|held| held.replays())
            .unwrap_or_default();
        let replayed = replays.into_iter().filter_map(|(channel, note, velocity)| {
            replayed_note_on(instruments, channel, note, velocity, transpose_offset)
        });
        let lit = self
            .link_arp
            .held_notes
            .get(layer_index)
            .map_or(&[][..], Vec::as_slice)
            .iter()
            .map(|note| {
                let event = MusicalEvent::NoteOn {
                    channel: note.channel,
                    note: note.note,
                    velocity: note.velocity,
                    duration_ms: None,
                };
                (note.audio, event)
            });
        let mut routed = RoutedMusicalEvents::default();
        let mut seen = Vec::new();
        for (audio, event) in replayed.chain(lit) {
            let MusicalEvent::NoteOn { channel, note, .. } = event else {
                continue;
            };
            if seen.contains(&(audio, channel, note)) {
                continue;
            }
            seen.push((audio, channel, note));
            if audio {
                routed.audio.push(event);
            } else {
                routed.midi.push(event);
            }
        }
        routed
    }
}

/// The One-Shot notes one tile's On routed to: internal ones and MIDI Inst. ones, not MIDI Drum
/// strikes.
pub(super) fn lit_one_shot_notes<'a>(
    routed: &'a RoutedMusicalEvents,
    intent: &TileTriggerIntent,
    midi_drum_channels: u16,
) -> impl Iterator<Item = LinkArpHeldNote> + 'a {
    let tile = intent.y * GRID_WIDTH + intent.x;
    let audio = routed.audio.iter().map(|event| (true, event));
    let midi = routed.midi.iter().map(|event| (false, event));
    audio
        .chain(midi)
        .filter_map(move |(audio, event)| match *event {
            MusicalEvent::NoteOn {
                channel,
                note,
                velocity,
                duration_ms: Some(_),
            } if audio || midi_drum_channels & 1 << channel == 0 => Some(LinkArpHeldNote {
                audio,
                channel,
                note,
                velocity,
                tile,
            }),
            _ => None,
        })
}
