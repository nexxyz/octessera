use super::NativeRunner;
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
        match self.turn_generated_behavior_target(&turn.key, delta, coarse) {
            Ok(Some(value)) => self.show_or_queue_aux_turn_toast(
                format!("{prefix}-{}: {}: ", index + 1, turn.label),
                value.to_string(),
            ),
            Ok(None) if self.menu.turn_key_with_precision(&turn.key, delta, coarse) => {
                self.apply_or_schedule_menu_key(&turn.key)?;
                let value = self
                    .menu
                    .value_for_key(&turn.key)
                    .or_else(|| {
                        self.menu
                            .number_for_key(&turn.key)
                            .map(|value| value.to_string())
                    })
                    .unwrap_or_else(|| "changed".into());
                self.show_or_queue_aux_turn_toast(
                    format!("{prefix}-{}: {}: ", index + 1, turn.label),
                    value,
                );
            }
            Ok(None) => match self.hidden_delay_time_item(&turn.key) {
                Some(hidden) => self.show_or_queue_aux_turn_toast(
                    format!("{prefix}-{}: {}: ", index + 1, hidden.label),
                    hidden_row_value(&hidden),
                ),
                None => {
                    self.show_toast(format!("{prefix}-{}: {} not active", index + 1, turn.label));
                }
            },
            Err(error) => {
                self.show_toast(error.clone());
                return Err(error);
            }
        }
        Ok(())
    }
}

/// A turn on a hidden delay time row reports its stored value: in note mode Time ms follows the
/// note, so there is nothing for the turn to change.
fn hidden_row_value(item: &NativeMenuItem) -> String {
    match &item.value {
        NativeMenuValue::Number { value, .. } => value.to_string(),
        NativeMenuValue::Enum { options, selected } => {
            options.get(*selected).cloned().unwrap_or_default()
        }
        _ => String::new(),
    }
}
