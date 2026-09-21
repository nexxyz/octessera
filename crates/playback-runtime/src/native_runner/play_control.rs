use crate::protocol::{RuntimeAudioCommand, RuntimePlatformEffect};

use super::pan_mapping::touch_pan_pos_from_grid_x;
use super::play_fx_config::{play_fx_params, play_fx_target_key, play_fx_type};
use super::{
    momentary_fx_target, play_fx_cell_id, trigger_gate_mode_for_column, NativePlayFxAssignment,
    NativeRunner, NativeToast, GRID_HEIGHT, GRID_WIDTH, PLAY_FX_MAX_CONCURRENT,
};

impl NativeRunner {
    fn play_fx_start_effect_for_assignment(
        &self,
        assignment: &NativePlayFxAssignment,
    ) -> Option<RuntimePlatformEffect> {
        let x = assignment.x;
        let y = assignment.y;
        let fx_type = play_fx_type(&assignment.config).to_string();
        if fx_type == "none" {
            return None;
        }
        Some(RuntimePlatformEffect::AudioCommand {
            command: RuntimeAudioCommand::MomentaryFxStart {
                id: play_fx_cell_id(x, y),
                epoch: 0,
                fx_type,
                params: play_fx_params(&assignment.config),
                target: momentary_fx_target(play_fx_target_key(&assignment.config)),
            },
        })
    }

    pub(super) fn play_fx_press_effects(
        &mut self,
        x: usize,
        y: usize,
    ) -> Vec<RuntimePlatformEffect> {
        let Some(assignment) = self.play_fx_assignment_at(x, y).cloned() else {
            return Vec::new();
        };
        let fx_type = play_fx_type(&assignment.config).to_string();
        if fx_type == "none" {
            return Vec::new();
        }
        let id = play_fx_cell_id(x, y);
        if self
            .active_play_fx
            .iter()
            .any(|(active_id, _)| active_id == &id)
        {
            return Vec::new();
        }
        let mut effects = Vec::new();
        if self
            .active_play_fx
            .iter()
            .any(|(_, active_type)| active_type == &fx_type)
        {
            self.show_toast(format!("Momentary FX type active ({fx_type})"));
            return Vec::new();
        } else if self.active_play_fx.len() >= PLAY_FX_MAX_CONCURRENT {
            self.show_toast(format!(
                "Momentary FX limit reached ({PLAY_FX_MAX_CONCURRENT})"
            ));
            return Vec::new();
        }
        if let Some(start) = self.play_fx_start_effect_for_assignment(&assignment) {
            self.active_play_fx.push((id, fx_type));
            effects.push(start);
        }
        effects
    }

    pub(super) fn play_fx_release_effects(
        &mut self,
        x: usize,
        y: usize,
    ) -> Vec<RuntimePlatformEffect> {
        let id = play_fx_cell_id(x, y);
        let Some(index) = self
            .active_play_fx
            .iter()
            .position(|(active_id, _)| active_id == &id)
        else {
            return Vec::new();
        };
        let (id, _) = self.active_play_fx.remove(index);
        vec![RuntimePlatformEffect::AudioCommand {
            command: RuntimeAudioCommand::MomentaryFxStop { id, epoch: 0 },
        }]
    }

    fn play_fx_assignment_at(&self, x: usize, y: usize) -> Option<&NativePlayFxAssignment> {
        self.play_fx_assignments
            .iter()
            .find(|assignment| assignment.x == x && assignment.y == y)
    }

    pub(super) fn handle_play_fx_assignment_grid_press(&mut self, x: usize, y: usize) {
        let Some(config) = self.play_fx_assign.take() else {
            return;
        };
        let same_existing = self.play_fx_assignments.iter().any(|assignment| {
            assignment.x == x && assignment.y == y && assignment.config == config
        });
        self.play_fx_assignments.retain(|assignment| {
            assignment.x != x || assignment.y != y || assignment.config != config
        });
        if same_existing {
            self.mark_config_dirty();
            self.display.toast = Some(NativeToast {
                message: "FX cleared".into(),
                offset: 0,
            });
            return;
        }
        self.play_fx_assignments
            .retain(|assignment| assignment.x != x || assignment.y != y);
        if play_fx_type(&config) != "none" {
            self.play_fx_assignments
                .push(NativePlayFxAssignment { x, y, config });
        }
        self.mark_config_dirty();
        self.display.toast = Some(NativeToast {
            message: "FX mapped".into(),
            offset: 0,
        });
    }

    pub(super) fn handle_play_grid_press(&mut self, x: usize, y: usize) {
        match self.active_play_mode.as_str() {
            "mix" => {
                if let Some(instrument) = self.instruments.get_mut(x) {
                    if instrument.kind != "none" {
                        let volume = ((y as f32 / (GRID_HEIGHT - 1) as f32) * 100.0).round() as u8;
                        if instrument.volume != volume {
                            instrument.volume = volume;
                            self.mark_config_dirty();
                            self.queue_audio_command(RuntimeAudioCommand::SetInstrumentMixer {
                                instrument_slot: x,
                                generation: 0,
                                volume_pct: Some(f32::from(volume)),
                                pan_pos: None,
                            });
                        }
                    }
                }
            }
            "pan" => {
                let pan_pos = touch_pan_pos_from_grid_x(x);
                let Some(instrument) = self.instruments.get_mut(y) else {
                    return;
                };
                if instrument.kind != "none" {
                    let bus_index = instrument
                        .route
                        .strip_prefix("fx_bus_")
                        .and_then(|value| value.parse::<usize>().ok())
                        .and_then(|value| value.checked_sub(1));
                    if instrument.pan_pos != pan_pos {
                        instrument.pan_pos = pan_pos;
                        self.mark_config_dirty();
                        self.queue_audio_command(RuntimeAudioCommand::SetInstrumentMixer {
                            instrument_slot: y,
                            generation: 0,
                            volume_pct: None,
                            pan_pos: Some(usize::from(pan_pos)),
                        });
                    }
                    if let Some(bus_index) = bus_index {
                        if let Some(bus) = self.fx_buses.get_mut(bus_index) {
                            if bus.pan_pos != pan_pos {
                                bus.pan_pos = pan_pos;
                                self.mark_config_dirty();
                                self.queue_audio_command(RuntimeAudioCommand::SetFxBusMixer {
                                    bus_index,
                                    generation: 0,
                                    pan_pos: Some(usize::from(pan_pos)),
                                    volume_pct: None,
                                });
                            }
                        }
                    }
                }
            }
            "xy" => self.handle_play_xy_press(x, y),
            _ => {}
        }
    }

    pub(super) fn handle_play_xy_press(&mut self, x: usize, y: usize) {
        self.handle_play_xy_press_at(x, y, std::time::Instant::now());
    }

    pub(super) fn handle_play_xy_press_at(&mut self, x: usize, y: usize, now: std::time::Instant) {
        let physical_x = x.min(GRID_WIDTH - 1) as f32 / (GRID_WIDTH - 1) as f32;
        let physical_y = y.min(GRID_HEIGHT - 1) as f32 / (GRID_HEIGHT - 1) as f32;
        self.xy_touch.display_x = physical_x;
        self.xy_touch.display_y = physical_y;
        self.xy_touch.active = true;
        if self.retarget_xy_runtime_sources_at(now, false) {
            if let Err(error) = self.process_dirty_modulation_step(true) {
                self.show_toast(format!("modulation composition unavailable: {error}"));
            }
        }
    }

    pub(super) fn handle_play_xy_release(&mut self) {
        self.handle_play_xy_release_at(std::time::Instant::now());
    }

    pub(super) fn handle_play_xy_release_at(&mut self, now: std::time::Instant) {
        if self.xy_release == "reset-center" {
            self.xy_touch.display_x = 0.5;
            self.xy_touch.display_y = 0.5;
            self.xy_touch.active = false;
            if self.retarget_xy_runtime_sources_at(now, true) {
                if let Err(error) = self.process_dirty_modulation_step(true) {
                    self.show_toast(format!("modulation composition unavailable: {error}"));
                }
            }
        } else {
            self.xy_touch.active = false;
        }
    }

    pub(super) fn handle_trigger_gate_grid_press(&mut self, x: usize, y: usize) {
        let mode = trigger_gate_mode_for_column(x);
        let Some(mode) = mode else {
            return;
        };
        if x == 6 && y == 0 {
            self.apply_trigger_gate_mode_to_all_layers(mode);
            return;
        }
        self.apply_trigger_gate_mode_to_layer(y, mode);
    }

    pub(super) fn select_active_layer(&mut self, index: usize) -> Result<(), String> {
        let index = index.min(GRID_HEIGHT.saturating_sub(1));
        if index == self.active_layer_index {
            return Ok(());
        }
        self.switch_active_engine(index)?;
        self.show_toast(self.active_layer_context_toast(index));
        Ok(())
    }

    pub(super) fn toggle_layer_trigger_gate(&mut self, index: usize) {
        let index = index.min(GRID_HEIGHT.saturating_sub(1));
        let current = self
            .link_layers
            .get(index)
            .map(|layer| layer.trigger_probability_mode.clone())
            .or_else(|| self.trigger_gate_modes.get(index).cloned())
            .unwrap_or_else(|| "full".into());
        if current == "zero" {
            let restore = self
                .trigger_gate_restore_modes
                .get(index)
                .and_then(Clone::clone)
                .unwrap_or_else(|| "full".into());
            if let Some(mode) = self.trigger_gate_modes.get_mut(index) {
                *mode = restore.clone();
            }
            if let Some(layer) = self.link_layers.get_mut(index) {
                layer.trigger_probability_mode = restore.clone();
            }
            if let Some(slot) = self.trigger_gate_restore_modes.get_mut(index) {
                *slot = None;
            }
            self.display.toast = Some(NativeToast {
                message: format!("L{} triggers {}", index + 1, restore),
                offset: 0,
            });
        } else {
            self.drain_layer_owned_notes(index);
            self.apply_trigger_gate_mode_to_layer(index, "zero");
            if let Some(slot) = self.trigger_gate_restore_modes.get_mut(index) {
                *slot = Some(current);
            }
            self.display.toast = Some(NativeToast {
                message: format!("L{} triggers off", index + 1),
                offset: 0,
            });
        }
        self.mark_config_dirty();
    }
}
