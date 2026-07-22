use super::NativeRunner;

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
        self.menu.rebuild(self.menu_config());
        let _ = self.menu.focus_item_key(key);
        true
    }
}
