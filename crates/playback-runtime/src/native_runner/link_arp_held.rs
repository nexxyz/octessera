//! What a held Arp holds: a Hold note from its note-on to its note-off, and a One-Shot note for
//! as long as the tile that played it stays lit.

use super::modulation::RoutedMusicalEvents;
use super::{LinkArpHeldNote, NativeRunner, GRID_WIDTH};
use platform_core::{MusicalEvent, TileTriggerIntent};

impl NativeRunner {
    /// Forgets the One-Shot notes of tiles the layer no longer shows lit, whatever its Off Trig.
    pub(super) fn release_dark_link_arp_tiles(&mut self, layer_index: usize, lit_tiles: &[bool]) {
        if let Some(held) = self.link_arp.held_notes.get_mut(layer_index) {
            held.retain(|note| {
                note.tile
                    .is_none_or(|tile| lit_tiles.get(tile).copied() == Some(true))
            });
        }
    }

    /// Adds a batch's Hold notes and the One-Shot notes `lit` by its tiles; a note-off ends only
    /// the Hold note of its pitch.
    pub(super) fn update_link_arp_held_notes(
        &mut self,
        layer_index: usize,
        routed: &RoutedMusicalEvents,
        lit: Vec<LinkArpHeldNote>,
    ) {
        let Some(held) = self.link_arp.held_notes.get_mut(layer_index) else {
            return;
        };
        for (audio, events) in [(true, &routed.audio), (false, &routed.midi)] {
            for event in events {
                let (channel, note, velocity) = match *event {
                    MusicalEvent::NoteOn {
                        channel,
                        note,
                        velocity,
                        duration_ms: None,
                    } => (channel, note, Some(velocity)),
                    MusicalEvent::NoteOff { channel, note } => (channel, note, None),
                    _ => continue,
                };
                held.retain(|held| {
                    held.tile.is_some()
                        || (held.audio, held.channel, held.note) != (audio, channel, note)
                });
                if let Some(velocity) = velocity {
                    held.push(LinkArpHeldNote {
                        audio,
                        channel,
                        note,
                        velocity,
                        tile: None,
                    });
                }
            }
        }
        for note in lit {
            held.retain(|held| {
                (held.tile, held.audio, held.channel) != (note.tile, note.audio, note.channel)
            });
            held.push(note);
        }
    }

    /// The held set as note-ons, one per pitch even when two lit tiles share it.
    pub(super) fn held_notes_as_routed(&self, layer_index: usize) -> RoutedMusicalEvents {
        let mut routed = RoutedMusicalEvents::default();
        let held = self
            .link_arp
            .held_notes
            .get(layer_index)
            .map_or(&[][..], Vec::as_slice);
        for (index, note) in held.iter().enumerate() {
            let key = (note.audio, note.channel, note.note);
            if held[..index]
                .iter()
                .any(|other| (other.audio, other.channel, other.note) == key)
            {
                continue;
            }
            let event = MusicalEvent::NoteOn {
                channel: note.channel,
                note: note.note,
                velocity: note.velocity,
                duration_ms: None,
            };
            if note.audio {
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
                tile: Some(tile),
            }),
            _ => None,
        })
}
