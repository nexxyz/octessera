use super::{NativeCellStamp, NativeRunner, NativeSampleAssignment, GRID_HEIGHT, GRID_WIDTH};
use serde_json::{json, Value};

impl NativeRunner {
    pub(super) fn fast_cell_stamp_key(&mut self, slot: usize, key: &str, field: &str) -> bool {
        let number = self.menu.number_for_key(key);
        let level = self.menu.value_for_key(key);
        let Some(stamp) = self
            .instruments
            .get_mut(slot)
            .map(|instrument| &mut instrument.cell_stamp)
        else {
            return false;
        };
        let before = stamp.clone();
        match field {
            "cell.tuneSemis" => {
                if let Some(value) = number {
                    stamp.tune_semis = value.clamp(-24, 24) as i8;
                }
            }
            "cell.tuneCents" => {
                if let Some(value) = number {
                    stamp.tune_cents = value.clamp(-100, 100) as i8;
                }
            }
            "cell.level" => {
                stamp.level = match level.as_deref() {
                    Some("High") => Some("high".into()),
                    Some("Medium") => Some("medium".into()),
                    Some("Low") => Some("low".into()),
                    _ => None,
                };
            }
            _ => return false,
        }
        if *stamp != before {
            self.mark_fast_autosave_dirty();
        }
        true
    }

    pub(super) fn fast_sample_follow_key(&mut self, slot: usize, key: &str, field: &str) -> bool {
        let Some(follow) = self.menu.value_for_key(key).map(|value| value == "true") else {
            return false;
        };
        let Some(instrument) = self.instruments.get_mut(slot) else {
            return false;
        };
        let target = if field == "sample.followPitch" {
            &mut instrument.sample_follow_pitch
        } else {
            &mut instrument.sample_follow_transpose
        };
        if *target != follow {
            *target = follow;
            self.mark_fast_autosave_dirty();
        }
        true
    }

    pub(super) fn cell_stamp(&self, slot: usize) -> NativeCellStamp {
        self.instruments
            .get(slot)
            .map(|instrument| instrument.cell_stamp.clone())
            .unwrap_or_default()
    }

    pub(super) fn handle_sample_assignment_grid_press(&mut self, x: usize, y: usize) {
        let Some((slot, sample_slot)) = self.sample_assign else {
            return;
        };
        let Some(levels_enabled) = self
            .instruments
            .get(slot)
            .map(|instrument| instrument.sample_velocity_levels_enabled)
        else {
            return;
        };
        let stamp = self.cell_stamp(slot);
        let level = levels_enabled.then(|| stamp.level.clone()).flatten();
        let cells = self.stamp_line_cells(x, y);
        let Some(instrument) = self.instruments.get_mut(slot) else {
            return;
        };
        let stamped = |x: usize, y: usize| NativeSampleAssignment {
            x,
            y,
            sample_slot,
            level: level.clone(),
            tune_semis: stamp.tune_semis,
            tune_cents: stamp.tune_cents,
        };
        let clear = cells
            .iter()
            .all(|&(x, y)| instrument.sample_assignments.contains(&stamped(x, y)));
        instrument
            .sample_assignments
            .retain(|assignment| !cells.contains(&(assignment.x, assignment.y)));
        if !clear {
            instrument
                .sample_assignments
                .extend(cells.iter().map(|&(x, y)| stamped(x, y)));
        }
        self.show_stamp_toast(clear, 'S', sample_slot, &stamp, level.as_deref());
        self.mark_config_dirty();
    }

    pub(super) fn handle_drum_assignment_grid_press(&mut self, x: usize, y: usize) {
        let Some((slot, voice)) = self.drum_assign else {
            return;
        };
        let stamp = self.cell_stamp(slot);
        let cells = self.stamp_line_cells(x, y);
        let Some(assignments) = self
            .instruments
            .get_mut(slot)
            .filter(|instrument| instrument.kind == "drum")
            .and_then(|instrument| instrument.drum_config.get_mut("assignments"))
            .and_then(Value::as_array_mut)
        else {
            return;
        };
        let stamped = |x: usize, y: usize| {
            let mut cell = json!({ "x": x, "y": y, "voice": voice });
            if stamp.tune_semis != 0 {
                cell["tuneSemis"] = stamp.tune_semis.into();
            }
            if stamp.tune_cents != 0 {
                cell["tuneCents"] = stamp.tune_cents.into();
            }
            if let Some(level) = &stamp.level {
                cell["level"] = level.as_str().into();
            }
            cell
        };
        let offset = |cell: &Value, key: &str| cell.get(key).and_then(Value::as_i64).unwrap_or(0);
        let matches = |cell: &Value, x: usize, y: usize| {
            cell.get("x").and_then(Value::as_u64) == Some(x as u64)
                && cell.get("y").and_then(Value::as_u64) == Some(y as u64)
                && cell.get("voice").and_then(Value::as_u64) == Some(u64::from(voice))
                && offset(cell, "tuneSemis") == i64::from(stamp.tune_semis)
                && offset(cell, "tuneCents") == i64::from(stamp.tune_cents)
                && cell.get("level").and_then(Value::as_str) == stamp.level.as_deref()
        };
        let clear = cells
            .iter()
            .all(|&(x, y)| assignments.iter().any(|cell| matches(cell, x, y)));
        assignments.retain(|cell| {
            let at = |key: &str| cell.get(key).and_then(Value::as_u64).map(|v| v as usize);
            !matches!((at("x"), at("y")), (Some(x), Some(y)) if cells.contains(&(x, y)))
        });
        if !clear {
            assignments.extend(cells.iter().map(|&(x, y)| stamped(x, y)));
        }
        self.show_stamp_toast(
            clear,
            'V',
            usize::from(voice),
            &stamp,
            stamp.level.as_deref(),
        );
        self.mark_config_dirty();
    }

    fn stamp_line_cells(&self, x: usize, y: usize) -> Vec<(usize, usize)> {
        if x >= GRID_WIDTH || y >= GRID_HEIGHT {
            Vec::new()
        } else if self.display.ui.combined_modifier_held {
            (0..GRID_HEIGHT).map(|row| (x, row)).collect()
        } else if self.display.ui.shift_held {
            (0..GRID_WIDTH).map(|column| (column, y)).collect()
        } else {
            vec![(x, y)]
        }
    }

    fn show_stamp_toast(
        &mut self,
        cleared: bool,
        prefix: char,
        index: usize,
        stamp: &NativeCellStamp,
        level: Option<&str>,
    ) {
        if cleared {
            self.show_toast("Cleared");
            return;
        }
        let level = match level {
            Some("high") => "High",
            Some("medium") => "Med",
            Some("low") => "Low",
            _ => "Base",
        };
        self.show_toast(format!(
            "{prefix}{} {:+} {:+}c {level}",
            index + 1,
            stamp.tune_semis,
            stamp.tune_cents
        ));
    }
}
