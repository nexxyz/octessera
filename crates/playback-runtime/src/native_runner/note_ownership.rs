use super::modulation::RoutedMusicalEvents;
use super::modulation_sampler::cell_release;
use super::NativeRunner;
use crate::protocol::CellHit;
use platform_core::MusicalEvent;

impl NativeRunner {
    pub(super) fn drain_layer_owned_notes(&mut self, layer_index: usize) {
        let audio_start = self.pending_transpose_note_offs.audio.len();
        let midi_start = self.pending_transpose_note_offs.midi.len();
        let cells_start = self.pending_transpose_note_offs.cells.len();
        self.clear_delayed_link_events_for_layer(layer_index);
        self.drain_layer_engine_notes(layer_index);
        self.drain_play_transpose_layers_for_layer_disable(layer_index);
        let mut drained = RoutedMusicalEvents {
            audio: self
                .pending_transpose_note_offs
                .audio
                .split_off(audio_start),
            midi: self.pending_transpose_note_offs.midi.split_off(midi_start),
            cells: self
                .pending_transpose_note_offs
                .cells
                .split_off(cells_start),
        };
        drained.extend(self.layer_owned_route_note_offs(layer_index));
        let released = self.release_layer_owned_route_notes(layer_index, drained);
        self.pending_transpose_note_offs.extend(released);
        self.clear_layer_replacement_state(layer_index);
        self.clear_layer_emitted_route_notes(layer_index);
    }

    pub(super) fn track_emitted_route_notes(
        &mut self,
        layer_index: usize,
        events: &RoutedMusicalEvents,
    ) {
        for (audio, lane) in [(true, &events.audio), (false, &events.midi)] {
            for event in lane {
                match event {
                    MusicalEvent::NoteOn {
                        channel,
                        note,
                        duration_ms: None,
                        ..
                    } => {
                        if let Some(owners) = self.emitted_route_note_owners.get_mut(layer_index) {
                            owners.insert((audio, *channel, *note));
                        }
                    }
                    MusicalEvent::NoteOff { channel, note } => {
                        self.emitted_route_note_owners
                            .get_mut(layer_index)
                            .map(|owners| owners.remove(&(audio, *channel, *note)));
                    }
                    _ => {}
                }
            }
        }
        for hit in &events.cells {
            let held = self.is_held_cell(layer_index, hit);
            if let Some(owners) = self.emitted_route_cell_owners.get_mut(layer_index) {
                if held {
                    owners.insert((hit.instrument_slot, hit.cell));
                } else if hit.velocity == 0 {
                    owners.remove(&(hit.instrument_slot, hit.cell));
                }
            }
        }
    }

    /// A sampler cell is sustained when its note-on was recorded in the layer's held-note map;
    /// one-shot cells are not, and are never released by layer drains.
    pub(super) fn is_held_cell(&self, layer_index: usize, hit: &CellHit) -> bool {
        hit.velocity > 0
            && self
                .play_transpose_active_notes
                .get(layer_index)
                .is_some_and(|notes| {
                    notes.values().flatten().any(|held| {
                        held.routed_channel == hit.instrument_slot
                            && held.sampler_cell == Some((hit.voice, hit.cell))
                    })
                })
    }

    pub(super) fn forget_layer_owned_route_note_offs(
        &mut self,
        layer_index: usize,
        events: &RoutedMusicalEvents,
    ) {
        for (audio, lane) in [(true, &events.audio), (false, &events.midi)] {
            for event in lane {
                let MusicalEvent::NoteOff { channel, note } = event else {
                    continue;
                };
                if let Some(owners) = self.emitted_route_note_owners.get_mut(layer_index) {
                    owners.remove(&(audio, *channel, *note));
                }
            }
        }
        if let Some(owners) = self.emitted_route_cell_owners.get_mut(layer_index) {
            for hit in events.cells.iter().filter(|hit| hit.velocity == 0) {
                owners.remove(&(hit.instrument_slot, hit.cell));
            }
        }
    }

    pub(super) fn layer_owned_route_note_offs(&self, layer_index: usize) -> RoutedMusicalEvents {
        let mut events = RoutedMusicalEvents::default();
        let Some(owners) = self.emitted_route_note_owners.get(layer_index) else {
            return events;
        };
        for (audio, channel, note) in owners {
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
        if let Some(owners) = self.emitted_route_cell_owners.get(layer_index) {
            events.cells.extend(
                owners
                    .iter()
                    .map(|&(slot, cell)| cell_release(slot, 0, cell)),
            );
        }
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
                    .emitted_route_note_owners
                    .get_mut(layer_index)
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
        released.cells = self.release_layer_owned_cells(layer_index, events.cells);
        released
    }

    fn release_layer_owned_cells(
        &mut self,
        layer_index: usize,
        hits: Vec<CellHit>,
    ) -> Vec<CellHit> {
        let mut released = Vec::new();
        for hit in hits.into_iter().filter(|hit| hit.velocity == 0) {
            let key = (hit.instrument_slot, hit.cell);
            let removed = self
                .emitted_route_cell_owners
                .get_mut(layer_index)
                .is_some_and(|owners| owners.remove(&key));
            let owned_elsewhere = self
                .emitted_route_cell_owners
                .iter()
                .enumerate()
                .any(|(index, owners)| index != layer_index && owners.contains(&key));
            if removed && !owned_elsewhere {
                released.push(hit);
            }
        }
        released
    }

    pub(super) fn clear_layer_emitted_route_notes(&mut self, layer_index: usize) {
        if let Some(owners) = self.emitted_route_note_owners.get_mut(layer_index) {
            owners.clear();
        }
        if let Some(owners) = self.emitted_route_cell_owners.get_mut(layer_index) {
            owners.clear();
        }
    }

    pub(super) fn clear_all_emitted_route_notes(&mut self) {
        for owners in &mut self.emitted_route_note_owners {
            owners.clear();
        }
        for owners in &mut self.emitted_route_cell_owners {
            owners.clear();
        }
    }

    fn route_note_owned_by_other_layer(&self, layer_index: usize, key: (bool, u8, u8)) -> bool {
        self.emitted_route_note_owners
            .iter()
            .enumerate()
            .any(|(index, owners)| index != layer_index && owners.contains(&key))
    }
}
