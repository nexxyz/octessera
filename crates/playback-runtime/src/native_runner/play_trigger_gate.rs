use super::{NativeRunner, TriggerProbabilityMode};

impl NativeRunner {
    pub(super) fn apply_trigger_gate_mode_to_all_layers(&mut self, mode: TriggerProbabilityMode) {
        self.trigger_gate_modes.fill(mode);
        for layer in &mut self.link_layers {
            layer.trigger_probability_mode = mode;
        }
        self.trigger_gate_restore_modes.fill(None);
    }

    pub(super) fn apply_trigger_gate_mode_to_layer(
        &mut self,
        layer_index: usize,
        mode: TriggerProbabilityMode,
    ) {
        if let Some(layer_mode) = self.trigger_gate_modes.get_mut(layer_index) {
            *layer_mode = mode;
        }
        if let Some(layer) = self.link_layers.get_mut(layer_index) {
            layer.trigger_probability_mode = mode;
        }
        if mode != TriggerProbabilityMode::Zero {
            if let Some(restore) = self.trigger_gate_restore_modes.get_mut(layer_index) {
                *restore = None;
            }
        }
    }
}
