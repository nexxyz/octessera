use super::NativeRunner;

impl NativeRunner {
    pub(super) fn select_sparks_page_from_fn_grid(&mut self, y: usize) {
        let next_mode = match y {
            0 => Some(("mix", 0)),
            1 => Some(("pan", 1)),
            2 => Some(("fx", 2)),
            3 => Some(("trigger-gate", 3)),
            4 => Some(("transpose", 4)),
            5 => Some(("xy", 5)),
            _ => None,
        };
        let Some((next_mode, cursor)) = next_mode else {
            return;
        };
        self.sparks_mode = next_mode.into();
        self.active_sparks_mode = self.sparks_mode.clone();
        self.menu.state.stack = vec![3];
        self.menu.state.cursor = cursor;
        self.menu.state.editing = false;
        self.show_toast(format!("Play: {}", sparks_mode_label(next_mode)));
    }

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
        for layer in &mut self.pulses_layers {
            layer.trigger_probability_mode = mode.into();
        }
        for restore in &mut self.trigger_gate_restore_modes {
            *restore = None;
        }
    }

    pub(super) fn apply_trigger_gate_mode_to_layer(&mut self, layer_index: usize, mode: &str) {
        if let Some(layer_mode) = self.trigger_gate_modes.get_mut(layer_index) {
            *layer_mode = mode.into();
        }
        if let Some(layer) = self.pulses_layers.get_mut(layer_index) {
            layer.trigger_probability_mode = mode.into();
        }
        if let Some(restore) = self.trigger_gate_restore_modes.get_mut(layer_index) {
            *restore = None;
        }
    }
}

fn sparks_mode_label(mode: &str) -> &str {
    match mode {
        "trigger-gate" => "trig",
        other => other,
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
