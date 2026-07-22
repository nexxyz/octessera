use super::*;
use crate::native_menu::section_labels::{BUILD_LABEL, LINK_LABEL, PLAY_LABEL, SHAPE_LABEL};

impl NativeRunner {
    pub(super) fn handle_sample_assignment_grid_press(&mut self, x: usize, y: usize) {
        let Some((instrument_slot, sample_slot)) = self.sample_assign else {
            return;
        };
        let mut points = Vec::new();
        if self.display.ui.combined_modifier_held {
            for row in 0..GRID_HEIGHT {
                points.push((x, row));
            }
        } else if self.display.ui.shift_held {
            for col in 0..GRID_WIDTH {
                points.push((col, y));
            }
        } else {
            points.push((x, y));
        }
        for (px, py) in points {
            self.assign_sample_cell(instrument_slot, sample_slot, px, py);
        }
        self.mark_config_dirty();
    }

    pub(super) fn assign_sample_cell(
        &mut self,
        instrument_slot: usize,
        sample_slot: usize,
        x: usize,
        y: usize,
    ) {
        let Some(instrument) = self.instruments.get_mut(instrument_slot) else {
            return;
        };
        if x >= GRID_WIDTH || y >= GRID_HEIGHT {
            return;
        }
        if let Some(index) = instrument
            .sample_assignments
            .iter()
            .position(|assignment| assignment.x == x && assignment.y == y)
        {
            if instrument.sample_assignments[index].sample_slot == sample_slot {
                if instrument.sample_velocity_levels_enabled {
                    instrument.sample_assignments[index].level =
                        match instrument.sample_assignments[index].level.as_deref() {
                            Some("high") => Some("medium".into()),
                            Some("medium") => Some("low".into()),
                            _ => {
                                let _ = instrument.sample_assignments.remove(index);
                                return;
                            }
                        };
                    return;
                }
                let _ = instrument.sample_assignments.remove(index);
                return;
            }
            instrument.sample_assignments[index].sample_slot = sample_slot;
            instrument.sample_assignments[index].level =
                if instrument.sample_velocity_levels_enabled {
                    Some("high".into())
                } else {
                    None
                };
            return;
        }
        instrument.sample_assignments.push(NativeSampleAssignment {
            x,
            y,
            sample_slot,
            level: if instrument.sample_velocity_levels_enabled {
                Some("high".into())
            } else {
                None
            },
        });
    }

    pub(super) fn handle_trigger_probability_grid_press(&mut self, x: usize, y: usize) {
        let Some(layer_index) = self.trigger_probability_assign else {
            return;
        };
        if x >= GRID_WIDTH || y >= GRID_HEIGHT {
            return;
        }
        let next = self.next_probability_state(layer_index, x, y);
        if self.display.ui.combined_modifier_held {
            for row in 0..GRID_HEIGHT {
                self.set_probability_cell(layer_index, x, row, &next);
            }
        } else if self.display.ui.shift_held {
            for column in 0..GRID_WIDTH {
                self.set_probability_cell(layer_index, column, y, &next);
            }
        } else {
            self.set_probability_cell(layer_index, x, y, &next);
        }
        self.mark_config_dirty();
    }

    pub(super) fn next_probability_state(&self, layer_index: usize, x: usize, y: usize) -> String {
        let current = self
            .trigger_probability_maps
            .get(layer_index)
            .and_then(|map| map.get(y * GRID_WIDTH + x))
            .map(String::as_str)
            .unwrap_or("zero");
        match current {
            "zero" => "low",
            "low" => "high",
            "high" => "full",
            _ => "zero",
        }
        .into()
    }

    pub(super) fn set_probability_cell(
        &mut self,
        layer_index: usize,
        x: usize,
        y: usize,
        value: &str,
    ) {
        let Some(map) = self.trigger_probability_maps.get_mut(layer_index) else {
            return;
        };
        if let Some(cell) = map.get_mut(y * GRID_WIDTH + x) {
            *cell = value.into();
        }
    }

    pub(super) fn enter_root_group(&mut self, label: Option<&str>) {
        match label {
            Some(PLAY_LABEL) => {
                self.active_sparks_mode = self.sparks_mode.clone();
            }
            Some(BUILD_LABEL) => {
                self.menu.state.cursor = self.active_layer_index.min(GRID_HEIGHT.saturating_sub(1));
                self.active_sparks_mode = "none".into();
            }
            Some(LINK_LABEL) => {
                self.menu.state.cursor = (self.active_layer_index + 4).min(GRID_HEIGHT + 3);
                self.active_sparks_mode = "none".into();
            }
            Some(SHAPE_LABEL) | Some("System") => {
                self.active_sparks_mode = "none".into();
            }
            _ => {}
        }
    }

    pub(super) fn enter_nested_group(
        &mut self,
        stack_depth_before: usize,
        label: Option<&str>,
    ) -> Result<(), String> {
        if stack_depth_before == 1 {
            if let Some(label) = label {
                if let Some(mode) = sparks_mode_from_page_label(label) {
                    self.sparks_mode = mode.into();
                    self.active_sparks_mode = self.sparks_mode.clone();
                    self.mark_config_dirty();
                    return Ok(());
                }
                if let Some(layer) = label
                    .strip_prefix('L')
                    .and_then(|rest| rest.split(':').next())
                {
                    if let Ok(index) = layer.parse::<usize>() {
                        let previous_index = self.active_layer_index;
                        self.select_active_layer(index.saturating_sub(1))?;
                        self.update_layer_worlds_menu_items(previous_index);
                        self.update_layer_worlds_menu_items(self.active_layer_index);
                        self.update_active_behavior_selector_label();
                    }
                }
            }
        }
        Ok(())
    }
}

fn sparks_mode_from_page_label(label: &str) -> Option<&'static str> {
    match label {
        "Mix" => Some("mix"),
        "Pan" => Some("pan"),
        "FX" => Some("fx"),
        "Trigger Gate" => Some("trigger-gate"),
        "Transpose" => Some("transpose"),
        "XY" => Some("xy"),
        _ => None,
    }
}
