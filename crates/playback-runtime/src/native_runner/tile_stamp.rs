use super::{NativeRunner, NativeSampleAssignment, NativeTileStamp, GRID_HEIGHT, GRID_WIDTH};
use serde_json::{json, Value};

impl NativeRunner {
    pub(super) fn fast_tile_stamp_key(&mut self, slot: usize, key: &str, field: &str) -> bool {
        let number = self.menu.number_for_key(key);
        let level = self.menu.value_for_key(key);
        let Some(stamp) = self
            .instruments
            .get_mut(slot)
            .map(|instrument| &mut instrument.tile_stamp)
        else {
            return false;
        };
        let before = stamp.clone();
        match field {
            "tile.tuneSemis" => {
                if let Some(value) = number {
                    stamp.tune_semis = value.clamp(-24, 24) as i8;
                }
            }
            "tile.tuneCents" => {
                if let Some(value) = number {
                    stamp.tune_cents = value.clamp(-100, 100) as i8;
                }
            }
            "tile.level" => {
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

    pub(super) fn tile_stamp(&self, slot: usize) -> NativeTileStamp {
        self.instruments
            .get(slot)
            .map(|instrument| instrument.tile_stamp.clone())
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
        let stamp = self.tile_stamp(slot);
        let level = levels_enabled.then(|| stamp.level.clone()).flatten();
        let (tiles, line) = self.stamp_line_tiles(x, y);
        let Some(instrument) = self.instruments.get_mut(slot).filter(|_| !tiles.is_empty()) else {
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
        let clear = tiles
            .iter()
            .all(|&(x, y)| instrument.sample_assignments.contains(&stamped(x, y)));
        instrument
            .sample_assignments
            .retain(|assignment| !tiles.contains(&(assignment.x, assignment.y)));
        if !clear {
            instrument
                .sample_assignments
                .extend(tiles.iter().map(|&(x, y)| stamped(x, y)));
        }
        self.show_stamp_toast(clear, line, 'S', sample_slot, &stamp, level.as_deref());
        self.mark_config_dirty();
    }

    pub(super) fn handle_drum_assignment_grid_press(&mut self, x: usize, y: usize) {
        let Some((slot, voice)) = self.drum_assign else {
            return;
        };
        let stamp = self.tile_stamp(slot);
        let (tiles, line) = self.stamp_line_tiles(x, y);
        let Some(assignments) = self
            .instruments
            .get_mut(slot)
            .filter(|instrument| instrument.kind == "drum" && !tiles.is_empty())
            .and_then(|instrument| instrument.drum_config.get_mut("assignments"))
            .and_then(Value::as_array_mut)
        else {
            return;
        };
        let stamped = |x: usize, y: usize| {
            let mut tile = json!({ "x": x, "y": y, "voice": voice });
            if stamp.tune_semis != 0 {
                tile["tuneSemis"] = stamp.tune_semis.into();
            }
            if stamp.tune_cents != 0 {
                tile["tuneCents"] = stamp.tune_cents.into();
            }
            if let Some(level) = &stamp.level {
                tile["level"] = level.as_str().into();
            }
            tile
        };
        let offset = |tile: &Value, key: &str| tile.get(key).and_then(Value::as_i64).unwrap_or(0);
        let matches = |tile: &Value, x: usize, y: usize| {
            tile.get("x").and_then(Value::as_u64) == Some(x as u64)
                && tile.get("y").and_then(Value::as_u64) == Some(y as u64)
                && tile.get("voice").and_then(Value::as_u64) == Some(u64::from(voice))
                && offset(tile, "tuneSemis") == i64::from(stamp.tune_semis)
                && offset(tile, "tuneCents") == i64::from(stamp.tune_cents)
                && tile.get("level").and_then(Value::as_str) == stamp.level.as_deref()
        };
        let clear = tiles
            .iter()
            .all(|&(x, y)| assignments.iter().any(|tile| matches(tile, x, y)));
        assignments.retain(|tile| {
            let at = |key: &str| tile.get(key).and_then(Value::as_u64).map(|v| v as usize);
            !matches!((at("x"), at("y")), (Some(x), Some(y)) if tiles.contains(&(x, y)))
        });
        if !clear {
            assignments.extend(tiles.iter().map(|&(x, y)| stamped(x, y)));
        }
        self.show_stamp_toast(
            clear,
            line,
            'V',
            usize::from(voice),
            &stamp,
            stamp.level.as_deref(),
        );
        self.mark_config_dirty();
    }

    /// The tiles a press stamps and, for a whole row or column, that line's toast word.
    fn stamp_line_tiles(&self, x: usize, y: usize) -> (Vec<(usize, usize)>, Option<&'static str>) {
        if x >= GRID_WIDTH || y >= GRID_HEIGHT {
            (Vec::new(), None)
        } else if self.display.ui.combined_modifier_held {
            ((0..GRID_HEIGHT).map(|row| (x, row)).collect(), Some("col"))
        } else if self.display.ui.shift_held {
            (
                (0..GRID_WIDTH).map(|column| (column, y)).collect(),
                Some("row"),
            )
        } else {
            (vec![(x, y)], None)
        }
    }

    /// A line toast names the line instead of the slot, so `row -24 +100c Base` still fits.
    fn show_stamp_toast(
        &mut self,
        cleared: bool,
        line: Option<&str>,
        prefix: char,
        index: usize,
        stamp: &NativeTileStamp,
        level: Option<&str>,
    ) {
        if cleared {
            self.show_toast(
                line.map_or_else(|| "Cleared".into(), |line| format!("Cleared {line}")),
            );
            return;
        }
        let level = match level {
            Some("high") => "High",
            Some("medium") => "Med",
            Some("low") => "Low",
            _ => "Base",
        };
        let target = line.map_or_else(|| format!("{prefix}{}", index + 1), str::to_owned);
        self.show_toast(format!(
            "{target} {:+} {:+}c {level}",
            stamp.tune_semis, stamp.tune_cents
        ));
    }
}
