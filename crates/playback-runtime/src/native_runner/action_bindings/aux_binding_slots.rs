use super::super::toast_text::bank_head;
use super::super::{aux_turn_binding_spec, NativeAuxBinding, NativeParamBinding, NativeRunner};

impl NativeRunner {
    pub(super) fn bind_aux_from_current(&mut self, index: usize, shifted: bool) -> bool {
        let (turn_key, press_action) = self.menu.current_binding_target();
        let press_action = press_action.map(|action| self.bound_click_action(action));
        if turn_key
            .as_deref()
            .is_some_and(super::inert_sample_binding_key)
        {
            self.show_toast(super::BindingValidationError::UnsupportedTarget.toast_message());
            return false;
        }
        let message = match (turn_key.as_deref(), press_action.as_ref()) {
            (Some(key), _) => {
                self.aux_bound_toast("Trn", index, shifted, self.aux_binding_key_label(key))
            }
            (None, Some(action)) => {
                self.aux_bound_toast("Clk", index, shifted, self.aux_binding_action_label(action))
            }
            (None, None) => {
                self.show_toast(format!(
                    "{}: No binding",
                    bank_head(shifted, &format!("Aux-{}", index + 1))
                ));
                return false;
            }
        };
        let Some(current) = self.aux_binding(index, shifted) else {
            return false;
        };
        let turn_changed = turn_key
            .as_ref()
            .is_some_and(|next| current.turn_key.as_ref() != Some(next));
        let click_changed = press_action
            .as_ref()
            .is_some_and(|next| current.press_action.as_ref() != Some(next));
        let next = NativeAuxBinding {
            turn_key: turn_key.or(current.turn_key),
            press_action: press_action.or(current.press_action),
        };
        self.store_aux_binding(index, shifted, next.clone());
        self.show_toast(message);
        if turn_changed {
            self.show_aux_turn_picker(index, shifted, next.turn_key.as_deref());
        }
        if click_changed {
            self.menu
                .show_aux_click_binding(index, shifted, next.press_action.as_ref());
        }
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
        let action = action.map(|action| self.bound_click_action(action));
        let Some(current) = self.aux_binding(index, shifted) else {
            return;
        };
        let label = action
            .as_ref()
            .map(|action| self.aux_binding_action_label(action));
        let changed = current.press_action != action;
        self.store_aux_binding(
            index,
            shifted,
            NativeAuxBinding {
                turn_key: current.turn_key,
                press_action: action.clone(),
            },
        );
        self.menu
            .show_aux_click_binding(index, shifted, action.as_ref());
        self.show_toast(self.aux_bound_toast_or_cleared("Clk", index, shifted, label));
        if changed {
            self.mark_fast_autosave_dirty();
        }
    }

    pub(super) fn set_aux_turn_binding(
        &mut self,
        index: usize,
        shifted: bool,
        binding: Option<NativeParamBinding>,
    ) {
        let Some(current) = self.aux_binding(index, shifted) else {
            return;
        };
        let next_key = binding.map(|binding| binding.key);
        let label = next_key
            .as_deref()
            .map(|key| self.aux_binding_key_label(key));
        let changed = current.turn_key != next_key;
        self.store_aux_binding(
            index,
            shifted,
            NativeAuxBinding {
                turn_key: next_key.clone(),
                press_action: current.press_action,
            },
        );
        self.show_toast(self.aux_bound_toast_or_cleared("Trn", index, shifted, label));
        if changed {
            self.mark_fast_autosave_dirty();
        }
        let picker = self.show_aux_turn_picker(index, shifted, next_key.as_deref());
        let _ = self.menu.focus_item_key(&picker);
    }

    fn aux_bound_toast(&self, control: &str, index: usize, shifted: bool, label: String) -> String {
        format!(
            "{}: Bound {label}",
            bank_head(shifted, &format!("{control}-{}", index + 1))
        )
    }

    fn aux_bound_toast_or_cleared(
        &self,
        control: &str,
        index: usize,
        shifted: bool,
        label: Option<String>,
    ) -> String {
        match label {
            Some(label) => self.aux_bound_toast(control, index, shifted, label),
            None => format!(
                "{}: Cleared",
                bank_head(shifted, &format!("{control}-{}", index + 1))
            ),
        }
    }

    /// Shows the turn binding on its Link > Aux Mappings picker; returns the picker key.
    fn show_aux_turn_picker(&mut self, index: usize, shifted: bool, key: Option<&str>) -> String {
        let picker = if shifted {
            format!("shiftAux:{index}:turn")
        } else {
            format!("aux:{index}:turn")
        };
        let binding = key.map(aux_turn_binding_spec);
        self.menu.show_param_binding(&picker, binding.as_ref());
        picker
    }

    /// Aux `index`'s binding in its bank, empty when nothing is bound.
    fn aux_binding(&self, index: usize, shifted: bool) -> Option<NativeAuxBinding> {
        let slot = self.aux_bindings_for_bank(shifted).get(index)?;
        Some(slot.clone().unwrap_or(NativeAuxBinding {
            turn_key: None,
            press_action: None,
        }))
    }

    /// Stores Aux `index`'s binding; a binding with neither a turn nor a click empties the slot.
    fn store_aux_binding(&mut self, index: usize, shifted: bool, binding: NativeAuxBinding) {
        if let Some(slot) = self.aux_bindings_for_bank_mut(shifted).get_mut(index) {
            *slot =
                (binding.turn_key.is_some() || binding.press_action.is_some()).then_some(binding);
        }
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
