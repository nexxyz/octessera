use super::NativeRunner;

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
            Ok(Some(value)) => self.show_or_queue_aux_turn_toast(format!(
                "{prefix}-{}: {}: {value}",
                index + 1,
                turn.label
            )),
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
                self.show_or_queue_aux_turn_toast(format!(
                    "{prefix}-{}: {}: {value}",
                    index + 1,
                    turn.label
                ));
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
}
