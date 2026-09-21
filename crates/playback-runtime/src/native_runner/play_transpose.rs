use super::led_color::LedColor;
use super::{NativeRunner, RoutedMusicalEvents, GRID_HEIGHT, GRID_WIDTH};
use platform_core::MusicalEvent;

pub(super) fn play_transpose_offset_at(x: usize, y: usize) -> Option<i8> {
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
    pub(super) fn handle_play_transpose_grid_press(&mut self, x: usize, y: usize) {
        if x == 0 {
            self.toggle_play_transpose_layer(y);
            return;
        }
        let Some(offset) = play_transpose_offset_at(x, y) else {
            return;
        };
        let mut changed = 0;
        let mut drain_layers = Vec::new();
        for layer in 0..self.play_transpose_offsets.len() {
            if self.play_transpose_selected.get(layer) == Some(&true)
                && self.play_transpose_enabled.get(layer) == Some(&true)
                && self.play_transpose_layer_eligible(layer)
                && self.play_transpose_offsets[layer] != offset
            {
                drain_layers.push(layer);
                self.play_transpose_offsets[layer] = offset;
                changed += 1;
            }
        }
        if changed > 0 {
            self.drain_play_transpose_layers(&drain_layers);
            self.show_toast(format!("Transpose {offset:+}"));
        }
    }

    pub(super) fn toggle_all_play_transpose_layers(&mut self) {
        let any_off = (0..self.play_transpose_enabled.len()).any(|layer| {
            self.play_transpose_layer_eligible(layer)
                && self.play_transpose_enabled.get(layer) != Some(&true)
        });
        for layer in 0..self.play_transpose_enabled.len() {
            if self.play_transpose_layer_eligible(layer) {
                self.play_transpose_enabled[layer] = any_off;
            }
        }
        if !any_off {
            let layers = (0..self.play_transpose_enabled.len()).collect::<Vec<_>>();
            self.drain_play_transpose_layers(&layers);
        }
        self.show_toast(if any_off {
            "Transpose all on"
        } else {
            "Transpose all off"
        });
    }

    pub(super) fn play_transpose_offsets_for_routing(&self) -> Vec<i8> {
        (0..self.play_transpose_offsets.len())
            .map(|layer| {
                if self.play_transpose_enabled.get(layer) == Some(&true)
                    && self.play_transpose_selected.get(layer) == Some(&true)
                    && self.play_transpose_layer_eligible(layer)
                {
                    self.play_transpose_offsets[layer]
                } else {
                    0
                }
            })
            .collect()
    }

    pub(super) fn apply_play_transpose_overlay(&self, leds: &mut [LedColor]) {
        self.dim_leds(leds, 4);
        for layer in 0..GRID_HEIGHT {
            let eligible = self.play_transpose_layer_eligible(layer);
            let selected = self.play_transpose_selected.get(layer) == Some(&true);
            let enabled = self.play_transpose_enabled.get(layer) == Some(&true);
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
        let selected_offsets = (0..self.play_transpose_offsets.len())
            .filter(|layer| {
                self.play_transpose_enabled.get(*layer) == Some(&true)
                    && self.play_transpose_selected.get(*layer) == Some(&true)
                    && self.play_transpose_layer_eligible(*layer)
            })
            .map(|layer| self.play_transpose_offsets[layer])
            .collect::<Vec<_>>();
        for y in 1..=6 {
            for x in 1..GRID_WIDTH {
                if let Some(offset) = play_transpose_offset_at(x, y) {
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

    fn toggle_play_transpose_layer(&mut self, layer: usize) {
        if layer >= self.play_transpose_selected.len() || !self.play_transpose_layer_eligible(layer)
        {
            return;
        }
        self.play_transpose_selected[layer] = !self.play_transpose_selected[layer];
        if !self.play_transpose_selected[layer] {
            self.drain_play_transpose_layers(&[layer]);
        }
    }

    pub(super) fn drain_all_play_transpose_notes(&mut self) {
        let layers = (0..self.play_transpose_active_notes.len()).collect::<Vec<_>>();
        self.drain_play_transpose_layers(&layers);
    }

    pub(super) fn drain_play_transpose_instrument_notes(&mut self, instrument_index: usize) {
        let mut drained_by_layer = Vec::new();
        for (layer_index, active_notes) in self.play_transpose_active_notes.iter_mut().enumerate() {
            let mut layer_drained = RoutedMusicalEvents::default();
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
                        layer_drained.midi.push(event);
                    } else {
                        layer_drained.audio.push(event);
                    }
                }
            }
            if !layer_drained.is_empty() {
                drained_by_layer.push((layer_index, layer_drained));
            }
        }
        let mut drained = RoutedMusicalEvents::default();
        for (layer_index, layer_drained) in drained_by_layer {
            self.forget_layer_owned_route_note_offs(layer_index, &layer_drained);
            drained.extend(layer_drained);
        }
        self.pending_transpose_note_offs.extend(drained);
    }

    pub(super) fn drain_play_transpose_layers(&mut self, layers: &[usize]) {
        self.drain_play_transpose_layers_inner(layers, true);
    }

    pub(super) fn drain_play_transpose_layers_for_layer_disable(&mut self, layer: usize) {
        self.drain_play_transpose_layers_inner(&[layer], false);
    }

    fn drain_play_transpose_layers_inner(&mut self, layers: &[usize], forget_ownership: bool) {
        let mut drained = RoutedMusicalEvents::default();
        for layer in layers {
            let mut layer_drained = RoutedMusicalEvents::default();
            let Some(active_notes) = self.play_transpose_active_notes.get_mut(*layer) else {
                continue;
            };
            for held_notes in std::mem::take(active_notes).into_values() {
                for held_note in held_notes {
                    let event = MusicalEvent::NoteOff {
                        channel: held_note.routed_channel,
                        note: held_note.routed_note,
                    };
                    if held_note.routed_to_midi {
                        layer_drained.midi.push(event);
                    } else {
                        layer_drained.audio.push(event);
                    }
                }
            }
            if forget_ownership {
                self.forget_layer_owned_route_note_offs(*layer, &layer_drained);
            }
            drained.extend(layer_drained);
        }
        self.pending_transpose_note_offs.extend(drained);
    }

    fn play_transpose_layer_eligible(&self, layer: usize) -> bool {
        let Some(sense) = self.link_layers.get(layer) else {
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
        .any(|(slot, action)| self.play_transpose_target_eligible(slot, action))
    }

    fn play_transpose_target_eligible(&self, slot: usize, action: &str) -> bool {
        if !matches!(action, "note_on" | "note_off") {
            return false;
        }
        let Some(instrument) = self.instruments.get(slot) else {
            return false;
        };
        instrument.kind == "synth" || (instrument.kind == "midi" && instrument.midi_enabled)
    }
}
