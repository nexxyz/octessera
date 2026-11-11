use super::modulation::RoutedMusicalEvents;
use super::modulation_sampler::tile_release;
use super::NativeRunner;
use crate::protocol::TileHit;
use platform_core::MusicalEvent;

impl NativeRunner {
    pub(super) fn drain_layer_owned_notes(&mut self, layer_index: usize) {
        let audio_start = self.transpose.pending_note_offs.audio.len();
        let midi_start = self.transpose.pending_note_offs.midi.len();
        let tiles_start = self.transpose.pending_note_offs.tiles.len();
        self.clear_delayed_link_events_for_layer(layer_index);
        self.drain_layer_engine_notes(layer_index);
        self.drain_play_transpose_layers_for_layer_disable(layer_index);
        let mut drained = RoutedMusicalEvents {
            audio: self
                .transpose
                .pending_note_offs
                .audio
                .split_off(audio_start),
            midi: self.transpose.pending_note_offs.midi.split_off(midi_start),
            tiles: self
                .transpose
                .pending_note_offs
                .tiles
                .split_off(tiles_start),
        };
        drained.extend(self.layer_owned_route_note_offs(layer_index));
        let released = self.release_layer_owned_route_notes(layer_index, drained);
        self.transpose.pending_note_offs.extend(released);
        self.clear_layer_replacement_state(layer_index);
        self.clear_layer_emitted_route_notes(layer_index);
    }

    pub(super) fn track_emitted_route_notes(
        &mut self,
        layer_index: usize,
        events: &RoutedMusicalEvents,
    ) {
        let Some(layer) = self.layers.get_mut(layer_index) else {
            return;
        };
        for (audio, lane) in [(true, &events.audio), (false, &events.midi)] {
            for event in lane {
                match event {
                    MusicalEvent::NoteOn {
                        channel,
                        note,
                        duration_ms: None,
                        ..
                    } => {
                        layer.emitted_note_owners.insert((audio, *channel, *note));
                    }
                    MusicalEvent::NoteOff { channel, note } => {
                        layer.emitted_note_owners.remove(&(audio, *channel, *note));
                    }
                    _ => {}
                }
            }
        }
        for hit in &events.tiles {
            let held = self.is_held_tile(layer_index, hit);
            let owners = &mut self.layers[layer_index].emitted_tile_owners;
            if held {
                owners.insert((hit.instrument_slot, hit.tile));
            } else if hit.velocity == 0 {
                owners.remove(&(hit.instrument_slot, hit.tile));
            }
        }
    }

    /// A sampler tile is sustained when its note-on was recorded in the layer's held-note map;
    /// one-shot tiles are not, and are never released by layer drains.
    pub(super) fn is_held_tile(&self, layer_index: usize, hit: &TileHit) -> bool {
        hit.velocity > 0
            && self
                .transpose
                .active_notes
                .get(layer_index)
                .is_some_and(|notes| {
                    notes.values().flatten().any(|held| {
                        held.routed_channel == hit.instrument_slot
                            && held.sampler_tile == Some((hit.voice, hit.tile))
                    })
                })
    }

    pub(super) fn forget_layer_owned_route_note_offs(
        &mut self,
        layer_index: usize,
        events: &RoutedMusicalEvents,
    ) {
        let Some(layer) = self.layers.get_mut(layer_index) else {
            return;
        };
        for (audio, lane) in [(true, &events.audio), (false, &events.midi)] {
            for event in lane {
                if let MusicalEvent::NoteOff { channel, note } = event {
                    layer.emitted_note_owners.remove(&(audio, *channel, *note));
                }
            }
        }
        for hit in events.tiles.iter().filter(|hit| hit.velocity == 0) {
            layer
                .emitted_tile_owners
                .remove(&(hit.instrument_slot, hit.tile));
        }
    }

    pub(super) fn layer_owned_route_note_offs(&self, layer_index: usize) -> RoutedMusicalEvents {
        let mut events = RoutedMusicalEvents::default();
        let Some(layer) = self.layers.get(layer_index) else {
            return events;
        };
        for (audio, channel, note) in &layer.emitted_note_owners {
            let event = MusicalEvent::NoteOff {
                channel: *channel,
                note: *note,
            };
            if *audio {
                events.audio.push(event);
            } else {
                events.midi.push(event);
            }
        }
        events.tiles.extend(
            layer
                .emitted_tile_owners
                .iter()
                .map(|&(slot, tile)| tile_release(slot, 0, tile)),
        );
        events
    }

    pub(super) fn release_layer_owned_route_notes(
        &mut self,
        layer_index: usize,
        events: RoutedMusicalEvents,
    ) -> RoutedMusicalEvents {
        let mut released = RoutedMusicalEvents::default();
        for (audio, lane) in [(true, events.audio), (false, events.midi)] {
            for event in lane {
                let MusicalEvent::NoteOff { channel, note } = event else {
                    continue;
                };
                let key = (audio, channel, note);
                let removed = self
                    .layers
                    .get_mut(layer_index)
                    .map(|layer| &mut layer.emitted_note_owners)
                    .is_some_and(|owners| owners.remove(&key));
                if !removed || self.route_note_owned_by_other_layer(layer_index, key) {
                    continue;
                }
                if audio {
                    released.audio.push(MusicalEvent::NoteOff { channel, note });
                } else {
                    released.midi.push(MusicalEvent::NoteOff { channel, note });
                }
            }
        }
        released.tiles = self.release_layer_owned_tiles(layer_index, events.tiles);
        released
    }

    fn release_layer_owned_tiles(
        &mut self,
        layer_index: usize,
        hits: Vec<TileHit>,
    ) -> Vec<TileHit> {
        let mut released = Vec::new();
        for hit in hits.into_iter().filter(|hit| hit.velocity == 0) {
            let key = (hit.instrument_slot, hit.tile);
            let removed = self
                .layers
                .get_mut(layer_index)
                .map(|layer| &mut layer.emitted_tile_owners)
                .is_some_and(|owners| owners.remove(&key));
            let owned_elsewhere = self.layers.iter().enumerate().any(|(index, layer)| {
                index != layer_index && layer.emitted_tile_owners.contains(&key)
            });
            if removed && !owned_elsewhere {
                released.push(hit);
            }
        }
        released
    }

    pub(super) fn clear_layer_emitted_route_notes(&mut self, layer_index: usize) {
        if let Some(layer) = self.layers.get_mut(layer_index) {
            layer.emitted_note_owners.clear();
            layer.emitted_tile_owners.clear();
        }
    }

    /// Queues note-offs for emitted notes whose own note-off has not gone out, such as one still
    /// waiting in the Link delay queue, skipping releases already pending, then forgets every
    /// layer's ownership.
    pub(super) fn release_all_remaining_owned_notes(&mut self) {
        for layer_index in 0..self.layers.len() {
            let owned = self.layer_owned_route_note_offs(layer_index);
            let pending = &mut self.transpose.pending_note_offs;
            for (lane, events) in [
                (&mut pending.audio, owned.audio),
                (&mut pending.midi, owned.midi),
            ] {
                for event in events {
                    if !lane.contains(&event) {
                        lane.push(event);
                    }
                }
            }
            for hit in owned.tiles {
                let already_released = pending.tiles.iter().any(|pending| {
                    pending.velocity == 0
                        && pending.instrument_slot == hit.instrument_slot
                        && pending.tile == hit.tile
                });
                if !already_released {
                    pending.tiles.push(hit);
                }
            }
            self.clear_layer_emitted_route_notes(layer_index);
        }
    }

    fn route_note_owned_by_other_layer(&self, layer_index: usize, key: (bool, u8, u8)) -> bool {
        self.layers
            .iter()
            .enumerate()
            .any(|(index, layer)| index != layer_index && layer.emitted_note_owners.contains(&key))
    }
}
