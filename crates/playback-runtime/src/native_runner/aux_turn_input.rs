use super::NativeRunner;
use crate::native_menu::format_values::format_display_value;
use crate::native_menu::{NativeMenuItem, NativeMenuValue};

impl NativeRunner {
    pub(super) fn handle_aux_turn(&mut self, index: usize, delta: i8) -> Result<(), String> {
        if delta == 0 {
            return Ok(());
        }
        let shifted = self.display.ui.shift_held || self.display.ui.combined_modifier_held;
        let prefix = if shifted { "S+Trn" } else { "Trn" };
        let binding = self.effective_aux_slot(index);
        let Some(turn) = binding.turn else {
            self.show_toast(format!("{prefix}-{}: No binding", index + 1));
            return Ok(());
        };
        let coarse = !(self.display.ui.fn_held || self.display.ui.combined_modifier_held);
        match self.turn_bound_key(&turn.key, &turn.label, delta, coarse) {
            Ok(Some((label, value))) => {
                self.show_or_queue_aux_turn_toast(
                    format!("{prefix}-{}: {label}: ", index + 1),
                    value,
                );
            }
            Ok(None) => {
                self.show_toast(format!("{prefix}-{}: {} not active", index + 1, turn.label));
            }
            Err(error) => {
                self.show_toast(error.clone());
                return Err(error);
            }
        }
        Ok(())
    }

    /// Turns a bound key one detent like an Aux turn: its label and new value, `None` when no
    /// row turns it.
    pub(super) fn turn_bound_key(
        &mut self,
        key: &str,
        label: &str,
        delta: i8,
        coarse: bool,
    ) -> Result<Option<(String, String)>, String> {
        if let Some(value) = self.turn_generated_behavior_target(key, delta, coarse)? {
            return Ok(Some((label.into(), format_display_value(Some(key), value))));
        }
        if self.menu.turn_key_with_precision(key, delta, coarse) {
            self.apply_or_schedule_menu_key(key)?;
            let value = self
                .menu
                .value_for_key(key)
                .or_else(|| self.menu.number_for_key(key).map(|value| value.to_string()))
                .map(|value| format_display_value(Some(key), value))
                .unwrap_or_else(|| "changed".into());
            return Ok(Some((label.into(), value)));
        }
        Ok(self
            .hidden_delay_time_item(key)
            .map(|hidden| (hidden.label.clone(), hidden_row_value(&hidden))))
    }
}

/// A turn on a hidden delay time row reports its stored value: in note mode Time follows the
/// note, so there is nothing for the turn to change.
fn hidden_row_value(item: &NativeMenuItem) -> String {
    let value = match &item.value {
        NativeMenuValue::Number { value, .. } => value.to_string(),
        NativeMenuValue::Enum { options, selected } => {
            options.get(*selected).cloned().unwrap_or_default()
        }
        _ => return String::new(),
    };
    format_display_value(item.key.as_deref(), value)
}
