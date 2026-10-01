use super::super::{NativeAuxBinding, NativeParamBinding, NativeRunner, NativeToast};

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
        let bank = if shifted {
            "shiftAuxBindings"
        } else {
            "auxBindings"
        };
        let Some(current) = self.aux_bindings_for_bank(shifted).get(index) else {
            return false;
        };
        let current = current.clone().unwrap_or(NativeAuxBinding {
            turn_key: None,
            press_action: None,
        });
        let old_turn_owner = current
            .turn_key
            .as_deref()
            .map(super::super::patch_device_payload::is_musical_aux_turn_key);
        let old_click_owner = current
            .press_action
            .as_ref()
            .map(super::super::patch_device_payload::is_musical_aux_click_action);
        let turn_changed = turn_key
            .as_ref()
            .is_some_and(|next| current.turn_key.as_ref() != Some(next));
        let click_changed = press_action
            .as_ref()
            .is_some_and(|next| current.press_action.as_ref() != Some(next));
        let next_turn_owner = turn_key
            .as_deref()
            .map(super::super::patch_device_payload::is_musical_aux_turn_key);
        let next_click_owner = press_action
            .as_ref()
            .map(super::super::patch_device_payload::is_musical_aux_click_action);
        if (turn_changed
            && !self.aux_side_change_allowed(
                bank,
                index,
                "turnKey",
                old_turn_owner,
                next_turn_owner,
            ))
            || (click_changed
                && !self.aux_side_change_allowed(
                    bank,
                    index,
                    "pressAction",
                    old_click_owner,
                    next_click_owner,
                ))
        {
            self.show_toast("Clear and save this Aux side before changing owner");
            return false;
        }
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
        let patch_changed = (turn_changed
            && (old_turn_owner == Some(true) || next_turn_owner == Some(true)))
            || (click_changed && (old_click_owner == Some(true) || next_click_owner == Some(true)));
        let system_changed = (turn_changed
            && (old_turn_owner == Some(false) || next_turn_owner == Some(false)))
            || (click_changed
                && (old_click_owner == Some(false) || next_click_owner == Some(false)));
        self.mark_aux_domains_dirty(patch_changed, system_changed);
        true
    }

    pub(in crate::native_runner) fn set_aux_click_binding(
        &mut self,
        index: usize,
        shifted: bool,
        action: Option<super::NativeMenuAction>,
    ) {
        let bank = if shifted {
            "shiftAuxBindings"
        } else {
            "auxBindings"
        };
        let Some(current) = self.aux_bindings_for_bank(shifted).get(index) else {
            return;
        };
        let current = current.clone().unwrap_or(NativeAuxBinding {
            turn_key: None,
            press_action: None,
        });
        let turn_key = current.turn_key.clone();
        let old_owner = current
            .press_action
            .as_ref()
            .map(super::super::patch_device_payload::is_musical_aux_click_action);
        let new_owner = action
            .as_ref()
            .map(super::super::patch_device_payload::is_musical_aux_click_action);
        if current.press_action != action
            && !self.aux_side_change_allowed(bank, index, "pressAction", old_owner, new_owner)
        {
            self.show_toast("Clear and save this Aux side before changing owner");
            return;
        }
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
        self.display.toast = Some(NativeToast {
            message: format!(
                "Aux {}{} click mapped",
                index + 1,
                if shifted { " S+" } else { "" }
            ),
            offset: 0,
        });
        if current.press_action != action {
            self.mark_aux_domains_dirty(
                old_owner == Some(true) || new_owner == Some(true),
                old_owner == Some(false) || new_owner == Some(false),
            );
        }
    }

    pub(super) fn set_aux_turn_binding(
        &mut self,
        index: usize,
        shifted: bool,
        binding: Option<NativeParamBinding>,
    ) {
        let bank = if shifted {
            "shiftAuxBindings"
        } else {
            "auxBindings"
        };
        let Some(current) = self.aux_bindings_for_bank(shifted).get(index) else {
            return;
        };
        let current = current.clone().unwrap_or(NativeAuxBinding {
            turn_key: None,
            press_action: None,
        });
        let next_key = binding.map(|binding| binding.key);
        let old_owner = current
            .turn_key
            .as_deref()
            .map(super::super::patch_device_payload::is_musical_aux_turn_key);
        let new_owner = next_key
            .as_deref()
            .map(super::super::patch_device_payload::is_musical_aux_turn_key);
        if current.turn_key != next_key
            && !self.aux_side_change_allowed(bank, index, "turnKey", old_owner, new_owner)
        {
            self.show_toast("Clear and save this Aux side before changing owner");
            return;
        }
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
            self.mark_aux_domains_dirty(
                old_owner == Some(true) || new_owner == Some(true),
                old_owner == Some(false) || new_owner == Some(false),
            );
        }
        self.menu.rebuild(self.menu_config());
        let key = if shifted {
            format!("shiftAux:{index}:turn")
        } else {
            format!("aux:{index}:turn")
        };
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

    fn aux_side_change_allowed(
        &self,
        bank: &str,
        index: usize,
        side: &str,
        old_owner: Option<bool>,
        new_owner: Option<bool>,
    ) -> bool {
        if old_owner == new_owner || new_owner.is_none() {
            return true;
        }
        if old_owner.is_some() {
            return false;
        }
        let slot = format!("aux{}", index + 1);
        match new_owner {
            Some(true) => {
                !self.pending.system_persistence.has_pending_request()
                    && !self
                        .pending
                        .system_persistence
                        .has_saved_aux_side(bank, index, side)
            }
            Some(false) => {
                !self.restart_settings.has_pending_patch_write()
                    && self.pending.pending_save_revision.is_none()
                    && self
                        .pending
                        .saved_patch_baseline
                        .as_deref()
                        .and_then(|patch| patch.get("runtimeConfig"))
                        .and_then(|runtime| runtime.get(bank))
                        .and_then(|bindings| bindings.get(slot.as_str()))
                        .and_then(|binding| binding.get(side))
                        .is_none_or(|value| value.is_null())
            }
            None => true,
        }
    }

    fn mark_aux_domains_dirty(&mut self, patch_changed: bool, system_changed: bool) {
        if patch_changed {
            self.mark_fast_autosave_dirty();
        }
        if system_changed {
            self.mark_system_dirty();
        }
    }
}

#[cfg(test)]
#[path = "aux_binding_ownership_tests.rs"]
mod tests;
