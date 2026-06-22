use crate::protocol::{RuntimeAudioCommand, RuntimePlatformEffect};

use super::{
    dance_fx_cell_id, dance_fx_params, dance_fx_target_key, dance_fx_type, momentary_fx_target,
    touch_pan_pos_from_grid_x, NativeDanceFxAssignment, NativeRunner, NativeToast, NativeXyTouch,
    GRID_HEIGHT, GRID_WIDTH, TOUCH_FX_MAX_CONCURRENT,
};

impl NativeRunner {
    fn dance_fx_start_effect_for_assignment(
        &self,
        assignment: &NativeDanceFxAssignment,
    ) -> Option<RuntimePlatformEffect> {
        let x = assignment.x;
        let y = assignment.y;
        let fx_type = dance_fx_type(&assignment.config).to_string();
        if fx_type == "none" {
            return None;
        }
        Some(RuntimePlatformEffect::AudioCommand {
            command: RuntimeAudioCommand::MomentaryFxStart {
                id: dance_fx_cell_id(x, y),
                fx_type,
                params: dance_fx_params(&assignment.config),
                target: momentary_fx_target(dance_fx_target_key(&assignment.config)),
            },
        })
    }

    pub(super) fn dance_fx_press_effects(
        &mut self,
        x: usize,
        y: usize,
    ) -> Vec<RuntimePlatformEffect> {
        let Some(assignment) = self.dance_fx_assignment_at(x, y).cloned() else {
            return Vec::new();
        };
        let fx_type = dance_fx_type(&assignment.config).to_string();
        if fx_type == "none" {
            return Vec::new();
        }
        let id = dance_fx_cell_id(x, y);
        if self
            .active_dance_fx
            .iter()
            .any(|(active_id, _)| active_id == &id)
        {
            return Vec::new();
        }
        let mut effects = Vec::new();
        if let Some(index) = self
            .active_dance_fx
            .iter()
            .position(|(_, active_type)| active_type == &fx_type)
        {
            let (old_id, _) = self.active_dance_fx.remove(index);
            effects.push(RuntimePlatformEffect::AudioCommand {
                command: RuntimeAudioCommand::MomentaryFxStop { id: old_id },
            });
        } else if self.active_dance_fx.len() >= TOUCH_FX_MAX_CONCURRENT {
            return Vec::new();
        }
        if let Some(start) = self.dance_fx_start_effect_for_assignment(&assignment) {
            self.active_dance_fx.push((id, fx_type));
            effects.push(start);
        }
        effects
    }

    pub(super) fn dance_fx_release_effects(
        &mut self,
        x: usize,
        y: usize,
    ) -> Vec<RuntimePlatformEffect> {
        let id = dance_fx_cell_id(x, y);
        let Some(index) = self
            .active_dance_fx
            .iter()
            .position(|(active_id, _)| active_id == &id)
        else {
            return Vec::new();
        };
        let (id, _) = self.active_dance_fx.remove(index);
        vec![RuntimePlatformEffect::AudioCommand {
            command: RuntimeAudioCommand::MomentaryFxStop { id },
        }]
    }

    fn dance_fx_assignment_at(&self, x: usize, y: usize) -> Option<&NativeDanceFxAssignment> {
        self.dance_fx_assignments
            .iter()
            .find(|assignment| assignment.x == x && assignment.y == y)
    }

    pub(super) fn handle_dance_fx_assignment_grid_press(&mut self, x: usize, y: usize) {
        let Some(config) = self.dance_fx_assign.take() else {
            return;
        };
        let same_existing = self.dance_fx_assignments.iter().any(|assignment| {
            assignment.x == x && assignment.y == y && assignment.config == config
        });
        self.dance_fx_assignments.retain(|assignment| {
            assignment.x != x || assignment.y != y || assignment.config != config
        });
        if same_existing {
            self.config_dirty = true;
            self.toast = Some(NativeToast {
                message: "FX cleared".into(),
                offset: 0,
            });
            return;
        }
        self.dance_fx_assignments
            .retain(|assignment| assignment.x != x || assignment.y != y);
        if dance_fx_type(&config) != "none" {
            self.dance_fx_assignments
                .push(NativeDanceFxAssignment { x, y, config });
        }
        self.config_dirty = true;
        self.toast = Some(NativeToast {
            message: "FX mapped".into(),
            offset: 0,
        });
    }

    pub(super) fn handle_dance_grid_press(&mut self, x: usize, y: usize) {
        match self.active_dance_mode.as_str() {
            "mix" => {
                if let Some(instrument) = self.instruments.get_mut(x) {
                    if instrument.kind != "none" {
                        let volume = ((y as f32 / (GRID_HEIGHT - 1) as f32) * 100.0).round() as u8;
                        if instrument.volume != volume {
                            instrument.volume = volume;
                            self.config_dirty = true;
                        }
                    }
                }
            }
            "pan" => {
                if let Some(instrument) = self.instruments.get_mut(y) {
                    if instrument.kind != "none" {
                        let pan_pos = touch_pan_pos_from_grid_x(x);
                        if let Some(bus_index) = instrument
                            .route
                            .strip_prefix("fx_bus_")
                            .and_then(|value| value.parse::<usize>().ok())
                            .and_then(|value| value.checked_sub(1))
                        {
                            if let Some(bus) = self.fx_buses.get_mut(bus_index) {
                                if bus.pan_pos != pan_pos {
                                    bus.pan_pos = pan_pos;
                                    self.config_dirty = true;
                                }
                            }
                        }
                        if instrument.pan_pos != pan_pos {
                            instrument.pan_pos = pan_pos;
                            self.config_dirty = true;
                        }
                    }
                }
            }
            "xy" => self.handle_dance_xy_press(x, y),
            _ => {}
        }
        self.menu.rebuild(self.menu_config());
    }

    pub(super) fn handle_dance_xy_press(&mut self, x: usize, y: usize) {
        let mut x_value = x.min(GRID_WIDTH - 1) as f32 / (GRID_WIDTH - 1) as f32;
        let mut y_value = y.min(GRID_HEIGHT - 1) as f32 / (GRID_HEIGHT - 1) as f32;
        if self.xy_invert_x {
            x_value = 1.0 - x_value;
        }
        if self.xy_invert_y {
            y_value = 1.0 - y_value;
        }
        self.xy_touch = NativeXyTouch {
            x: x_value,
            y: y_value,
            display_x: x.min(GRID_WIDTH - 1) as f32 / (GRID_WIDTH - 1) as f32,
            display_y: y.min(GRID_HEIGHT - 1) as f32 / (GRID_HEIGHT - 1) as f32,
            active: true,
        };
        self.config_dirty = true;
    }

    pub(super) fn handle_dance_xy_release(&mut self) {
        if self.xy_release == "reset-center" {
            self.xy_touch = NativeXyTouch {
                x: 0.5,
                y: 0.5,
                display_x: 0.5,
                display_y: 0.5,
                active: false,
            };
        } else {
            self.xy_touch.active = false;
        }
        self.config_dirty = true;
    }

    pub(super) fn handle_trigger_gate_grid_press(&mut self, x: usize, y: usize) {
        let mode = trigger_gate_mode_for_column(x);
        let Some(mode) = mode else {
            return;
        };
        if x == 6 && y == 0 {
            self.apply_trigger_gate_mode_to_all_parts(mode);
            return;
        }
        self.apply_trigger_gate_mode_to_part(y, mode);
    }

    pub(super) fn select_active_part(&mut self, index: usize) -> Result<(), String> {
        let index = index.min(GRID_HEIGHT.saturating_sub(1));
        if index == self.active_part_index {
            return Ok(());
        }
        self.store_active_engine();
        self.active_part_index = index;
        self.algorithm_step_pulses = self
            .part_algorithm_step_pulses
            .get(index)
            .copied()
            .unwrap_or(super::DEFAULT_ALGORITHM_STEP_PULSES);
        self.activate_engine(index)?;
        self.menu.rebuild(self.menu_config());
        self.show_toast(self.active_part_context_toast(index));
        Ok(())
    }

    pub(super) fn toggle_active_part_trigger_gate(&mut self) {
        let current = self
            .sense_parts
            .get(self.active_part_index)
            .map(|part| part.trigger_probability_mode.clone())
            .or_else(|| self.trigger_gate_modes.get(self.active_part_index).cloned())
            .unwrap_or_else(|| "full".into());
        if current == "zero" {
            let restore = self
                .trigger_gate_restore_modes
                .get(self.active_part_index)
                .and_then(Clone::clone)
                .unwrap_or_else(|| "full".into());
            if let Some(mode) = self.trigger_gate_modes.get_mut(self.active_part_index) {
                *mode = restore.clone();
            }
            if let Some(part) = self.sense_parts.get_mut(self.active_part_index) {
                part.trigger_probability_mode = restore.clone();
            }
            if let Some(slot) = self
                .trigger_gate_restore_modes
                .get_mut(self.active_part_index)
            {
                *slot = None;
            }
            self.toast = Some(NativeToast {
                message: format!("P{} triggers {}", self.active_part_index + 1, restore),
                offset: 0,
            });
        } else {
            if let Some(slot) = self
                .trigger_gate_restore_modes
                .get_mut(self.active_part_index)
            {
                *slot = Some(current);
            }
            if let Some(mode) = self.trigger_gate_modes.get_mut(self.active_part_index) {
                *mode = "zero".into();
            }
            if let Some(part) = self.sense_parts.get_mut(self.active_part_index) {
                part.trigger_probability_mode = "zero".into();
            }
            self.toast = Some(NativeToast {
                message: format!("P{} triggers off", self.active_part_index + 1),
                offset: 0,
            });
        }
        self.menu.rebuild(self.menu_config());
    }

    pub(super) fn select_dance_page_from_fn_grid(&mut self, y: usize) {
        let next_mode = match y {
            0 => Some("mix"),
            1 => Some("pan"),
            2 => Some("fx"),
            3 => Some("trigger-gate"),
            4 => Some("xy"),
            _ => None,
        };
        let Some(next_mode) = next_mode else {
            return;
        };
        self.dance_mode = next_mode.into();
        self.active_dance_mode = self.dance_mode.clone();
        self.menu.state.stack = vec![3];
        self.menu.state.cursor = 0;
        self.menu.state.editing = false;
        self.menu.rebuild(self.menu_config());
        self.show_toast(format!("Dance: {}", dance_mode_label(next_mode)));
    }

    fn active_part_context_toast(&self, index: usize) -> String {
        let label = self
            .part_labels()
            .get(index)
            .cloned()
            .unwrap_or_else(|| format!("P{}", index + 1));
        format!("Part: {}", label.replace(": ", " "))
    }

    fn apply_trigger_gate_mode_to_all_parts(&mut self, mode: &str) {
        for part_mode in &mut self.trigger_gate_modes {
            *part_mode = mode.into();
        }
        for part in &mut self.sense_parts {
            part.trigger_probability_mode = mode.into();
        }
        for restore in &mut self.trigger_gate_restore_modes {
            *restore = None;
        }
        self.activate_trigger_gate_part_with_toast(self.active_part_index);
    }

    fn apply_trigger_gate_mode_to_part(&mut self, part_index: usize, mode: &str) {
        if let Some(part_mode) = self.trigger_gate_modes.get_mut(part_index) {
            *part_mode = mode.into();
        }
        if let Some(part) = self.sense_parts.get_mut(part_index) {
            part.trigger_probability_mode = mode.into();
        }
        if let Some(restore) = self.trigger_gate_restore_modes.get_mut(part_index) {
            *restore = None;
        }
        if part_index == self.active_part_index {
            self.activate_trigger_gate_part_with_toast(part_index);
        }
    }

    fn activate_trigger_gate_part_with_toast(&mut self, part_index: usize) {
        if let Err(error) = self.activate_engine(part_index) {
            self.toast = Some(NativeToast {
                message: error,
                offset: 0,
            });
        }
    }
}

fn dance_mode_label(mode: &str) -> &str {
    match mode {
        "trigger-gate" => "trig",
        other => other,
    }
}

fn trigger_gate_mode_for_column(x: usize) -> Option<&'static str> {
    match x {
        0 => Some("zero"),
        1 => Some("custom"),
        2 => Some("full"),
        6 => Some("custom"),
        7 => Some("full"),
        _ => None,
    }
}
