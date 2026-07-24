use super::NativeRunner;
use crate::native_menu::NativeMenuItem;

impl NativeRunner {
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
