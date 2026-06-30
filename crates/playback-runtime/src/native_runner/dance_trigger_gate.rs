use super::{NativeRunner, NativeToast};

impl NativeRunner {
    pub(super) fn select_dance_page_from_fn_grid(&mut self, y: usize) {
        let next_mode = match y {
            0 => Some("mix"),
            1 => Some("pan"),
            2 => Some("fx"),
            3 => Some("trigger-gate"),
            4 => Some("xy"),
            _ => None,
        };
        let Some(next_mode) = next_mode else {
            return;
        };
        self.dance_mode = next_mode.into();
        self.active_dance_mode = self.dance_mode.clone();
        self.menu.state.stack = vec![3];
        self.menu.state.cursor = 0;
        self.menu.state.editing = false;
        self.menu.rebuild(self.menu_config());
        self.show_toast(format!("Dance: {}", dance_mode_label(next_mode)));
    }

    pub(super) fn active_part_context_toast(&self, index: usize) -> String {
        let label = self
            .part_labels()
            .get(index)
            .cloned()
            .unwrap_or_else(|| format!("P{}", index + 1));
        format!("Part: {}", label.replace(": ", " "))
    }

    pub(super) fn apply_trigger_gate_mode_to_all_parts(&mut self, mode: &str) {
        for part_mode in &mut self.trigger_gate_modes {
            *part_mode = mode.into();
        }
        for part in &mut self.sense_parts {
            part.trigger_probability_mode = mode.into();
        }
        for restore in &mut self.trigger_gate_restore_modes {
            *restore = None;
        }
        self.activate_trigger_gate_part_with_toast(self.active_part_index);
    }

    pub(super) fn apply_trigger_gate_mode_to_part(&mut self, part_index: usize, mode: &str) {
        if let Some(part_mode) = self.trigger_gate_modes.get_mut(part_index) {
            *part_mode = mode.into();
        }
        if let Some(part) = self.sense_parts.get_mut(part_index) {
            part.trigger_probability_mode = mode.into();
        }
        if let Some(restore) = self.trigger_gate_restore_modes.get_mut(part_index) {
            *restore = None;
        }
        if part_index == self.active_part_index {
            self.activate_trigger_gate_part_with_toast(part_index);
        }
    }

    pub(super) fn activate_trigger_gate_part_with_toast(&mut self, part_index: usize) {
        if let Err(error) = self.activate_engine(part_index) {
            self.toast = Some(NativeToast {
                message: error,
                offset: 0,
            });
        }
    }
}

fn dance_mode_label(mode: &str) -> &str {
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
