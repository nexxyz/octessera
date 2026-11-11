use super::{NativeRunner, TriggerProbabilityMode};

impl NativeRunner {
    pub(super) fn apply_trigger_gate_mode_to_all_layers(&mut self, mode: TriggerProbabilityMode) {
        for layer in &mut self.layers {
            layer.link.trigger_probability_mode = mode;
            layer.trigger_gate_restore_mode = None;
        }
    }

    pub(super) fn apply_trigger_gate_mode_to_layer(
        &mut self,
        layer_index: usize,
        mode: TriggerProbabilityMode,
    ) {
        if let Some(layer) = self.layers.get_mut(layer_index) {
            layer.link.trigger_probability_mode = mode;
            if mode != TriggerProbabilityMode::Zero {
                layer.trigger_gate_restore_mode = None;
            }
        }
    }
}
