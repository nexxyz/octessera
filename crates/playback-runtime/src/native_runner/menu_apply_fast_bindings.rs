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
        if let Some(binding) = self.param_binding_target_mut(target) {
            let binding = param_binding_spec_from_native(binding);
            self.menu.show_param_binding_rows(target, &binding);
        }
        true
    }
}
