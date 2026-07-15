use crate::protocol::{RunnerMessage, RuntimePlatformEffect};
use std::sync::OnceLock;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use super::algorithm::LinkRoutingInput;
use super::{
    display_layer_index_from_y, DeviceInput, NativeRunner, NativeToast, RuntimeTransportState,
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
        let trace_context = WakeTraceContext::capture(self, &input);
        if self.startup_splash_presented
            && self.oled_mode == super::NativeOledMode::Splash
            && self.oled_splash_text == super::OLED_STARTUP_SPLASH_KEY
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
        if self.confirm_dialog.is_some() {
            return self.handle_confirm_device_input(input);
        }
        if self.usb_sd_transfer_modal.is_some() {
            return self.handle_usb_sd_transfer_modal_input(input);
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
                    } else if self.ui.fn_held && delta > 0 {
                        return self.handle_single_step_input();
                    } else if self.ui.fn_held {
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
        if self.sparks_fx_assign.is_some() {
            self.handle_sparks_fx_assignment_grid_press(x, y);
        } else if self.sample_assign.is_some() {
            self.handle_sample_assignment_grid_press(x, y);
        } else if self.trigger_probability_assign.is_some() {
            self.handle_trigger_probability_grid_press(x, y);
        } else if self.active_sparks_mode == "transpose" && x == 0 && self.ui.shift_held {
            self.toggle_all_sparks_transpose_layers();
        } else if self.ui.combined_modifier_held && x == 0 {
            self.toggle_layer_trigger_gate(display_layer_index_from_y(y));
        } else if self.ui.fn_held && x == 0 && !self.ui.shift_held {
            self.select_active_layer(display_layer_index_from_y(y))?;
            self.active_sparks_mode = "none".into();
        } else if self.ui.fn_held && x == super::GRID_WIDTH - 1 && !self.ui.shift_held {
            self.select_sparks_page_from_fn_grid(y);
        } else if self.ui.shift_held && !self.ui.fn_held && self.active_sparks_mode == "none" {
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
            self.config_dirty = true;
        }
    }

    fn handle_button_s_input(
        &mut self,
        pressed: Option<bool>,
    ) -> Result<Vec<RunnerMessage>, String> {
        if pressed.unwrap_or(true) {
            if self.ui.combined_modifier_held {
                return self.messages_with_snapshot();
            } else if self.ui.fn_held {
                return self.reset_stop_with_midi_panic();
            } else if let Some(effect) = self.preview_selected_sample()? {
                return self.messages_with_effects(vec![effect]);
            } else if self.ui.shift_held && self.sync_source == SyncSource::External {
                self.pending_resync = true;
            } else if self.ui.shift_held {
                return self.reset_stop_with_midi_panic();
            } else {
                if self.transport == RuntimeTransportState::Stopped {
                    self.reset_transport_position();
                }
                let was_playing = self.transport == RuntimeTransportState::Playing;
                self.transport = if self.transport == RuntimeTransportState::Playing {
                    RuntimeTransportState::Paused
                } else {
                    RuntimeTransportState::Playing
                };
                if was_playing && self.transport == RuntimeTransportState::Paused {
                    return self.messages_with_effects(vec![RuntimePlatformEffect::MidiPanic]);
                }
            }
        }
        self.messages_with_snapshot()
    }

    fn reset_stop_with_midi_panic(&mut self) -> Result<Vec<RunnerMessage>, String> {
        self.transport = RuntimeTransportState::Stopped;
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
            self.usb_sd_transfer_modal = None;
            return self.messages_with_effects(vec![RuntimePlatformEffect::UsbSdTransferStop]);
        }
        self.messages_with_snapshot()
    }

    fn handle_single_step_input(&mut self) -> Result<Vec<RunnerMessage>, String> {
        if self.transport == RuntimeTransportState::Playing {
            self.show_toast("Pause first");
            return self.messages_with_snapshot();
        }
        let tick = self.active_engine_tick_result()?;
        self.tick = self.tick.saturating_add(1);
        if let Some(layer_tick) = self.layer_ticks.get_mut(self.active_layer_index) {
            *layer_tick = self.tick;
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
        events.dedupe_note_ons_by_highest_velocity();
        let mut messages = self.messages_with_routed_events(events)?;
        messages.extend(self.messages_with_snapshot()?);
        Ok(messages)
    }
}

struct WakeTraceContext {
    input: String,
    mode: &'static str,
    splash: String,
}

impl WakeTraceContext {
    fn capture(runner: &NativeRunner, input: &DeviceInput) -> Option<Self> {
        wake_trace_enabled().then(|| Self {
            input: device_input_trace_summary(input),
            mode: oled_mode_trace_name(&runner.oled_mode),
            splash: runner.oled_splash_text.clone(),
        })
    }
}

fn trace_device_input_wake(
    context: Option<&WakeTraceContext>,
    woke_display: bool,
    consumed: bool,
    outcome: &str,
) {
    let Some(context) = context else {
        return;
    };
    eprintln!(
        "wake_trace ts_ms={} source=runtime event=wake_decision mode={} splash={} woke_display={woke_display} consumed={consumed} outcome={outcome} {}",
        wake_trace_timestamp_ms(),
        context.mode,
        context.splash,
        context.input
    );
}

fn device_input_trace_summary(input: &DeviceInput) -> String {
    match input {
        DeviceInput::EncoderTurn { delta, id } => {
            format!("type=encoder_turn id={} delta={delta}", id_trace_value(id))
        }
        DeviceInput::EncoderPress { id } => {
            format!("type=encoder_press id={}", id_trace_value(id))
        }
        DeviceInput::ButtonA { pressed } => button_trace_summary("button_a", *pressed),
        DeviceInput::ButtonS { pressed } => button_trace_summary("button_s", *pressed),
        DeviceInput::ButtonShift { pressed } => button_trace_summary("button_shift", *pressed),
        DeviceInput::ButtonFn { pressed } => button_trace_summary("button_fn", *pressed),
        DeviceInput::ButtonCombinedModifier { pressed } => {
            button_trace_summary("button_combined_modifier", *pressed)
        }
        DeviceInput::GridPress { x, y } => format!("type=grid_press x={x} y={y}"),
        DeviceInput::GridRelease { x, y } => format!("type=grid_release x={x} y={y}"),
        DeviceInput::BehaviorAction(action) => {
            format!("type=behavior_action action_type={}", action.action_type)
        }
        DeviceInput::Other => "type=other".to_string(),
    }
}

fn button_trace_summary(input_type: &str, pressed: Option<bool>) -> String {
    format!("type={input_type} pressed={}", pressed_trace_value(pressed))
}

fn id_trace_value(id: &Option<String>) -> &str {
    id.as_deref().unwrap_or("default")
}

fn pressed_trace_value(pressed: Option<bool>) -> &'static str {
    match pressed {
        Some(true) => "true",
        Some(false) => "false",
        None => "default",
    }
}

fn oled_mode_trace_name(mode: &super::NativeOledMode) -> &'static str {
    match mode {
        super::NativeOledMode::Normal => "normal",
        super::NativeOledMode::Splash => "splash",
        super::NativeOledMode::Off => "off",
    }
}

fn wake_trace_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("OCTESSERA_WAKE_TRACE")
            .is_ok_and(|value| !matches!(value.as_str(), "" | "0" | "false" | "off"))
    })
}

fn wake_trace_timestamp_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}
