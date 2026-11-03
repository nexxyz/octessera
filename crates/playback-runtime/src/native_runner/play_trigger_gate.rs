use super::NativeRunner;

impl NativeRunner {
    pub(super) fn active_layer_context_toast(&self, index: usize) -> String {
        let label = self
            .layer_labels()
            .get(index)
            .cloned()
            .unwrap_or_else(|| format!("L{}", index + 1));
        format!("Layer: {}", label.replace(": ", " "))
    }

    pub(super) fn apply_trigger_gate_mode_to_all_layers(&mut self, mode: &str) {
        for layer_mode in &mut self.trigger_gate_modes {
            *layer_mode = mode.into();
        }
        for layer in &mut self.link_layers {
            layer.trigger_probability_mode = mode.into();
        }
        self.trigger_gate_restore_modes.fill(None);
    }

    pub(super) fn apply_trigger_gate_mode_to_layer(&mut self, layer_index: usize, mode: &str) {
        if let Some(layer_mode) = self.trigger_gate_modes.get_mut(layer_index) {
            *layer_mode = mode.into();
        }
        if let Some(layer) = self.link_layers.get_mut(layer_index) {
            layer.trigger_probability_mode = mode.into();
        }
        if mode != "zero" {
            if let Some(restore) = self.trigger_gate_restore_modes.get_mut(layer_index) {
                *restore = None;
            }
        }
    }
}

pub(super) fn trigger_gate_mode_for_column(x: usize) -> Option<&'static str> {
    match x {
        0 => Some("zero"),
        1 => Some("custom"),
        2 => Some("full"),
        6 => Some("custom"),
        7 => Some("full"),
        _ => None,
    }
}
