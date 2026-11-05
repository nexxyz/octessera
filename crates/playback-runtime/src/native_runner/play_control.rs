use crate::protocol::{RuntimeAudioCommand, RuntimePlatformEffect};

use super::grid_assign::GridAssign;
use super::pan_mapping::touch_pan_pos_from_grid_x;
use super::play_fx_config::{play_fx_params, play_fx_target_key, play_fx_type};
use super::{
    momentary_fx_target, play_fx_tile_id, NativePlayFxAssignment, NativeRunner, GRID_HEIGHT,
    GRID_WIDTH, PLAY_FX_MAX_CONCURRENT, TRIGGER_GATE_MODES,
};

pub(super) const MIX_MASTER_COLUMN: usize = 5;

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
                id: play_fx_tile_id(x, y),
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
        let id = play_fx_tile_id(x, y);
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
        let id = play_fx_tile_id(x, y);
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
        let Some(GridAssign::PlayFx(config)) = self.grid_assign.take() else {
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
            self.show_toast("FX cleared");
            return;
        }
        self.play_fx_assignments
            .retain(|assignment| assignment.x != x || assignment.y != y);
        if play_fx_type(&config) != "none" {
            self.play_fx_assignments
                .push(NativePlayFxAssignment { x, y, config });
        }
        self.mark_config_dirty();
        self.show_toast("FX mapped");
    }

    pub(super) fn handle_play_grid_press(&mut self, x: usize, y: usize) {
        match self.active_play_mode.as_str() {
            "mix" if self.display.ui.shift_held => self.handle_play_mix_bus_press(x, y),
            "mix" if y == 0 => self.toggle_instrument_mute(x),
            "mix" => {
                if self
                    .instruments
                    .get(x)
                    .is_some_and(|instrument| instrument.kind != "none")
                {
                    let volume = ((y as f32 / (GRID_HEIGHT - 1) as f32) * 100.0).round() as i32;
                    self.set_play_menu_number(&format!("instruments.{x}.mixer.volume"), volume);
                }
            }
            "pan" => self.handle_play_pan_press(x, y),
            "route" => self.handle_play_route_press(x, y),
            "tempo" => self.handle_play_tempo_press(x, y),
            "lfo" => self.handle_play_lfo_press(x, y),
            "faders" => self.handle_play_faders_press(x, y),
            "rate" => self.handle_play_rate_press(x, y),
            "xy" => self.handle_play_xy_press(x, y),
            _ => {}
        }
    }

    /// The Mix page's Shift view: buses B1..B4 in columns 0..3, master in column 5.
    fn handle_play_mix_bus_press(&mut self, x: usize, y: usize) {
        let volume = ((y as f32 / (GRID_HEIGHT - 1) as f32) * 100.0).round() as u8;
        match x {
            0..=3 if y == 0 => self.toggle_bus_mute(x),
            0..=3 => {
                self.set_fx_bus_volume(x, volume);
            }
            MIX_MASTER_COLUMN if y > 0 => self.set_play_menu_number("masterVolume", volume.into()),
            _ => {}
        }
    }

    /// Bus-routed instruments pan their bus; their own pan has no menu row then but is kept.
    fn handle_play_pan_press(&mut self, x: usize, y: usize) {
        let pan_pos = i32::from(touch_pan_pos_from_grid_x(x));
        let Some(instrument) = self
            .instruments
            .get(y)
            .filter(|instrument| instrument.kind != "none")
        else {
            return;
        };
        let bus_index = instrument
            .route
            .strip_prefix("fx_bus_")
            .and_then(|value| value.parse::<usize>().ok())
            .and_then(|value| value.checked_sub(1));
        let Some(bus_index) = bus_index else {
            self.set_play_menu_number(&format!("instruments.{y}.mixer.panPos"), pan_pos);
            return;
        };
        if self.fast_instrument_pan_key(y, Some(pan_pos)) {
            self.mark_fast_autosave_dirty();
        }
        self.set_play_menu_number(&format!("mixer.buses.{bus_index}.panPos"), pan_pos);
    }

    /// A Play page press edits its menu row like a turn would, so the row and modulation follow.
    fn set_play_menu_number(&mut self, key: &str, value: i32) {
        self.menu.set_number_value_for_key(key, value);
        if let Err(error) = self.apply_or_schedule_menu_key(key) {
            self.show_toast(error);
        }
    }

    pub(super) fn handle_play_xy_press(&mut self, x: usize, y: usize) {
        self.handle_play_xy_press_at(x, y, std::time::Instant::now());
    }

    pub(super) fn handle_play_xy_press_at(&mut self, x: usize, y: usize, now: std::time::Instant) {
        self.play_live.xy_latched = self.display.ui.shift_held;
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
        if self.xy_release == "reset-center" && !self.play_live.xy_latched {
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
        match (x, y) {
            (0..=2, _) => self.apply_trigger_gate_mode_to_layer(y, TRIGGER_GATE_MODES[x]),
            (5..=7, 0) => self.apply_trigger_gate_mode_to_all_layers(TRIGGER_GATE_MODES[x - 5]),
            _ => {}
        }
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
            self.show_toast(format!("L{} triggers {}", index + 1, restore));
        } else {
            self.drain_layer_owned_notes(index);
            self.apply_trigger_gate_mode_to_layer(index, "zero");
            if let Some(slot) = self.trigger_gate_restore_modes.get_mut(index) {
                *slot = Some(current);
            }
            self.show_toast(format!("L{} triggers off", index + 1));
        }
        self.mark_config_dirty();
    }
}
