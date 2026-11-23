//! Drains end held keys outside a source's own note-offs: a layer's gate closing, behavior change
//! or scan change releases every key its target instruments hold, from any source, and stops its
//! Arp; Stop and a patch load release every key. Releases still waiting for a row's Delay land
//! with the drain.

use super::held_keys::KeyEvent;
use super::link_routing::{dedupe_note_ons, releases_before_presses};
use super::modulation::RoutedMusicalEvents;
use super::NativeRunner;
use platform_core::MusicalEvent;

const ALL_NOTES_OFF: u8 = 123;

impl NativeRunner {
    /// Plays `events` through the held keys into their output lanes, every release of a key
    /// before its presses, after merging the tick's duplicates.
    pub(super) fn sound_keys(&mut self, events: &mut RoutedMusicalEvents) {
        releases_before_presses(&mut events.keys);
        dedupe_note_ons(events, &self.instruments);
        let keys = std::mem::take(&mut events.keys);
        let stopped = self.transport.transport != super::RuntimeTransportState::Playing;
        self.held_keys
            .follow_host_time(self.display.transients.now(), stopped);
        self.held_keys.play(keys, &self.note_behaviors, events);
        for event in &events.midi {
            if let MusicalEvent::NoteOn { channel, .. } = event {
                self.transport.midi_channels_sounded |= 1 << (channel & 15);
            }
        }
    }

    pub(super) fn drain_layer_owned_notes(&mut self, layer_index: usize) {
        self.drain_layer_notes(layer_index, self.layer_target_slots(layer_index));
    }

    /// Drains the layer and releases every key the `slots` bit mask holds, before the layer's
    /// next notes.
    pub(super) fn drain_layer_notes(&mut self, layer_index: usize, slots: u16) {
        let mut drained = self.layer_drain(layer_index, slots);
        self.sound_keys(&mut drained);
        self.transpose.pending_note_offs.extend(drained);
    }

    /// What draining `layer_index` plays: its Arp lets go and its queued presses go, their
    /// releases land, its mono lanes and its scan line forget what they saw, and every key the
    /// `slots` bit mask holds is released.
    pub(super) fn layer_drain(&mut self, layer_index: usize, slots: u16) -> RoutedMusicalEvents {
        if let Some(layer) = self.layers.get_mut(layer_index) {
            layer.mono_lanes.forget();
            layer.engine.forget_scanned_line();
        }
        self.clear_link_arp_state_for_layer(layer_index);
        let mut drained = RoutedMusicalEvents {
            keys: self.clear_delayed_link_events_for_layer(layer_index),
            ..RoutedMusicalEvents::default()
        };
        drained.keys.push(KeyEvent::ReleaseAll { slots });
        drained
    }

    pub(super) fn drain_all_layer_notes(&mut self) {
        for layer_index in 0..self.delayed_link_events.len() {
            self.clear_delayed_link_events_for_layer(layer_index);
        }
        self.clear_all_link_arp_state();
        for layer in &mut self.layers {
            layer.mono_lanes.forget();
            layer.engine.forget_scanned_line();
        }
        self.forget_keyboard_notes();
        let mut released = RoutedMusicalEvents::default();
        self.held_keys.release_all(&mut released);
        self.transpose.pending_note_offs.extend(released);
    }

    /// Releases every key `slot` holds, from any source, as an instrument change ends its voices.
    pub(super) fn release_instrument_keys(&mut self, slot: usize) {
        let mut released = RoutedMusicalEvents::default();
        let slots = u32::try_from(slot)
            .ok()
            .and_then(|slot| 1_u16.checked_shl(slot))
            .unwrap_or(0);
        self.held_keys.release_slots(slots, &mut released);
        self.transpose.pending_note_offs.extend(released);
    }

    /// Stop's backstop: All Notes Off on every external MIDI channel that sounded since the last
    /// Stop, sent after the note-offs the transport reset queued.
    pub(super) fn queue_all_notes_off_for_sounded_midi_channels(&mut self) {
        let channels = std::mem::take(&mut self.transport.midi_channels_sounded);
        if !self.midi.enabled {
            return;
        }
        for channel in (0..16).filter(|channel| channels & (1 << channel) != 0) {
            self.transpose
                .pending_note_offs
                .midi
                .push(MusicalEvent::Cc {
                    channel,
                    controller: ALL_NOTES_OFF,
                    value: 0,
                });
        }
    }
}
