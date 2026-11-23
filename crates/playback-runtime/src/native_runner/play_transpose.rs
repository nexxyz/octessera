use super::led_color::LedColor;
use super::{
    InstrumentType, NativeRunner, RoutedMusicalEvents, TriggerAction, GRID_HEIGHT, GRID_WIDTH,
    LAYER_COUNT,
};

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

/// The Transpose page's live state per layer, never saved: the selection, the on/off state and
/// offset, and the note-offs a retarget leaves to send.
#[derive(Clone, Debug)]
pub(super) struct PlayTranspose {
    pub(super) selected: Vec<bool>,
    pub(super) enabled: Vec<bool>,
    pub(super) offsets: Vec<i8>,
    pub(super) pending_note_offs: RoutedMusicalEvents,
}

impl Default for PlayTranspose {
    fn default() -> Self {
        Self {
            selected: vec![true; LAYER_COUNT],
            enabled: vec![true; LAYER_COUNT],
            offsets: vec![0; LAYER_COUNT],
            pending_note_offs: RoutedMusicalEvents::default(),
        }
    }
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
        for layer in 0..self.transpose.offsets.len() {
            if self.transpose.selected.get(layer) == Some(&true)
                && self.transpose.enabled.get(layer) == Some(&true)
                && self.play_transpose_layer_eligible(layer)
                && self.transpose.offsets[layer] != offset
            {
                drain_layers.push(layer);
                self.transpose.offsets[layer] = offset;
                changed += 1;
            }
        }
        if changed > 0 {
            self.drain_play_transpose_layers(&drain_layers);
            self.show_toast(format!("Transpose {offset:+}"));
        }
    }

    pub(super) fn toggle_all_play_transpose_layers(&mut self) {
        let any_off = (0..self.transpose.enabled.len()).any(|layer| {
            self.play_transpose_layer_eligible(layer)
                && self.transpose.enabled.get(layer) != Some(&true)
        });
        for layer in 0..self.transpose.enabled.len() {
            if self.play_transpose_layer_eligible(layer) {
                self.transpose.enabled[layer] = any_off;
            }
        }
        if !any_off {
            let layers = (0..self.transpose.enabled.len()).collect::<Vec<_>>();
            self.drain_play_transpose_layers(&layers);
        }
        self.show_toast(if any_off {
            "Transpose all on"
        } else {
            "Transpose all off"
        });
    }

    pub(super) fn play_transpose_offsets_for_routing(&self) -> Vec<i8> {
        (0..self.transpose.offsets.len())
            .map(|layer| self.play_transpose_offset_for_layer(layer))
            .collect()
    }

    /// The semitones Play Transpose shifts a layer's notes by; 0 when it does not apply.
    pub(super) fn play_transpose_offset_for_layer(&self, layer: usize) -> i8 {
        match self.transpose.offsets.get(layer) {
            Some(&offset)
                if self.transpose.enabled.get(layer) == Some(&true)
                    && self.transpose.selected.get(layer) == Some(&true)
                    && self.play_transpose_layer_eligible(layer) =>
            {
                offset
            }
            _ => 0,
        }
    }

    pub(super) fn apply_play_transpose_overlay(&self, leds: &mut [LedColor]) {
        self.dim_leds(leds, 4);
        for layer in 0..GRID_HEIGHT {
            let eligible = self.play_transpose_layer_eligible(layer);
            let selected = self.transpose.selected.get(layer) == Some(&true);
            let enabled = self.transpose.enabled.get(layer) == Some(&true);
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
        let selected_offsets = (0..self.transpose.offsets.len())
            .filter(|layer| {
                self.transpose.enabled.get(*layer) == Some(&true)
                    && self.transpose.selected.get(*layer) == Some(&true)
                    && self.play_transpose_layer_eligible(*layer)
            })
            .map(|layer| self.transpose.offsets[layer])
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
        if layer >= self.transpose.selected.len() || !self.play_transpose_layer_eligible(layer) {
            return;
        }
        self.transpose.selected[layer] = !self.transpose.selected[layer];
        if !self.transpose.selected[layer] {
            self.drain_play_transpose_layers(&[layer]);
        }
    }

    pub(super) fn drain_all_play_transpose_notes(&mut self) {
        let layers = (0..self.held_notes.len()).collect::<Vec<_>>();
        self.drain_play_transpose_layers(&layers);
    }

    pub(super) fn drain_play_transpose_instrument_notes(&mut self, instrument_index: usize) {
        let channel = instrument_index as u8;
        let mut drained = RoutedMusicalEvents::default();
        for layer_index in 0..self.held_notes.len() {
            let mut layer_drained = RoutedMusicalEvents::default();
            self.held_notes[layer_index].release_sounding(Some(channel), &mut layer_drained);
            self.held_notes[layer_index].forget_channel(channel);
            if !layer_drained.is_empty() {
                self.forget_layer_owned_route_note_offs(layer_index, &layer_drained);
                drained.extend(layer_drained);
            }
        }
        self.transpose.pending_note_offs.extend(drained);
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
            let Some(held) = self.held_notes.get_mut(*layer) else {
                continue;
            };
            held.release_sounding(None, &mut layer_drained);
            if forget_ownership {
                self.forget_layer_owned_route_note_offs(*layer, &layer_drained);
            }
            drained.extend(layer_drained);
        }
        self.transpose.pending_note_offs.extend(drained);
    }

    fn play_transpose_layer_eligible(&self, layer: usize) -> bool {
        let Some(sense) = self.layers.get(layer).map(|layer| &layer.link) else {
            return false;
        };
        [
            (sense.activate_slot, sense.activate_action),
            (sense.stable_slot, sense.stable_action),
            (sense.deactivate_slot, sense.deactivate_action),
        ]
        .into_iter()
        .any(|(slot, action)| self.play_transpose_target_eligible(slot, action))
    }

    pub(super) fn play_transpose_target_eligible(
        &self,
        slot: usize,
        action: TriggerAction,
    ) -> bool {
        if action == TriggerAction::None {
            return false;
        }
        let Some(instrument) = self.instruments.get(slot) else {
            return false;
        };
        match instrument.kind {
            InstrumentType::Synth | InstrumentType::Fm | InstrumentType::Pluck => true,
            InstrumentType::Midi => instrument.midi_enabled,
            InstrumentType::Sampler => instrument.sample_follow_transpose,
            InstrumentType::Drum => instrument
                .drum_config
                .get("followTranspose")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false),
            InstrumentType::None | InstrumentType::MidiDrum => false,
        }
    }
}
