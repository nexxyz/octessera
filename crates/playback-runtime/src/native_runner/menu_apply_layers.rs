use super::NativeRunner;

impl NativeRunner {
    pub(super) fn apply_layer_menu_state(&mut self) -> bool {
        let mut changed = false;
        for index in 0..self.layer_auto_names.len() {
            let before_name = self.layer_names.get(index).cloned().unwrap_or_default();
            let Some(auto_name) = self
                .menu
                .value_for_key(&format!("layers.{index}.autoName"))
                .map(|value| value == "true")
            else {
                continue;
            };
            if self.layer_auto_names[index] != auto_name {
                self.layer_auto_names[index] = auto_name;
                if auto_name {
                    let behavior_id = self
                        .layer_behavior_ids
                        .get(index)
                        .cloned()
                        .unwrap_or_else(|| self.behavior.id().into());
                    if let Some(name) = self.layer_names.get_mut(index) {
                        *name = behavior_id;
                    }
                }
                changed = true;
            }
            let name_key = format!("layers.{index}.name");
            if self.menu.current_key() == Some(name_key.as_str()) {
                if let Some(name) = self.menu.value_for_key(&name_key) {
                    if name != before_name {
                        if let Some(target) = self.layer_names.get_mut(index) {
                            *target = name;
                        }
                        if let Some(auto_name) = self.layer_auto_names.get_mut(index) {
                            *auto_name = false;
                        }
                        changed = true;
                    }
                }
            }
            if self.layer_auto_names[index] {
                let behavior_id = self
                    .layer_behavior_ids
                    .get(index)
                    .cloned()
                    .unwrap_or_else(|| self.behavior.id().into());
                if self.layer_names.get(index) != Some(&behavior_id) {
                    if let Some(target) = self.layer_names.get_mut(index) {
                        *target = behavior_id;
                    }
                    changed = true;
                }
            }
        }
        changed
    }
}
