use crate::native_runner::InstrumentType;
use crate::protocol::{RuntimeAudioCommand, RuntimePlatformEffect};

use super::grid_assign::GridAssign;
use super::pan_mapping::touch_pan_pos_from_grid_x;
use super::play_fx_config::{play_fx_audio_params, play_fx_target_key, play_fx_type};
use super::play_pages::PlayMode;
use super::{
    momentary_fx_target, play_fx_tile_id, NativePlayFxAssignment, NativeRunner,
    TriggerProbabilityMode, GRID_HEIGHT, GRID_WIDTH, PLAY_FX_MAX_CONCURRENT,
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
                params: play_fx_audio_params(&assignment.config, self.play_fx_tempo_bpm()),
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
            self.show_toast(format!(
                "FX {} already on",
                play_fx_type_label(&assignment.config)
            ));
            return Vec::new();
        } else if self.active_play_fx.len() >= PLAY_FX_MAX_CONCURRENT {
            self.show_toast(format!("FX limit: {PLAY_FX_MAX_CONCURRENT} at once"));
            return Vec::new();
        }
        if let Some(start) = self.play_fx_start_effect_for_assignment(&assignment) {
            self.active_play_fx.push((id, fx_type));
            self.play_live.held_fx_bpm = Some(self.play_fx_tempo_bpm());
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
            self.show_toast("Cleared");
            return;
        }
        self.play_fx_assignments
            .retain(|assignment| assignment.x != x || assignment.y != y);
        let toast = format!("FX {}", play_fx_type_label(&config));
        if play_fx_type(&config) != "none" {
            self.play_fx_assignments
                .push(NativePlayFxAssignment { x, y, config });
        }
        self.mark_config_dirty();
        self.show_toast(toast);
    }

    pub(super) fn handle_play_grid_press(
        &mut self,
        mode: PlayMode,
        x: usize,
        y: usize,
    ) -> Vec<RuntimePlatformEffect> {
        match mode {
            PlayMode::Mix if self.display.ui.shift_held => self.handle_play_mix_bus_press(x, y),
            PlayMode::Mix if y == 0 => self.toggle_instrument_mute(x),
            PlayMode::Mix => {
                if self
                    .instruments
                    .get(x)
                    .is_some_and(|instrument| instrument.kind != InstrumentType::None)
                {
                    let volume = ((y as f32 / (GRID_HEIGHT - 1) as f32) * 100.0).round() as i32;
                    self.set_play_menu_number(&format!("instruments.{x}.mixer.volume"), volume);
                }
            }
            PlayMode::Pan => self.handle_play_pan_press(x, y),
            PlayMode::Route => self.handle_play_route_press(x, y),
            PlayMode::Tempo => self.handle_play_tempo_press(x, y),
            PlayMode::Lfo => self.handle_play_lfo_press(x, y),
            PlayMode::Faders => self.handle_play_faders_press(x, y),
            PlayMode::Rate => self.handle_play_rate_press(x, y),
            PlayMode::Xy => self.handle_play_xy_press(x, y),
            PlayMode::TriggerGate => self.handle_trigger_gate_grid_press(x, y),
            PlayMode::Transpose => self.handle_play_transpose_grid_press(x, y),
            PlayMode::Fx => return self.play_fx_press_effects(x, y),
        }
        Vec::new()
    }

    /// The Mix page's Shift view: buses B1..B4 in columns 0..3, master in column 5.
    fn handle_play_mix_bus_press(&mut self, x: usize, y: usize) {
        let volume = ((y as f32 / (GRID_HEIGHT - 1) as f32) * 100.0).round() as u8;
        match x {
            0..=3 if y == 0 => self.toggle_bus_mute(x),
            0..=3 => self.set_play_menu_number(&format!("mixer.buses.{x}.volume"), volume.into()),
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
            .filter(|instrument| instrument.kind != InstrumentType::None)
        else {
            return;
        };
        let Some(bus_index) = instrument.route.bus_index() else {
            self.set_play_menu_number(&format!("instruments.{y}.mixer.panPos"), pan_pos);
            return;
        };
        if self.fast_instrument_pan_key(y, Some(pan_pos)) {
            self.mark_fast_autosave_dirty();
        }
        self.set_play_menu_number(&format!("mixer.buses.{bus_index}.panPos"), pan_pos);
    }

    fn set_play_menu_number(&mut self, key: &str, value: i32) {
        self.menu.set_number_value_for_key(key, value);
        self.dispatch_play_menu_key(key);
    }

    /// A Play page press dispatches the menu row it wrote like a turn would, so the row and
    /// modulation follow; an error shows as a toast. Returns whether the edit applied.
    pub(super) fn dispatch_play_menu_key(&mut self, key: &str) -> bool {
        match self.apply_or_schedule_menu_key(key) {
            Ok(()) => true,
            Err(error) => {
                self.show_toast(error);
                false
            }
        }
    }

    pub(super) fn handle_play_xy_press(&mut self, x: usize, y: usize) {
        self.handle_play_xy_press_at(x, y, std::time::Instant::now());
    }

    pub(super) fn handle_play_xy_press_at(&mut self, x: usize, y: usize, now: std::time::Instant) {
        self.play_live.xy_latched = self.display.ui.shift_held;
        self.play_live.xy_held_tile = Some((x, y));
        let physical_x = x.min(GRID_WIDTH - 1) as f32 / (GRID_WIDTH - 1) as f32;
        let physical_y = y.min(GRID_HEIGHT - 1) as f32 / (GRID_HEIGHT - 1) as f32;
        self.xy.touch.display_x = physical_x;
        self.xy.touch.display_y = physical_y;
        self.xy.touch.active = true;
        if self.retarget_xy_runtime_sources_at(now, false) {
            if let Err(error) = self.process_dirty_modulation_step(true) {
                self.show_modulation_unavailable(&error);
            }
        }
    }

    pub(super) fn handle_play_xy_release(&mut self) {
        self.handle_play_xy_release_at(std::time::Instant::now());
    }

    pub(super) fn handle_play_xy_release_at(&mut self, now: std::time::Instant) {
        self.play_live.xy_held_tile = None;
        if self.xy.release == "reset-center" && !self.play_live.xy_latched {
            self.xy.touch.display_x = 0.5;
            self.xy.touch.display_y = 0.5;
            self.xy.touch.active = false;
            if self.retarget_xy_runtime_sources_at(now, true) {
                if let Err(error) = self.process_dirty_modulation_step(true) {
                    self.show_modulation_unavailable(&error);
                }
            }
        } else {
            self.xy.touch.active = false;
        }
    }

    pub(super) fn handle_trigger_gate_grid_press(&mut self, x: usize, y: usize) {
        match (x, y) {
            (0..=2, _) => {
                self.apply_trigger_gate_mode_to_layer(y, TriggerProbabilityMode::ALL[x]);
            }
            (5..=7, 0) => {
                self.apply_trigger_gate_mode_to_all_layers(TriggerProbabilityMode::ALL[x - 5]);
            }
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
            .map(|layer| layer.trigger_probability_mode)
            .or_else(|| self.trigger_gate_modes.get(index).copied())
            .unwrap_or(TriggerProbabilityMode::Full);
        if current == TriggerProbabilityMode::Zero {
            let restore = self
                .trigger_gate_restore_modes
                .get(index)
                .copied()
                .flatten()
                .unwrap_or(TriggerProbabilityMode::Full);
            if let Some(mode) = self.trigger_gate_modes.get_mut(index) {
                *mode = restore;
            }
            if let Some(layer) = self.link_layers.get_mut(index) {
                layer.trigger_probability_mode = restore;
            }
            if let Some(slot) = self.trigger_gate_restore_modes.get_mut(index) {
                *slot = None;
            }
            self.show_toast(format!("L{} triggers {}", index + 1, restore.wire_name()));
        } else {
            self.drain_layer_owned_notes(index);
            self.apply_trigger_gate_mode_to_layer(index, TriggerProbabilityMode::Zero);
            if let Some(slot) = self.trigger_gate_restore_modes.get_mut(index) {
                *slot = Some(current);
            }
            self.show_toast(format!("L{} triggers off", index + 1));
        }
        self.mark_config_dirty();
    }
}

/// The display name of a Play FX config's type (`filter sweep`, not `filter_sweep`).
pub(super) fn play_fx_type_label(config: &serde_json::Value) -> String {
    crate::native_menu::format_values::format_display_value(
        Some("play.fx.type"),
        play_fx_type(config),
    )
}
