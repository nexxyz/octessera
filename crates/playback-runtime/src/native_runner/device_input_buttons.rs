use crate::native_menu::{NativeMenuAction, NativeMenuPressResult};
use crate::protocol::{RunnerMessage, RuntimePlatformEffect};

use super::NativeRunner;

impl NativeRunner {
    pub(super) fn handle_encoder_press_input(
        &mut self,
        id: Option<&str>,
    ) -> Result<Vec<RunnerMessage>, String> {
        if let Some(index) = Self::aux_index(id) {
            if let Some(messages) = self.handle_aux_behavior_action_press(index)? {
                return Ok(messages);
            }
            if let Some(effect) = self.handle_aux_press(index)? {
                return self.messages_with_effects(vec![effect]);
            }
        } else if id.unwrap_or("main") == "main" {
            self.reset_menu_scroll();
            if self.help_popup.is_some() {
                self.help_popup = None;
                return self.messages_with_snapshot();
            }
            if self.ui.combined_modifier_held {
                self.open_contextual_help();
                return self.messages_with_snapshot();
            }
            let stack_depth_before = self.menu.state.stack.len();
            let selected_root_label = if self.menu.state.stack.is_empty() {
                self.menu.current_label().map(str::to_string)
            } else {
                None
            };
            let selected_nested_label = if self.menu.state.stack.len() == 1 {
                self.menu.current_label().map(str::to_string)
            } else {
                None
            };
            let selected_group_label = self.menu.current_label().map(str::to_string);
            let selected_group_key = self.menu.current_key().map(str::to_string);
            if let Some(key) = selected_group_key.as_deref() {
                if self.menu.state.stack.len() == 1
                    && self.menu.is_in_sparks_root_group()
                    && parameterless_sparks_page_key(key)
                {
                    self.apply_or_schedule_menu_key(key)?;
                    return self.messages_with_snapshot();
                }
            }
            let mut should_apply = false;
            let mut effects = Vec::new();
            let press_result = self.menu.press();
            if let Some(result) = press_result {
                match result {
                    NativeMenuPressResult::Action(action) => {
                        if let NativeMenuAction::BehaviorAction(action_type) = action {
                            let result = self.trigger_behavior_action_result(action_type)?;
                            return self.messages_with_input_result(result);
                        }
                        if let Some(effect) = self.execute_menu_action(action)? {
                            effects.push(effect);
                        }
                    }
                    NativeMenuPressResult::EnteredGroup => {
                        self.enter_root_group(selected_root_label.as_deref());
                        self.enter_nested_group(
                            stack_depth_before,
                            selected_nested_label.as_deref(),
                        )?;
                        match selected_group_label.as_deref() {
                            Some("MIDI Out") => {
                                effects.push(RuntimePlatformEffect::MidiListOutputsRequest)
                            }
                            Some("MIDI In") => {
                                effects.push(RuntimePlatformEffect::MidiListInputsRequest)
                            }
                            _ => {}
                        }
                        if let Some(key) = selected_group_key.as_deref() {
                            if let Some(effect) = self.sample_open_effect_for_key(key) {
                                effects.push(effect);
                            }
                        } else if let Some(effect) = self.sample_open_effect_for_current_group() {
                            effects.push(effect);
                        }
                    }
                    NativeMenuPressResult::EditingToggled { editing } => {
                        should_apply = !editing;
                    }
                    NativeMenuPressResult::TextCursorAdvanced => {}
                }
            }
            if should_apply {
                if let Some(key) = selected_group_key.as_deref() {
                    self.apply_or_schedule_menu_key(key)?;
                } else {
                    return Err("cannot apply menu edit: current row has no key".into());
                }
            }
            if !effects.is_empty() {
                return self.messages_with_effects(effects);
            }
        }
        self.messages_with_snapshot()
    }

    fn handle_aux_behavior_action_press(
        &mut self,
        index: usize,
    ) -> Result<Option<Vec<RunnerMessage>>, String> {
        if self.ui.fn_held || self.ui.combined_modifier_held {
            return Ok(None);
        }
        let click_prefix = if self.ui.shift_held { "S+Clk" } else { "Clk" };
        let Some(press) = self.effective_aux_slot(index).press else {
            return Ok(None);
        };
        let NativeMenuAction::BehaviorAction(action_type) = press.action else {
            return Ok(None);
        };
        let valid = self.worlds_menu_items().into_iter().any(|item| {
            matches!(
                item.value,
                crate::native_menu::NativeMenuValue::Action(NativeMenuAction::BehaviorAction(ref current)) if current == &action_type
            )
        });
        if !valid {
            self.show_toast(format!(
                "{click_prefix}-{}: {} not active",
                index + 1,
                press.label
            ));
            return Ok(Some(self.messages_with_snapshot()?));
        }
        let suppress_aux_toast = self.behavior.id() == "looper" && action_type == "toggleMode";
        let result = self.trigger_behavior_action_result(action_type)?;
        if !suppress_aux_toast {
            self.show_toast(format!("{click_prefix}-{}: {}", index + 1, press.label));
        }
        Ok(Some(self.messages_with_input_result(result)?))
    }

    pub(super) fn handle_button_a_input(
        &mut self,
        pressed: Option<bool>,
    ) -> Result<Vec<RunnerMessage>, String> {
        if !pressed.unwrap_or(true) {
            return self.messages_with_snapshot();
        }
        if self.sparks_fx_assign.is_some() {
            self.sparks_fx_assign = None;
        } else if self.sample_assign.is_some() {
            self.sample_assign = None;
        } else if self.trigger_probability_assign.is_some() {
            self.trigger_probability_assign = None;
        } else if self.help_popup.is_some() {
            self.help_popup = None;
        } else if self.ui.shift_held {
            let editing_key = self
                .menu
                .state
                .editing
                .then(|| self.menu.current_key().map(str::to_owned))
                .flatten();
            if self.menu.delete_text_char() {
                let Some(key) = editing_key else {
                    return Err("cannot apply text delete: current row has no key".into());
                };
                self.apply_or_schedule_menu_key(&key)?;
            } else {
                self.rebuild_engine(self.behavior)?;
            }
        } else {
            if self.active_sparks_mode != "none" {
                self.active_sparks_mode = "none".into();
            }
            let editing_key = self
                .menu
                .state
                .editing
                .then(|| self.menu.current_key().map(str::to_owned))
                .flatten();
            let prompt_for_audio_buffer_reboot = editing_key.as_deref()
                == Some("sound.audioOutputBufferFrames")
                && self.pending_audio_output_buffer_reboot_prompt;
            self.reset_menu_scroll();
            self.menu.back();
            if let Some(key) = editing_key {
                self.apply_or_schedule_menu_key(&key)?;
            }
            if prompt_for_audio_buffer_reboot {
                self.pending_audio_output_buffer_reboot_prompt = false;
                self.toast = None;
                let action = NativeMenuAction::PlatformEffect("system.reboot".into());
                self.confirm_dialog = self.confirmation_for_action(&action);
            }
        }
        self.messages_with_snapshot()
    }
}

fn parameterless_sparks_page_key(key: &str) -> bool {
    matches!(
        key,
        "sparks.page.mix"
            | "sparks.page.pan"
            | "sparks.page.trigger-gate"
            | "sparks.page.transpose"
    )
}
