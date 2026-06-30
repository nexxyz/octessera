use crate::protocol::{RunnerMessage, RuntimePlatformEffect};
use std::time::Instant;

use super::{
    display_part_index_from_y, DeviceInput, NativeRunner, NativeToast, RuntimeTransportState,
    SyncSource,
};

impl NativeRunner {
    fn refresh_modifier_state(&mut self) {
        let was_fn_held = self.ui.fn_held;
        let was_modifier_held =
            self.ui.fn_held || self.ui.shift_held || self.ui.combined_modifier_held;
        self.ui.combined_modifier_held = self.ui.combined_button_pressed
            || (self.ui.fn_button_pressed && self.ui.shift_button_pressed);
        self.ui.fn_held = self.ui.fn_button_pressed && !self.ui.combined_modifier_held;
        self.ui.shift_held = self.ui.shift_button_pressed && !self.ui.combined_modifier_held;
        let modifier_held = self.ui.fn_held || self.ui.shift_held || self.ui.combined_modifier_held;
        if self.ui.fn_held && !was_fn_held {
            self.fn_hold_started_at = Some(Instant::now());
        } else if !self.ui.fn_held {
            self.fn_hold_started_at = None;
        }
        if modifier_held && !was_modifier_held {
            self.modifier_hint_started_at = Some(Instant::now());
        } else if !modifier_held {
            self.modifier_hint_started_at = None;
        }
    }

    fn mark_modifier_consumed(&mut self) {
        if self.ui.fn_held || self.ui.shift_held || self.ui.combined_modifier_held {
            self.modifier_hint_started_at = None;
        }
    }

    pub(super) fn handle_device_input(
        &mut self,
        input: DeviceInput,
    ) -> Result<Vec<RunnerMessage>, String> {
        self.advance_oled_sleep_state();
        if self.startup_splash_presented
            && self.oled_mode == super::NativeOledMode::Splash
            && self.oled_splash_text == super::OLED_STARTUP_SPLASH_KEY
        {
            return self.messages_with_snapshot();
        }
        if self.record_display_interaction() {
            return self.messages_with_snapshot();
        }
        if self.confirm_dialog.is_some() {
            return self.handle_confirm_device_input(input);
        }
        let is_modifier_input = matches!(
            input,
            DeviceInput::ButtonShift { .. }
                | DeviceInput::ButtonFn { .. }
                | DeviceInput::ButtonCombinedModifier { .. }
        );
        let result = match input {
            DeviceInput::GridPress { x, y } => self.handle_grid_press_input(x, y),
            DeviceInput::GridRelease { x, y } => self.handle_grid_release_input(x, y),
            DeviceInput::BehaviorAction(action) => {
                let result = self.trigger_behavior_action_result(action.action_type)?;
                self.messages_with_input_result(result)
            }
            DeviceInput::ButtonS { pressed } => self.handle_button_s_input(pressed),
            DeviceInput::ButtonShift { pressed } => {
                self.ui.shift_button_pressed = pressed.unwrap_or(false);
                self.refresh_modifier_state();
                self.messages_with_snapshot()
            }
            DeviceInput::ButtonFn { pressed } => {
                self.ui.fn_button_pressed = pressed.unwrap_or(false);
                self.refresh_modifier_state();
                self.messages_with_snapshot()
            }
            DeviceInput::ButtonCombinedModifier { pressed } => {
                self.ui.combined_button_pressed = pressed.unwrap_or(false);
                self.refresh_modifier_state();
                self.messages_with_snapshot()
            }
            DeviceInput::EncoderTurn { delta, id } => {
                if let Some(index) = Self::aux_index(id.as_deref()) {
                    self.handle_aux_turn(index, delta)?;
                } else if id.as_deref().unwrap_or("main") == "main" && delta != 0 {
                    if self.help_popup.is_some() {
                        self.turn_help_popup(delta);
                    } else {
                        let editing = self.menu.state.editing;
                        let editing_key = if editing {
                            self.menu.current_key().map(str::to_owned)
                        } else {
                            None
                        };
                        self.menu.turn(delta);
                        if let Some(key) = editing_key {
                            self.apply_or_schedule_menu_key(&key)?;
                        }
                    }
                }
                self.messages_with_snapshot()
            }
            DeviceInput::EncoderPress { id } => self.handle_encoder_press_input(id.as_deref()),
            DeviceInput::ButtonA { pressed } => self.handle_button_a_input(pressed),
            DeviceInput::Other => self.messages_with_snapshot(),
        };
        if !is_modifier_input {
            self.mark_modifier_consumed();
        }
        result
    }

    fn handle_confirm_device_input(
        &mut self,
        input: DeviceInput,
    ) -> Result<Vec<RunnerMessage>, String> {
        match input {
            DeviceInput::EncoderTurn { delta, id } if id.as_deref().unwrap_or("main") == "main" => {
                self.turn_confirm_dialog(delta);
            }
            DeviceInput::EncoderPress { id } if id.as_deref().unwrap_or("main") == "main" => {
                if let Some(effect) = self.confirm_dialog_selection()? {
                    return self.messages_with_effects(vec![effect]);
                }
            }
            DeviceInput::ButtonA { pressed } if pressed.unwrap_or(true) => {
                self.confirm_dialog = None;
                self.toast = Some(NativeToast {
                    message: "Cancelled".into(),
                    offset: 0,
                });
            }
            _ => {}
        }
        self.messages_with_snapshot()
    }

    fn handle_grid_press_input(
        &mut self,
        x: usize,
        y: usize,
    ) -> Result<Vec<RunnerMessage>, String> {
        if self.dance_fx_assign.is_some() {
            self.handle_dance_fx_assignment_grid_press(x, y);
        } else if self.sample_assign.is_some() {
            self.handle_sample_assignment_grid_press(x, y);
        } else if self.trigger_probability_assign.is_some() {
            self.handle_trigger_probability_grid_press(x, y);
        } else if self.ui.fn_held && x == 0 && !self.ui.shift_held {
            self.select_active_part(display_part_index_from_y(y))?;
            self.active_dance_mode = "none".into();
        } else if self.ui.fn_held && x == super::GRID_WIDTH - 1 && !self.ui.shift_held {
            self.select_dance_page_from_fn_grid(y);
        } else if self.ui.shift_held && !self.ui.fn_held && self.active_dance_mode == "none" {
            if !self.handle_param_mod_grid_press(x, y) {
                self.mark_grid_input_dirty();
                let result = self.active_engine_input_result(DeviceInput::GridPress { x, y })?;
                return self.messages_with_input_result(result);
            }
        } else if self.active_dance_mode == "trigger-gate" {
            self.handle_trigger_gate_grid_press(x, y);
        } else if self.active_dance_mode == "fx" {
            let effects = self.dance_fx_press_effects(x, y);
            if !effects.is_empty() {
                return self.messages_with_effects(effects);
            }
        } else if self.active_dance_mode != "none" {
            self.handle_dance_grid_press(x, y);
        } else {
            self.mark_grid_input_dirty();
            let result = self.active_engine_input_result(DeviceInput::GridPress { x, y })?;
            return self.messages_with_input_result(result);
        }
        self.messages_with_snapshot()
    }

    fn handle_grid_release_input(
        &mut self,
        x: usize,
        y: usize,
    ) -> Result<Vec<RunnerMessage>, String> {
        if self.active_dance_mode != "none" {
            if self.active_dance_mode == "fx" {
                let effects = self.dance_fx_release_effects(x, y);
                if !effects.is_empty() {
                    return self.messages_with_effects(effects);
                }
                return self.messages_with_snapshot();
            }
            if self.active_dance_mode == "xy" {
                self.handle_dance_xy_release();
            }
            return self.messages_with_snapshot();
        }
        self.mark_grid_input_dirty();
        let result = self.active_engine_input_result(DeviceInput::GridRelease { x, y })?;
        self.messages_with_input_result(result)
    }

    fn mark_grid_input_dirty(&mut self) {
        if self.behavior.id() == "looper" {
            self.mark_fast_autosave_dirty();
        } else {
            self.config_dirty = true;
        }
    }

    fn handle_button_s_input(
        &mut self,
        pressed: Option<bool>,
    ) -> Result<Vec<RunnerMessage>, String> {
        if pressed.unwrap_or(true) {
            if let Some(effect) = self.preview_selected_sample()? {
                return self.messages_with_effects(vec![effect]);
            } else if self.ui.shift_held && self.sync_source == SyncSource::External {
                self.pending_resync = true;
            } else if self.ui.shift_held {
                self.transport = RuntimeTransportState::Stopped;
                self.reset_transport_position();
                return self.messages_with_effects(vec![RuntimePlatformEffect::MidiPanic]);
            } else if self.ui.fn_held {
                self.toggle_active_part_trigger_gate();
            } else {
                if self.transport == RuntimeTransportState::Stopped {
                    self.reset_transport_position();
                }
                self.transport = if self.transport == RuntimeTransportState::Playing {
                    RuntimeTransportState::Paused
                } else {
                    RuntimeTransportState::Playing
                };
            }
        }
        self.messages_with_snapshot()
    }
}
