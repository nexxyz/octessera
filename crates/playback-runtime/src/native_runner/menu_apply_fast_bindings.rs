use super::{param_binding_spec_from_native, NativeRunner};

impl NativeRunner {
    pub(super) fn apply_binding_range_key_fast(&mut self, key: &str) -> bool {
        let Some((target, suffix)) = key.rsplit_once('.') else {
            return false;
        };
        let is_min = match suffix {
            "rangeMin" => true,
            "rangeMax" => false,
            _ => return false,
        };
        let Some(value) = self.menu.number_for_key(key) else {
            return false;
        };
        self.set_param_binding_range_value(target, is_min, value);
        self.show_param_binding_target(target);
        true
    }

    pub(super) fn show_param_binding_target(&mut self, target: &str) {
        let binding = self
            .param_binding_target_mut(target)
            .map(|binding| param_binding_spec_from_native(binding));
        self.menu.show_param_binding(target, binding.as_ref());
    }
}
