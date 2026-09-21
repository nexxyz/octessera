use super::modulation::RoutedMusicalEvents;
use super::NativeRunner;
use platform_core::MusicalEvent;

impl NativeRunner {
    pub(super) fn drain_layer_owned_notes(&mut self, layer_index: usize) {
        let audio_start = self.pending_transpose_note_offs.audio.len();
        let midi_start = self.pending_transpose_note_offs.midi.len();
        self.clear_delayed_link_events_for_layer(layer_index);
        self.drain_layer_engine_notes(layer_index);
        self.drain_play_transpose_layers_for_layer_disable(layer_index);
        let mut drained = RoutedMusicalEvents {
            audio: self
                .pending_transpose_note_offs
                .audio
                .split_off(audio_start),
            midi: self.pending_transpose_note_offs.midi.split_off(midi_start),
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
        released
    }

    pub(super) fn clear_layer_emitted_route_notes(&mut self, layer_index: usize) {
        if let Some(owners) = self.emitted_route_note_owners.get_mut(layer_index) {
            owners.clear();
        }
    }

    pub(super) fn clear_all_emitted_route_notes(&mut self) {
        for owners in &mut self.emitted_route_note_owners {
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
