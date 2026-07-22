use crate::timing_units::note_unit_to_pulses;

use super::NativeRunner;

impl NativeRunner {
    pub(super) fn apply_layer_menu_key_fast(&mut self, key: &str) -> Option<bool> {
        let rest = key.strip_prefix("layers.")?;
        let (index, suffix) = super::menu_apply_fast_values::parse_indexed_key(rest)?;
        match suffix {
            "algorithmStep" => Some(self.fast_layer_algorithm_step_key(index, key)),
            "autoName" => Some(self.fast_layer_auto_name_key(index, key)),
            _ => None,
        }
    }

    fn fast_layer_algorithm_step_key(&mut self, index: usize, key: &str) -> bool {
        let Some(value) = self.menu.value_for_key(key) else {
            return false;
        };
        let Some(layer_step) = self.transport.layer_algorithm_step_pulses.get_mut(index) else {
            return false;
        };
        let pulses = note_unit_to_pulses(&value);
        if *layer_step == pulses {
            return false;
        }
        *layer_step = pulses;
        if index == self.active_layer_index {
            self.transport.algorithm_step_pulses = pulses;
        }
        true
    }

    fn fast_layer_auto_name_key(&mut self, index: usize, key: &str) -> bool {
        let Some(auto_name) = self.menu.value_for_key(key).map(|value| value == "true") else {
            return false;
        };
        let Some(target) = self.layer_auto_names.get_mut(index) else {
            return false;
        };
        let mut changed = false;
        if *target != auto_name {
            *target = auto_name;
            changed = true;
        }
        if auto_name {
            let behavior_id = self
                .layer_behavior_ids
                .get(index)
                .cloned()
                .unwrap_or_else(|| self.behavior.id().into());
            if let Some(name) = self.layer_names.get_mut(index) {
                changed |= super::menu_apply_fast::value_changed(name, behavior_id);
            }
        }
        if changed {
            self.mark_fast_autosave_dirty();
        }
        true
    }
}
