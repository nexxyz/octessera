use super::super::{aux_turn_binding_spec, NativeAuxBinding, NativeParamBinding, NativeRunner};

impl NativeRunner {
    pub(super) fn bind_aux_from_current(&mut self, index: usize, shifted: bool) -> bool {
        let (turn_key, press_action) = self.menu.current_binding_target();
        if turn_key
            .as_deref()
            .is_some_and(super::inert_sample_binding_key)
        {
            self.show_toast(super::BindingValidationError::UnsupportedTarget.toast_message());
            return false;
        }
        let prefix = if shifted { "S+Clk" } else { "Clk" };
        if turn_key.is_none() && press_action.is_none() {
            self.show_toast(format!("{prefix}-{}: No binding", index + 1));
            return false;
        }
        let Some(current) = self.aux_bindings_for_bank(shifted).get(index) else {
            return false;
        };
        let current = current.clone().unwrap_or(NativeAuxBinding {
            turn_key: None,
            press_action: None,
        });
        let turn_changed = turn_key
            .as_ref()
            .is_some_and(|next| current.turn_key.as_ref() != Some(next));
        let click_changed = press_action
            .as_ref()
            .is_some_and(|next| current.press_action.as_ref() != Some(next));
        let message = if let Some(key) = turn_key.as_deref() {
            format!(
                "{prefix}-{}: Bound turn: {}",
                index + 1,
                self.aux_binding_key_label(key)
            )
        } else if let Some(action) = press_action.as_ref() {
            format!(
                "{prefix}-{}: Bound click: {}",
                index + 1,
                self.aux_binding_action_label(action)
            )
        } else {
            format!("{prefix}-{}: Bound", index + 1)
        };
        let next_turn_key = turn_key.or(current.turn_key.clone());
        let next_press_action = press_action.or(current.press_action.clone());
        let Some(slot) = self.aux_bindings_for_bank_mut(shifted).get_mut(index) else {
            return false;
        };
        *slot = if next_turn_key.is_some() || next_press_action.is_some() {
            Some(NativeAuxBinding {
                turn_key: next_turn_key,
                press_action: next_press_action,
            })
        } else {
            None
        };
        self.show_toast(message);
        if turn_changed || click_changed {
            self.mark_fast_autosave_dirty();
        }
        true
    }

    pub(in crate::native_runner) fn set_aux_click_binding(
        &mut self,
        index: usize,
        shifted: bool,
        action: Option<super::NativeMenuAction>,
    ) {
        let Some(current) = self.aux_bindings_for_bank(shifted).get(index) else {
            return;
        };
        let current = current.clone().unwrap_or(NativeAuxBinding {
            turn_key: None,
            press_action: None,
        });
        let turn_key = current.turn_key.clone();
        *self
            .aux_bindings_for_bank_mut(shifted)
            .get_mut(index)
            .unwrap() = if turn_key.is_some() || action.is_some() {
            Some(NativeAuxBinding {
                turn_key,
                press_action: action.clone(),
            })
        } else {
            None
        };
        self.menu
            .show_aux_click_binding(index, shifted, action.as_ref());
        self.show_toast(format!(
            "Aux {}{} click mapped",
            index + 1,
            if shifted { " S+" } else { "" }
        ));
        if current.press_action != action {
            self.mark_fast_autosave_dirty();
        }
    }

    pub(super) fn set_aux_turn_binding(
        &mut self,
        index: usize,
        shifted: bool,
        binding: Option<NativeParamBinding>,
    ) {
        let Some(current) = self.aux_bindings_for_bank(shifted).get(index) else {
            return;
        };
        let current = current.clone().unwrap_or(NativeAuxBinding {
            turn_key: None,
            press_action: None,
        });
        let next_key = binding.map(|binding| binding.key);
        *self
            .aux_bindings_for_bank_mut(shifted)
            .get_mut(index)
            .unwrap() = if next_key.is_some() || current.press_action.is_some() {
            Some(NativeAuxBinding {
                turn_key: next_key.clone(),
                press_action: current.press_action.clone(),
            })
        } else {
            None
        };
        self.show_toast(format!("Mapped {}", next_key.as_deref().unwrap_or("none")));
        if current.turn_key != next_key {
            self.mark_fast_autosave_dirty();
        }
        let key = if shifted {
            format!("shiftAux:{index}:turn")
        } else {
            format!("aux:{index}:turn")
        };
        let binding = next_key.as_deref().map(aux_turn_binding_spec);
        self.menu.show_param_binding(&key, binding.as_ref());
        let _ = self.menu.focus_item_key(&key);
    }

    fn aux_bindings_for_bank(&self, shifted: bool) -> &[Option<NativeAuxBinding>] {
        if shifted {
            &self.shift_aux_bindings
        } else {
            &self.aux_bindings
        }
    }

    fn aux_bindings_for_bank_mut(&mut self, shifted: bool) -> &mut [Option<NativeAuxBinding>] {
        if shifted {
            &mut self.shift_aux_bindings
        } else {
            &mut self.aux_bindings
        }
    }
}

#[cfg(test)]
mod tests;
