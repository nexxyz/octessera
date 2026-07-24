use crate::protocol::{RuntimeAudioCommand, RuntimePlatformEffect};

use super::modulation_source::ModulationSourceId;
use super::{
    momentary_fx_target, sparks_fx_cell_id, sparks_fx_params, sparks_fx_target_key, sparks_fx_type,
    touch_pan_pos_from_grid_x, trigger_gate_mode_for_column, NativeRunner,
    NativeSparksFxAssignment, NativeToast, NativeXyTouch, GRID_HEIGHT, GRID_WIDTH,
    SPARKS_FX_MAX_CONCURRENT,
};

impl NativeRunner {
    fn sparks_fx_start_effect_for_assignment(
        &self,
        assignment: &NativeSparksFxAssignment,
    ) -> Option<RuntimePlatformEffect> {
        let x = assignment.x;
        let y = assignment.y;
        let fx_type = sparks_fx_type(&assignment.config).to_string();
        if fx_type == "none" {
            return None;
        }
        Some(RuntimePlatformEffect::AudioCommand {
            command: RuntimeAudioCommand::MomentaryFxStart {
                id: sparks_fx_cell_id(x, y),
                fx_type,
                params: sparks_fx_params(&assignment.config),
                target: momentary_fx_target(sparks_fx_target_key(&assignment.config)),
            },
        })
    }

    pub(super) fn sparks_fx_press_effects(
        &mut self,
        x: usize,
        y: usize,
    ) -> Vec<RuntimePlatformEffect> {
        let Some(assignment) = self.sparks_fx_assignment_at(x, y).cloned() else {
            return Vec::new();
        };
        let fx_type = sparks_fx_type(&assignment.config).to_string();
        if fx_type == "none" {
            return Vec::new();
        }
        let id = sparks_fx_cell_id(x, y);
        if self
            .active_sparks_fx
            .iter()
            .any(|(active_id, _)| active_id == &id)
        {
            return Vec::new();
        }
        let mut effects = Vec::new();
        if self
            .active_sparks_fx
            .iter()
            .any(|(_, active_type)| active_type == &fx_type)
        {
            self.show_toast(format!("Momentary FX type active ({fx_type})"));
            return Vec::new();
        } else if self.active_sparks_fx.len() >= SPARKS_FX_MAX_CONCURRENT {
            self.show_toast(format!(
                "Momentary FX limit reached ({SPARKS_FX_MAX_CONCURRENT})"
            ));
            return Vec::new();
        }
        if let Some(start) = self.sparks_fx_start_effect_for_assignment(&assignment) {
            self.active_sparks_fx.push((id, fx_type));
            effects.push(start);
        }
        effects
    }

    pub(super) fn sparks_fx_release_effects(
        &mut self,
        x: usize,
        y: usize,
    ) -> Vec<RuntimePlatformEffect> {
        let id = sparks_fx_cell_id(x, y);
        let Some(index) = self
            .active_sparks_fx
            .iter()
            .position(|(active_id, _)| active_id == &id)
        else {
            return Vec::new();
        };
        let (id, _) = self.active_sparks_fx.remove(index);
        vec![RuntimePlatformEffect::AudioCommand {
            command: RuntimeAudioCommand::MomentaryFxStop { id },
        }]
    }

    fn sparks_fx_assignment_at(&self, x: usize, y: usize) -> Option<&NativeSparksFxAssignment> {
        self.sparks_fx_assignments
            .iter()
            .find(|assignment| assignment.x == x && assignment.y == y)
    }

    pub(super) fn handle_sparks_fx_assignment_grid_press(&mut self, x: usize, y: usize) {
        let Some(config) = self.sparks_fx_assign.take() else {
            return;
        };
        let same_existing = self.sparks_fx_assignments.iter().any(|assignment| {
            assignment.x == x && assignment.y == y && assignment.config == config
        });
        self.sparks_fx_assignments.retain(|assignment| {
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
        self.sparks_fx_assignments
            .retain(|assignment| assignment.x != x || assignment.y != y);
        if sparks_fx_type(&config) != "none" {
            self.sparks_fx_assignments
                .push(NativeSparksFxAssignment { x, y, config });
        }
        self.mark_config_dirty();
        self.display.toast = Some(NativeToast {
            message: "FX mapped".into(),
            offset: 0,
        });
    }

    pub(super) fn handle_sparks_grid_press(&mut self, x: usize, y: usize) {
        match self.active_sparks_mode.as_str() {
            "mix" => {
                if let Some(instrument) = self.instruments.get_mut(x) {
                    if instrument.kind != "none" {
                        let volume = ((y as f32 / (GRID_HEIGHT - 1) as f32) * 100.0).round() as u8;
                        if instrument.volume != volume {
                            instrument.volume = volume;
                            self.mark_config_dirty();
                            self.queue_audio_command(RuntimeAudioCommand::SetInstrumentMixer {
                                instrument_slot: x,
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
                                    pan_pos: Some(usize::from(pan_pos)),
                                    volume_pct: None,
                                });
                            }
                        }
                    }
                }
            }
            "xy" => self.handle_sparks_xy_press(x, y),
            _ => {}
        }
    }

    pub(super) fn handle_sparks_xy_press(&mut self, x: usize, y: usize) {
        let physical_x = x.min(GRID_WIDTH - 1) as f32 / (GRID_WIDTH - 1) as f32;
        let physical_y = y.min(GRID_HEIGHT - 1) as f32 / (GRID_HEIGHT - 1) as f32;
        let mut x_value = physical_x;
        let mut y_value = physical_y;
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
        let changed_x = self.set_xy_runtime_source(
            ModulationSourceId::play_x(),
            self.xy_x_binding.clone(),
            x_value,
        );
        let changed_y = self.set_xy_runtime_source(
            ModulationSourceId::play_y(),
            self.xy_y_binding.clone(),
            y_value,
        );
        if changed_x || changed_y {
            if let Err(error) = self.process_dirty_modulation_step(true) {
                self.show_toast(format!("modulation composition unavailable: {error}"));
            }
        }
    }

    pub(super) fn handle_sparks_xy_release(&mut self) {
        if self.xy_release == "reset-center" {
            self.xy_touch = NativeXyTouch {
                x: 0.5,
                y: 0.5,
                display_x: 0.5,
                display_y: 0.5,
                active: false,
            };
            let changed_x = self.set_xy_runtime_source(
                ModulationSourceId::play_x(),
                self.xy_x_binding.clone(),
                0.5,
            );
            let changed_y = self.set_xy_runtime_source(
                ModulationSourceId::play_y(),
                self.xy_y_binding.clone(),
                0.5,
            );
            if changed_x || changed_y {
                if let Err(error) = self.process_dirty_modulation_step(true) {
                    self.show_toast(format!("modulation composition unavailable: {error}"));
                }
            }
        } else {
            self.xy_touch.active = false;
        }
    }

    fn set_xy_runtime_source(
        &mut self,
        source: ModulationSourceId,
        binding: Option<super::NativeParamBinding>,
        normalized: f32,
    ) -> bool {
        if let Some(binding) = binding {
            self.set_runtime_source_input(source, binding, f64::from(normalized))
        } else {
            self.clear_runtime_source_input(source)
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
            .pulses_layers
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
            if let Some(layer) = self.pulses_layers.get_mut(index) {
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
            if let Some(slot) = self.trigger_gate_restore_modes.get_mut(index) {
                *slot = Some(current);
            }
            if let Some(mode) = self.trigger_gate_modes.get_mut(index) {
                *mode = "zero".into();
            }
            if let Some(layer) = self.pulses_layers.get_mut(index) {
                layer.trigger_probability_mode = "zero".into();
            }
            self.display.toast = Some(NativeToast {
                message: format!("L{} triggers off", index + 1),
                offset: 0,
            });
        }
        self.mark_config_dirty();
    }
}
