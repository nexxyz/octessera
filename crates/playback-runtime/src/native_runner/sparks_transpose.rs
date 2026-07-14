use super::{LedColor, NativeRunner, RoutedMusicalEvents, GRID_HEIGHT, GRID_WIDTH};
use platform_core::MusicalEvent;

pub(super) fn sparks_transpose_offset_at(x: usize, y: usize) -> Option<i8> {
    if x == 0 || y == 0 || y == 7 || x >= GRID_WIDTH || y >= GRID_HEIGHT {
        return None;
    }
    let octave = match y {
        1 | 2 => -1,
        3 | 4 => 0,
        5 | 6 => 1,
        _ => return None,
    };
    let semitone = match y {
        1 | 3 | 5 => [0, 2, 4, 5, 7, 9, 11].get(x - 1).copied(),
        2 | 4 | 6 => match x {
            1 => Some(1),
            2 => Some(3),
            4 => Some(6),
            5 => Some(8),
            6 => Some(10),
            _ => None,
        },
        _ => None,
    }?;
    Some((semitone + octave * 12) as i8)
}

impl NativeRunner {
    pub(super) fn handle_sparks_transpose_grid_press(&mut self, x: usize, y: usize) {
        if x == 0 {
            self.toggle_sparks_transpose_layer(y);
            return;
        }
        let Some(offset) = sparks_transpose_offset_at(x, y) else {
            return;
        };
        let mut changed = 0;
        let mut drain_layers = Vec::new();
        for layer in 0..self.sparks_transpose_offsets.len() {
            if self.sparks_transpose_selected.get(layer) == Some(&true)
                && self.sparks_transpose_enabled.get(layer) == Some(&true)
                && self.sparks_transpose_layer_eligible(layer)
                && self.sparks_transpose_offsets[layer] != offset
            {
                drain_layers.push(layer);
                self.sparks_transpose_offsets[layer] = offset;
                changed += 1;
            }
        }
        if changed > 0 {
            self.drain_sparks_transpose_layers(&drain_layers);
            self.show_toast(format!("Transpose {offset:+}"));
        }
    }

    pub(super) fn toggle_all_sparks_transpose_layers(&mut self) {
        let any_off = (0..self.sparks_transpose_enabled.len()).any(|layer| {
            self.sparks_transpose_layer_eligible(layer)
                && self.sparks_transpose_enabled.get(layer) != Some(&true)
        });
        for layer in 0..self.sparks_transpose_enabled.len() {
            if self.sparks_transpose_layer_eligible(layer) {
                self.sparks_transpose_enabled[layer] = any_off;
            }
        }
        if !any_off {
            let layers = (0..self.sparks_transpose_enabled.len()).collect::<Vec<_>>();
            self.drain_sparks_transpose_layers(&layers);
        }
        self.show_toast(if any_off {
            "Transpose all on"
        } else {
            "Transpose all off"
        });
    }

    pub(super) fn sparks_transpose_offsets_for_routing(&self) -> Vec<i8> {
        (0..self.sparks_transpose_offsets.len())
            .map(|layer| {
                if self.sparks_transpose_enabled.get(layer) == Some(&true)
                    && self.sparks_transpose_selected.get(layer) == Some(&true)
                    && self.sparks_transpose_layer_eligible(layer)
                {
                    self.sparks_transpose_offsets[layer]
                } else {
                    0
                }
            })
            .collect()
    }

    pub(super) fn apply_sparks_transpose_overlay(&self, leds: &mut [LedColor]) {
        self.dim_leds(leds, 4);
        for layer in 0..GRID_HEIGHT {
            let eligible = self.sparks_transpose_layer_eligible(layer);
            let selected = self.sparks_transpose_selected.get(layer) == Some(&true);
            let enabled = self.sparks_transpose_enabled.get(layer) == Some(&true);
            let color = if eligible && selected && enabled {
                LedColor::GREEN
            } else if eligible && selected {
                LedColor::BLUE.dim(2)
            } else if eligible {
                LedColor::SYSTEM.dim(4)
            } else {
                LedColor::BLACK
            };
            self.set_display_led(leds, 0, layer, color);
        }
        let selected_offsets = (0..self.sparks_transpose_offsets.len())
            .filter(|layer| {
                self.sparks_transpose_enabled.get(*layer) == Some(&true)
                    && self.sparks_transpose_selected.get(*layer) == Some(&true)
                    && self.sparks_transpose_layer_eligible(*layer)
            })
            .map(|layer| self.sparks_transpose_offsets[layer])
            .collect::<Vec<_>>();
        for y in 1..=6 {
            for x in 1..GRID_WIDTH {
                if let Some(offset) = sparks_transpose_offset_at(x, y) {
                    let selected = selected_offsets.contains(&offset);
                    let color = if selected {
                        LedColor::GREEN
                    } else if offset == 0 {
                        LedColor::WHITE
                    } else {
                        LedColor::BLUE.dim(3)
                    };
                    self.set_display_led(leds, x, y, color);
                }
            }
        }
    }

    fn toggle_sparks_transpose_layer(&mut self, layer: usize) {
        if layer >= self.sparks_transpose_selected.len()
            || !self.sparks_transpose_layer_eligible(layer)
        {
            return;
        }
        self.sparks_transpose_selected[layer] = !self.sparks_transpose_selected[layer];
        if !self.sparks_transpose_selected[layer] {
            self.drain_sparks_transpose_layers(&[layer]);
        }
    }

    pub(super) fn drain_all_sparks_transpose_notes(&mut self) {
        let layers = (0..self.sparks_transpose_active_notes.len()).collect::<Vec<_>>();
        self.drain_sparks_transpose_layers(&layers);
    }

    pub(super) fn drain_sparks_transpose_instrument_notes(&mut self, instrument_index: usize) {
        let mut drained = RoutedMusicalEvents::default();
        for active_notes in &mut self.sparks_transpose_active_notes {
            let keys = active_notes
                .keys()
                .copied()
                .filter(|(channel, _)| usize::from(*channel) == instrument_index)
                .collect::<Vec<_>>();
            for key in keys {
                let Some(held_notes) = active_notes.remove(&key) else {
                    continue;
                };
                for held_note in held_notes {
                    let event = MusicalEvent::NoteOff {
                        channel: held_note.routed_channel,
                        note: held_note.routed_note,
                    };
                    if held_note.routed_to_midi {
                        drained.midi.push(event);
                    } else {
                        drained.audio.push(event);
                    }
                }
            }
        }
        self.pending_transpose_note_offs.extend(drained);
    }

    fn drain_sparks_transpose_layers(&mut self, layers: &[usize]) {
        let mut drained = RoutedMusicalEvents::default();
        for layer in layers {
            let Some(active_notes) = self.sparks_transpose_active_notes.get_mut(*layer) else {
                continue;
            };
            for held_notes in std::mem::take(active_notes).into_values() {
                for held_note in held_notes {
                    let event = MusicalEvent::NoteOff {
                        channel: held_note.routed_channel,
                        note: held_note.routed_note,
                    };
                    if held_note.routed_to_midi {
                        drained.midi.push(event);
                    } else {
                        drained.audio.push(event);
                    }
                }
            }
        }
        self.pending_transpose_note_offs.extend(drained);
    }

    fn sparks_transpose_layer_eligible(&self, layer: usize) -> bool {
        let Some(sense) = self.pulses_layers.get(layer) else {
            return false;
        };
        [
            (sense.scanned_slot, sense.scanned_action.as_str()),
            (
                sense.scanned_empty_slot,
                sense.scanned_empty_action.as_str(),
            ),
            (sense.activate_slot, sense.activate_action.as_str()),
            (sense.stable_slot, sense.stable_action.as_str()),
            (sense.deactivate_slot, sense.deactivate_action.as_str()),
        ]
        .into_iter()
        .any(|(slot, action)| self.sparks_transpose_target_eligible(slot, action))
    }

    fn sparks_transpose_target_eligible(&self, slot: usize, action: &str) -> bool {
        if !matches!(action, "note_on" | "note_off") {
            return false;
        }
        let Some(instrument) = self.instruments.get(slot) else {
            return false;
        };
        instrument.kind == "synth" || (instrument.kind == "midi" && instrument.midi_enabled)
    }
}
