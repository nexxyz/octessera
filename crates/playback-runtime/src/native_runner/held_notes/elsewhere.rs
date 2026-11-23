//! A note-off from one layer for a pitch another layer or Notes In holds on that instrument ends
//! it there, as that holder sounds it.

use super::{HeldNote, HeldNotes, NoteBehaviorWithIntentsResult, Play};
use crate::native_runner::link_routing::link_delay_for_kind;
use crate::native_runner::modulation::RoutedMusicalEvents;
use crate::native_runner::{DelayedRoutedEvents, NativeRunner};
use platform_core::{MusicalEvent, NoteBehavior, TileTriggerKind};

impl NativeRunner {
    /// Ends `key` wherever another layer or Notes In holds it, as that holder sounds it, after
    /// the Delay of the sending layer's `kind` row, as that layer's own note-offs go.
    pub(super) fn end_held_note_elsewhere(
        &mut self,
        layer_index: usize,
        key: HeldNote,
        kind: Option<TileTriggerKind>,
    ) {
        let mut released = RoutedMusicalEvents::default();
        for other in (0..self.held_notes.len()).filter(|other| *other != layer_index) {
            let mut ended = RoutedMusicalEvents::default();
            if self.held_notes[other].end_held(key, &mut ended) {
                self.forget_layer_owned_route_note_offs(other, &ended);
                released.extend(ended);
            }
        }
        self.release_keyboard_note(key.channel, key.note, &mut released);
        let delay =
            link_delay_for_kind(self.layers.get(layer_index).map(|layer| &layer.link), kind);
        match self.delayed_link_events.get_mut(layer_index) {
            Some(queue) if delay > 0 => queue.push(DelayedRoutedEvents {
                remaining_steps: u16::from(delay),
                events: released,
            }),
            _ => self.ended_elsewhere.extend(released),
        }
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

    /// A note-off this layer sends for a pitch it held ends that instrument's voices of it, so
    /// every other ledger holding the pitch lets go of it too.
    pub(super) fn end_ended_notes_elsewhere(
        &self,
        out: &mut NoteBehaviorWithIntentsResult,
        play: &Play<'_>,
    ) {
        for (event, intent) in out.events.iter().zip(&out.event_intents) {
            let MusicalEvent::NoteOff { channel, note } = *event else {
                continue;
            };
            let key = HeldNote { channel, note };
            let hold =
                play.behaviors.get(usize::from(channel)).copied() == Some(NoteBehavior::Hold);
            if hold
                && (play.elsewhere)(key)
                && !out.ended_elsewhere.iter().any(|(ended, _)| *ended == key)
            {
                out.ended_elsewhere
                    .push((key, intent.as_ref().map(|intent| intent.kind)));
            }
        }
    }
}
