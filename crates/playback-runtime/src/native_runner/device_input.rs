use crate::protocol::{RunnerMessage, RuntimePlatformEffect};
use std::time::Instant;

use super::algorithm::LinkRoutingInput;
use super::{
    display_layer_index_from_y, DeviceInput, NativeRunner, NativeToast, RuntimeTransportState,
    SyncSource,
};

#[path = "device_input_wake_trace.rs"]
mod device_input_wake_trace;
use device_input_wake_trace::{trace_device_input_wake, WakeTraceContext};

impl NativeRunner {
    pub(super) fn refresh_modifier_state(&mut self) {
        let was_fn_held = self.display.ui.fn_held;
        let was_modifier_held = self.display.ui.fn_held
            || self.display.ui.shift_held
            || self.display.ui.combined_modifier_held;
        self.display.ui.combined_modifier_held = self.display.ui.combined_button_pressed
            || (self.display.ui.fn_button_pressed && self.display.ui.shift_button_pressed);
        self.display.ui.fn_held =
            self.display.ui.fn_button_pressed && !self.display.ui.combined_modifier_held;
        self.display.ui.shift_held =
            self.display.ui.shift_button_pressed && !self.display.ui.combined_modifier_held;
        let modifier_held = self.display.ui.fn_held
            || self.display.ui.shift_held
            || self.display.ui.combined_modifier_held;
        if self.display.ui.fn_held && !was_fn_held {
            self.display.fn_hold_started_at = Some(Instant::now());
        } else if !self.display.ui.fn_held {
            self.display.fn_hold_started_at = None;
        }
        if modifier_held && !was_modifier_held {
            self.display.modifier_hint_started_at = Some(Instant::now());
        } else if !modifier_held {
            self.display.modifier_hint_started_at = None;
        }
    }

    fn mark_modifier_consumed(&mut self) {
        if self.display.ui.fn_held
            || self.display.ui.shift_held
            || self.display.ui.combined_modifier_held
        {
            self.display.modifier_hint_started_at = None;
        }
    }

    fn reconcile_modifier_input(&mut self, input: &DeviceInput) -> bool {
        match input {
            DeviceInput::ButtonShift { pressed } => {
                self.display.ui.shift_button_pressed = pressed.unwrap_or(false);
                self.refresh_modifier_state();
                true
            }
            DeviceInput::ButtonFn { pressed } => {
                self.display.ui.fn_button_pressed = pressed.unwrap_or(false);
                self.refresh_modifier_state();
                true
            }
            DeviceInput::ButtonCombinedModifier { pressed } => {
                self.display.ui.combined_button_pressed = pressed.unwrap_or(false);
                self.refresh_modifier_state();
                true
            }
            _ => false,
        }
    }

    pub(super) fn handle_device_input(
        &mut self,
        input: DeviceInput,
    ) -> Result<Vec<RunnerMessage>, String> {
        self.handle_device_input_with_error_mode(input, false)
    }

    pub(super) fn handle_presented_runtime_error_input(
        &mut self,
        input: DeviceInput,
    ) -> Result<Vec<RunnerMessage>, String> {
        self.handle_device_input_with_error_mode(input, true)
    }

    fn handle_device_input_with_error_mode(
        &mut self,
        input: DeviceInput,
        force_error_presentation: bool,
    ) -> Result<Vec<RunnerMessage>, String> {
        let is_modifier_input = self.reconcile_modifier_input(&input);
        if self.display.oled_mode == super::NativeOledMode::Splash
            && self.display.oled_splash_text == super::OLED_STARTUP_SPLASH_KEY
        {
            self.advance_oled_sleep_state();
        }
        let trace_context = WakeTraceContext::capture(self, &input);
        if self.display.startup_splash_presented
            && self.display.oled_mode == super::NativeOledMode::Splash
            && self.display.oled_splash_text == super::OLED_STARTUP_SPLASH_KEY
        {
            trace_device_input_wake(trace_context.as_ref(), false, true, "startup_splash");
            return self.messages_with_forced_snapshot();
        }
        let woke_display = self.record_display_interaction();
        if woke_display {
            trace_device_input_wake(trace_context.as_ref(), true, true, "wake_consumed");
            return self.messages_with_forced_snapshot();
        }
        trace_device_input_wake(trace_context.as_ref(), false, false, "active_dispatch");
        if force_error_presentation || self.display.runtime_error_presentation.is_some() {
            return self.handle_runtime_error_presentation_input(input, force_error_presentation);
        }
        if self.display.user_data_restore.is_some() {
            return self.handle_user_data_restore_input(input);
        }
        if self.display.confirm_dialog.is_some() {
            return self.handle_confirm_device_input(input);
        }
        if self
            .display
            .setup_portal
            .as_ref()
            .is_some_and(|setup| setup.visible)
        {
            return self.handle_setup_portal_modal_input(input);
        }
        if self
            .display
            .user_data_transfer
            .as_ref()
            .is_some_and(|transfer| transfer.visible)
        {
            return self.handle_user_data_transfer_modal_input(input);
        }
        if self.display.usb_sd_transfer_modal.is_some() {
            return self.handle_usb_sd_transfer_modal_input(input);
        }
        if self.display.system_info_modal.is_some() {
            return self.handle_system_info_modal_input(input);
        }
        let result = match input {
            DeviceInput::GridPress { x, y } => self.handle_grid_press_input(x, y),
            DeviceInput::GridRelease { x, y } => self.handle_grid_release_input(x, y),
            DeviceInput::BehaviorAction(action) => {
                let result = self.trigger_behavior_action_result(action.action_type)?;
                self.messages_with_input_result(result)
            }
            DeviceInput::ButtonS { pressed } => self.handle_button_s_input(pressed),
            DeviceInput::ButtonShift { .. }
            | DeviceInput::ButtonFn { .. }
            | DeviceInput::ButtonCombinedModifier { .. } => self.messages_with_snapshot(),
            DeviceInput::EncoderTurn { delta, id } => {
                if let Some(index) = Self::aux_index(id.as_deref()) {
                    self.handle_aux_turn(index, delta)?;
                } else if id.as_deref().unwrap_or("main") == "main" && delta != 0 {
                    if self.display.help_popup.is_some() {
                        self.turn_help_popup(delta);
                    } else if self.display.ui.fn_held && delta > 0 {
                        return self.handle_single_step_input();
                    } else if self.display.ui.fn_held {
                    } else {
                        let editing = self.menu.state.editing;
                        let editing_key = if editing {
                            self.menu.current_key().map(str::to_owned)
                        } else {
                            None
                        };
                        self.reset_menu_scroll();
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

    fn handle_user_data_restore_input(
        &mut self,
        input: DeviceInput,
    ) -> Result<Vec<RunnerMessage>, String> {
        if !self.user_data_restore_is_active()
            && (matches!(
                input,
                DeviceInput::EncoderPress { ref id }
                    if id.as_deref().unwrap_or("main") == "main"
            ) || matches!(input, DeviceInput::ButtonA { pressed } if pressed.unwrap_or(true)))
        {
            self.display.user_data_restore = None;
        }
        self.messages_with_snapshot()
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
                self.display.confirm_dialog = None;
                self.display.toast = Some(NativeToast {
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
        if self.sparks_fx_assign.is_some() {
            self.handle_sparks_fx_assignment_grid_press(x, y);
        } else if self.sample_assign.is_some() {
            self.handle_sample_assignment_grid_press(x, y);
        } else if self.trigger_probability_assign.is_some() {
            self.handle_trigger_probability_grid_press(x, y);
        } else if self.active_sparks_mode == "transpose" && x == 0 && self.display.ui.shift_held {
            self.toggle_all_sparks_transpose_layers();
        } else if self.display.ui.combined_modifier_held && x == 0 {
            self.toggle_layer_trigger_gate(display_layer_index_from_y(y));
        } else if self.display.ui.fn_held && x == 0 && !self.display.ui.shift_held {
            self.select_active_layer(display_layer_index_from_y(y))?;
            self.active_sparks_mode = "none".into();
        } else if self.display.ui.fn_held
            && x == super::GRID_WIDTH - 1
            && !self.display.ui.shift_held
        {
            self.select_sparks_page_from_fn_grid(y);
        } else if self.display.ui.shift_held
            && !self.display.ui.fn_held
            && self.active_sparks_mode == "none"
        {
            if !self.handle_param_mod_grid_press(x, y) {
                self.mark_grid_input_dirty();
                let result = self.active_engine_input_result(DeviceInput::GridPress { x, y })?;
                return self.messages_with_input_result(result);
            }
        } else if self.active_sparks_mode == "trigger-gate" {
            self.handle_trigger_gate_grid_press(x, y);
        } else if self.active_sparks_mode == "transpose" {
            self.handle_sparks_transpose_grid_press(x, y);
        } else if self.active_sparks_mode == "fx" {
            let effects = self.sparks_fx_press_effects(x, y);
            if !effects.is_empty() {
                return self.messages_with_effects(effects);
            }
        } else if self.active_sparks_mode != "none" {
            self.handle_sparks_grid_press(x, y);
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
        if self.active_sparks_mode != "none" {
            if self.active_sparks_mode == "fx" {
                let effects = self.sparks_fx_release_effects(x, y);
                if !effects.is_empty() {
                    return self.messages_with_effects(effects);
                }
                return self.messages_with_snapshot();
            }
            if self.active_sparks_mode == "xy" {
                self.handle_sparks_xy_release();
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
            self.mark_config_dirty();
        }
    }

    fn handle_button_s_input(
        &mut self,
        pressed: Option<bool>,
    ) -> Result<Vec<RunnerMessage>, String> {
        if pressed.unwrap_or(true) {
            if self.display.ui.combined_modifier_held {
                return self.messages_with_snapshot();
            } else if self.display.ui.fn_held {
                return self.reset_stop_with_midi_panic();
            } else if let Some(effect) = self.preview_selected_sample()? {
                return self.messages_with_effects(vec![effect]);
            } else if self.display.ui.shift_held
                && self.transport.sync_source == SyncSource::External
            {
                self.transport.pending_resync = true;
            } else if self.display.ui.shift_held {
                return self.reset_stop_with_midi_panic();
            } else {
                if self.transport.transport == RuntimeTransportState::Stopped {
                    self.reset_transport_position();
                }
                let was_playing = self.transport.transport == RuntimeTransportState::Playing;
                self.transport.transport =
                    if self.transport.transport == RuntimeTransportState::Playing {
                        RuntimeTransportState::Paused
                    } else {
                        RuntimeTransportState::Playing
                    };
                if was_playing && self.transport.transport == RuntimeTransportState::Paused {
                    return self.messages_with_effects(vec![RuntimePlatformEffect::MidiPanic]);
                }
            }
        }
        self.messages_with_snapshot()
    }

    fn reset_stop_with_midi_panic(&mut self) -> Result<Vec<RunnerMessage>, String> {
        self.transport.transport = RuntimeTransportState::Stopped;
        self.reset_transport_position();
        self.messages_with_effects(vec![RuntimePlatformEffect::MidiPanic])
    }

    fn handle_usb_sd_transfer_modal_input(
        &mut self,
        input: DeviceInput,
    ) -> Result<Vec<RunnerMessage>, String> {
        let close_requested = matches!(
            input,
            DeviceInput::EncoderPress { ref id } if id.as_deref().unwrap_or("main") == "main"
        ) || matches!(input, DeviceInput::ButtonA { pressed } if pressed.unwrap_or(true));
        if close_requested {
            self.display.usb_sd_transfer_modal = None;
            return self.messages_with_effects(vec![RuntimePlatformEffect::UsbSdTransferStop]);
        }
        self.messages_with_snapshot()
    }

    fn handle_system_info_modal_input(
        &mut self,
        input: DeviceInput,
    ) -> Result<Vec<RunnerMessage>, String> {
        match input {
            DeviceInput::EncoderTurn { delta, id } if id.as_deref().unwrap_or("main") == "main" => {
                if let Some(modal) = self.display.system_info_modal.as_mut() {
                    modal.turn(delta);
                }
            }
            DeviceInput::EncoderPress { id } if id.as_deref().unwrap_or("main") == "main" => {
                self.display.system_info_modal = None;
            }
            DeviceInput::ButtonA { pressed } if pressed.unwrap_or(true) => {
                self.display.system_info_modal = None;
            }
            _ => {}
        }
        self.messages_with_snapshot()
    }

    fn handle_single_step_input(&mut self) -> Result<Vec<RunnerMessage>, String> {
        if self.transport.transport == RuntimeTransportState::Playing {
            self.show_toast("Pause first");
            return self.messages_with_snapshot();
        }
        let tick = self.active_engine_tick_result()?;
        self.transport.tick = self.transport.tick.saturating_add(1);
        if let Some(layer_tick) = self.transport.layer_ticks.get_mut(self.active_layer_index) {
            *layer_tick = self.transport.tick;
        }
        let mut events = self.take_due_link_events(self.active_layer_index);
        self.apply_runtime_modulation(&tick.mapped_intents, self.active_layer_index);
        let transpose_offset = self
            .sparks_transpose_offsets_for_routing()
            .get(self.active_layer_index)
            .copied()
            .unwrap_or(0);
        let instruments = self.instruments.clone();
        let sense = self.pulses_layers.get(self.active_layer_index).cloned();
        events.extend(self.route_events_with_link_timing(
            self.active_layer_index,
            LinkRoutingInput {
                events: tick.events,
                event_intents: &tick.event_intents,
                instruments: &instruments,
                sense,
                transpose_offset,
            },
        )?);
        let mut messages = self.messages_with_routed_events(events)?;
        messages.extend(self.messages_with_snapshot()?);
        Ok(messages)
    }
}
