use super::{LedColor, NativeRunner, GRID_HEIGHT, GRID_WIDTH};

pub(super) fn dance_transpose_offset_at(x: usize, y: usize) -> Option<i8> {
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
    pub(super) fn handle_dance_transpose_grid_press(&mut self, x: usize, y: usize) {
        if x == 0 {
            self.toggle_dance_transpose_part(y);
            return;
        }
        let Some(offset) = dance_transpose_offset_at(x, y) else {
            return;
        };
        let mut changed = 0;
        for part in 0..self.dance_transpose_offsets.len() {
            if self.dance_transpose_selected.get(part) == Some(&true)
                && self.dance_transpose_enabled.get(part) == Some(&true)
                && self.dance_transpose_part_eligible(part)
            {
                self.dance_transpose_offsets[part] = offset;
                changed += 1;
            }
        }
        if changed > 0 {
            self.show_toast(format!("Transpose {offset:+}"));
        }
    }

    pub(super) fn toggle_all_dance_transpose_parts(&mut self) {
        let any_off = (0..self.dance_transpose_enabled.len()).any(|part| {
            self.dance_transpose_part_eligible(part)
                && self.dance_transpose_enabled.get(part) != Some(&true)
        });
        for part in 0..self.dance_transpose_enabled.len() {
            if self.dance_transpose_part_eligible(part) {
                self.dance_transpose_enabled[part] = any_off;
            }
        }
        self.show_toast(if any_off {
            "Transpose all on"
        } else {
            "Transpose all off"
        });
    }

    pub(super) fn dance_transpose_offsets_for_routing(&self) -> Vec<i8> {
        (0..self.dance_transpose_offsets.len())
            .map(|part| {
                if self.dance_transpose_enabled.get(part) == Some(&true)
                    && self.dance_transpose_selected.get(part) == Some(&true)
                    && self.dance_transpose_part_eligible(part)
                {
                    self.dance_transpose_offsets[part]
                } else {
                    0
                }
            })
            .collect()
    }

    pub(super) fn apply_dance_transpose_overlay(&self, leds: &mut [LedColor]) {
        self.dim_leds(leds, 4);
        for part in 0..GRID_HEIGHT {
            let eligible = self.dance_transpose_part_eligible(part);
            let selected = self.dance_transpose_selected.get(part) == Some(&true);
            let enabled = self.dance_transpose_enabled.get(part) == Some(&true);
            let color = if eligible && selected && enabled {
                LedColor::rgb(0, 220, 150)
            } else if eligible && selected {
                LedColor::rgb(0, 90, 80)
            } else if eligible {
                LedColor::rgb(0, 45, 40)
            } else {
                LedColor::rgb(20, 20, 20)
            };
            self.set_display_led(leds, 0, part, color);
        }
        for y in 1..=6 {
            for x in 1..GRID_WIDTH {
                if let Some(offset) = dance_transpose_offset_at(x, y) {
                    let color = if offset == 0 {
                        LedColor::rgb(255, 255, 255)
                    } else {
                        LedColor::rgb(90, 90, 180)
                    };
                    self.set_display_led(leds, x, y, color);
                }
            }
        }
    }

    fn toggle_dance_transpose_part(&mut self, part: usize) {
        if part >= self.dance_transpose_selected.len() || !self.dance_transpose_part_eligible(part)
        {
            return;
        }
        self.dance_transpose_selected[part] = !self.dance_transpose_selected[part];
    }

    fn dance_transpose_part_eligible(&self, part: usize) -> bool {
        let Some(sense) = self.sense_parts.get(part) else {
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
        .any(|(slot, action)| self.dance_transpose_target_eligible(slot, action))
    }

    fn dance_transpose_target_eligible(&self, slot: usize, action: &str) -> bool {
        if !matches!(action, "note_on" | "note_off") {
            return false;
        }
        let Some(instrument) = self.instruments.get(slot) else {
            return false;
        };
        instrument.kind == "synth" || (instrument.kind == "midi" && instrument.midi_enabled)
    }
}
