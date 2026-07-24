use super::NativeRunner;

impl NativeRunner {
    pub(super) fn apply_behavior_config_menu_key_fast(&mut self, key: &str) -> Option<bool> {
        if !key.contains(".worlds.behaviorConfig.") {
            return None;
        }
        if let Err(error) = self.apply_behavior_config_menu_key(key) {
            self.show_toast(error);
            self.restore_behavior_config_menu_value(key);
        }
        Some(true)
    }
}
