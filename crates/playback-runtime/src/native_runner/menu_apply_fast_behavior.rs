use super::NativeRunner;

impl NativeRunner {
    pub(super) fn apply_behavior_config_menu_key_fast(&mut self, key: &str) -> Option<bool> {
        if !key.contains(".worlds.behaviorConfig.") {
            return None;
        }
        Some(self.fast_behavior_config_key().unwrap_or(false))
    }

    fn fast_behavior_config_key(&mut self) -> Result<bool, String> {
        if self.apply_behavior_config_menu_state()? {
            self.mark_fast_autosave_dirty();
        }
        Ok(true)
    }
}
