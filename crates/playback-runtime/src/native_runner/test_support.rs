use super::{NativeOledMode, NativeRunner, Value};
use crate::native_menu::NativeMenuItem;
use std::time::{Duration, Instant};

impl NativeRunner {
    pub fn test_config_payload(&self) -> Value {
        self.config_payload()
    }

    pub fn test_device_config_payload(payload: Value) -> Result<Value, String> {
        super::device_config_payload_from_payload(payload)
    }

    pub fn test_portable_patch_bytes(payload: &Value) -> Result<Vec<u8>, String> {
        super::portable_patch_bytes(payload)
    }

    pub fn test_confirmation_is_open(&self) -> bool {
        self.display.confirm_dialog.is_some()
    }

    pub fn test_focus_menu_item(&mut self, key: &str) -> Result<String, String> {
        let label = find_menu_item(&self.menu.root, key)
            .map(|item| item.label.clone())
            .ok_or_else(|| format!("native menu item key not found: {key}"))?;
        if !self.menu.focus_item_key(key) {
            return Err(format!("native menu item key could not be focused: {key}"));
        }
        Ok(label)
    }

    pub fn test_current_menu_label(&self) -> Option<String> {
        self.menu.current_label().map(str::to_owned)
    }

    pub fn test_current_menu_path(&self) -> String {
        self.menu.current_focus_path()
    }

    pub fn test_menu_cursor(&self) -> usize {
        self.menu.state.cursor
    }

    pub fn test_set_display_time(&mut self, now: Instant) {
        self.display.transients.set_test_now(now);
    }

    pub fn test_advance_display_time(&mut self, elapsed: Duration) {
        let now = self.display.transients.now() + elapsed;
        self.display.transients.set_test_now(now);
    }

    pub fn test_set_oled_off(&mut self) {
        self.display.oled_mode = NativeOledMode::Off;
        self.display.oled_splash_text.clear();
        self.display.oled_splash_until = None;
    }

    pub fn test_fail_next_snapshot(&self) {
        self.test_snapshot_failure.set(true);
    }
}

fn find_menu_item<'a>(item: &'a NativeMenuItem, key: &str) -> Option<&'a NativeMenuItem> {
    if item.key.as_deref() == Some(key) {
        return Some(item);
    }
    item.children
        .iter()
        .find_map(|child| find_menu_item(child, key))
}

#[cfg(test)]
mod tests {
    use crate::{NativeRunner, NativeRunnerConfig};

    #[test]
    fn focuses_update_item_by_stable_key() {
        let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();

        assert_eq!(
            runner.test_focus_menu_item("system.updateApply").unwrap(),
            "Apply"
        );
        assert_eq!(runner.menu.current_key(), Some("system.updateApply"));
    }
}
