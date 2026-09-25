use super::{NativeRunner, GRID_HEIGHT, GRID_WIDTH};
use crate::protocol::DrumHit;
use platform_core::{CellTriggerIntent, CellTriggerKind};
use serde_json::{json, Value};

impl NativeRunner {
    pub(super) fn enter_drum_assign(&mut self, slot: usize, voice: usize) {
        if self
            .instruments
            .get(slot)
            .is_none_or(|instrument| instrument.kind != "drum")
            || voice >= 8
        {
            return;
        }
        self.play_fx_assign = None;
        self.sample_assign = None;
        self.trigger_probability_assign = None;
        self.drum_cell_tune = None;
        self.drum_assign = Some((slot, voice as u8));
        self.show_toast(format!("Assign V{}: grid", voice + 1));
    }

    pub(super) fn enter_drum_cell_tune(&mut self, slot: usize) {
        if self
            .instruments
            .get(slot)
            .is_none_or(|instrument| instrument.kind != "drum")
        {
            return;
        }
        self.play_fx_assign = None;
        self.sample_assign = None;
        self.trigger_probability_assign = None;
        self.drum_assign = None;
        self.drum_cell_tune = Some((slot, None));
        self.show_toast("Cell Tune: grid");
    }

    pub(super) fn handle_drum_assignment_grid_press(&mut self, x: usize, y: usize) {
        let Some((slot, voice)) = self.drum_assign else {
            return;
        };
        if x >= GRID_WIDTH || y >= GRID_HEIGHT {
            return;
        }
        if self.display.ui.combined_modifier_held {
            for row in 0..GRID_HEIGHT {
                self.assign_drum_cell(slot, voice, x, row);
            }
        } else if self.display.ui.shift_held {
            for col in 0..GRID_WIDTH {
                self.assign_drum_cell(slot, voice, col, y);
            }
        } else {
            self.assign_drum_cell(slot, voice, x, y);
        }
        self.mark_config_dirty();
    }

    pub(super) fn assign_drum_cell(&mut self, slot: usize, voice: u8, x: usize, y: usize) {
        if x >= GRID_WIDTH || y >= GRID_HEIGHT || voice >= 8 {
            return;
        }
        let Some(instrument) = self
            .instruments
            .get_mut(slot)
            .filter(|instrument| instrument.kind == "drum")
        else {
            return;
        };
        let Some(assignments) = instrument
            .drum_config
            .get_mut("assignments")
            .and_then(Value::as_array_mut)
        else {
            return;
        };
        if let Some(index) = assignments.iter().position(|cell| {
            cell.get("x").and_then(Value::as_u64) == Some(x as u64)
                && cell.get("y").and_then(Value::as_u64) == Some(y as u64)
        }) {
            if assignments[index]["voice"] == voice {
                assignments.remove(index);
            } else {
                assignments[index] = json!({ "x": x, "y": y, "voice": voice });
            }
        } else {
            assignments.push(json!({ "x": x, "y": y, "voice": voice }));
        }
    }

    pub(super) fn select_drum_tune_cell(&mut self, x: usize, y: usize) {
        let Some((slot, _)) = self.drum_cell_tune else {
            return;
        };
        let assigned = self
            .instruments
            .get(slot)
            .and_then(|instrument| instrument.drum_config.get("assignments"))
            .and_then(Value::as_array)
            .is_some_and(|cells| {
                cells.iter().any(|cell| {
                    cell.get("x").and_then(Value::as_u64) == Some(x as u64)
                        && cell.get("y").and_then(Value::as_u64) == Some(y as u64)
                })
            });
        if assigned {
            self.drum_cell_tune = Some((slot, Some((x, y))));
            self.show_toast(format!("Tune: {x},{y}"));
        } else {
            self.show_toast("No drum here");
        }
    }

    pub(super) fn turn_drum_cell_tune(&mut self, delta: i8) {
        let Some((slot, Some((x, y)))) = self.drum_cell_tune else {
            return;
        };
        let Some(cells) = self
            .instruments
            .get_mut(slot)
            .and_then(|instrument| instrument.drum_config.get_mut("assignments"))
            .and_then(Value::as_array_mut)
        else {
            return;
        };
        let Some(cell) = cells
            .iter_mut()
            .find(|cell| cell["x"] == x && cell["y"] == y)
        else {
            return;
        };
        let current = cell.get("tuneSemis").and_then(Value::as_i64).unwrap_or(0);
        let next = (current + i64::from(delta)).clamp(-24, 24);
        if current != next {
            cell["tuneSemis"] = json!(next);
            self.mark_fast_autosave_dirty();
        }
        self.show_toast(format!("Cell Tune: {next:+}"));
    }

    pub(super) fn fast_play_drum_slot_key(&mut self, key: &str) -> bool {
        let Some(slot) = self
            .menu
            .value_for_key(key)
            .and_then(|value| {
                value
                    .strip_prefix('I')
                    .and_then(|value| value.split_once(':'))
                    .and_then(|(number, _)| number.parse::<usize>().ok())
            })
            .and_then(|number| number.checked_sub(1))
        else {
            return false;
        };
        if self
            .instruments
            .get(slot)
            .is_none_or(|instrument| instrument.kind != "drum")
        {
            return false;
        }
        self.play_drum_selected_slot = Some(slot);
        true
    }

    pub(super) fn play_drum_cell(&mut self, x: usize, y: usize) {
        if self.display.ui.shift_held
            || self.display.ui.fn_held
            || self.display.ui.combined_modifier_held
        {
            return;
        }
        let slot = self.play_drum_selected_slot.or_else(|| {
            self.instruments
                .iter()
                .position(|instrument| instrument.kind == "drum")
        });
        let Some(slot) = slot else {
            return;
        };
        let intent = CellTriggerIntent {
            x,
            y,
            degree: 0,
            kind: CellTriggerKind::Activate,
        };
        let Some(hit) = super::modulation_sampler::drum_hit_for_intent(
            &self.instruments,
            slot as u8,
            &intent,
            100,
        ) else {
            return;
        };
        self.pending.drum_hits.push(hit);
        let now = self.display.transients.now();
        self.display.transients.trigger_event_dot(now);
    }

    pub(super) fn preview_drum_voice(&mut self, slot: usize, voice: usize) {
        if voice >= 8
            || self
                .instruments
                .get(slot)
                .is_none_or(|instrument| instrument.kind != "drum")
        {
            return;
        }
        self.pending.drum_hits.push(DrumHit {
            instrument_slot: slot as u8,
            voice: voice as u8,
            tune_semis: 0,
            velocity: 100,
        });
    }
}
