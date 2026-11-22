use super::{NativeRunner, TriggerProbabilityMode};

impl NativeRunner {
    pub(super) fn apply_trigger_gate_mode_to_all_layers(&mut self, mode: TriggerProbabilityMode) {
        for layer_index in 0..self.layers.len() {
            self.apply_trigger_gate_mode_to_layer(layer_index, mode);
            self.layers[layer_index].trigger_gate_restore_mode = None;
        }
    }

    /// A gate that closes ends the layer's notes and clears what its Arp holds.
    pub(super) fn apply_trigger_gate_mode_to_layer(
        &mut self,
        layer_index: usize,
        mode: TriggerProbabilityMode,
    ) {
        let Some(layer) = self.layers.get_mut(layer_index) else {
            return;
        };
        let closes = mode == TriggerProbabilityMode::Zero
            && layer.link.trigger_probability_mode != TriggerProbabilityMode::Zero;
        layer.link.trigger_probability_mode = mode;
        if mode != TriggerProbabilityMode::Zero {
            layer.trigger_gate_restore_mode = None;
        }
        if closes {
            self.drain_layer_owned_notes(layer_index);
        }
    }
}
