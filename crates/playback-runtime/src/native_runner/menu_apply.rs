use super::NativeRunner;

impl NativeRunner {
    pub(super) fn apply_behavior_selection(&mut self, behavior_id: &str) -> Result<bool, String> {
        let current_layer_behavior_id = self
            .layer_behavior_ids
            .get(self.active_layer_index)
            .cloned()
            .unwrap_or_else(|| self.behavior.id().into());
        let behavior_changed = behavior_id != self.behavior.id();
        let layer_behavior_changed = behavior_id != current_layer_behavior_id;
        if !behavior_changed && !layer_behavior_changed {
            return Ok(self.sync_active_layer_auto_name(behavior_id));
        }
        let previous_behavior_id = current_layer_behavior_id;
        let previous_config = self.behavior_config.clone();
        let behavior = platform_core::get_native_behavior(behavior_id)
            .ok_or_else(|| format!("unsupported native behavior `{behavior_id}`"))?;
        let next_config =
            self.remembered_layer_behavior_config(self.active_layer_index, behavior_id);
        self.replace_layer_engine_with_config(
            self.active_layer_index,
            behavior,
            next_config.clone(),
            None,
        )?;
        self.remember_layer_behavior_config(
            self.active_layer_index,
            &previous_behavior_id,
            previous_config,
        );
        if let Some(layer_behavior_id) = self.layer_behavior_ids.get_mut(self.active_layer_index) {
            *layer_behavior_id = behavior_id.to_string();
        }
        self.sync_active_layer_auto_name(behavior_id);
        self.remap_bindings_for_behavior_change(
            &previous_behavior_id,
            behavior_id,
            self.active_layer_index,
        );
        self.set_layer_behavior_config(self.active_layer_index, behavior_id, next_config);
        Ok(true)
    }

    pub(super) fn apply_layer_behavior_selection(
        &mut self,
        layer_index: usize,
        behavior_id: &str,
    ) -> Result<bool, String> {
        let current_behavior_id = self
            .layer_behavior_ids
            .get(layer_index)
            .cloned()
            .unwrap_or_else(|| "none".into());
        if behavior_id == current_behavior_id {
            return Ok(self.sync_layer_auto_name(layer_index, behavior_id));
        }
        let behavior = platform_core::get_native_behavior(behavior_id)
            .ok_or_else(|| format!("unsupported native behavior `{behavior_id}`"))?;
        let previous_config = self.layer_behavior_config(layer_index);
        let next_config = self.remembered_layer_behavior_config(layer_index, behavior_id);
        self.replace_layer_engine_with_config(layer_index, behavior, next_config.clone(), None)?;
        self.remember_layer_behavior_config(layer_index, &current_behavior_id, previous_config);
        if let Some(layer_behavior_id) = self.layer_behavior_ids.get_mut(layer_index) {
            *layer_behavior_id = behavior_id.to_string();
        }
        self.set_layer_behavior_config(layer_index, behavior_id, next_config);
        self.sync_layer_auto_name(layer_index, behavior_id);
        self.remap_bindings_for_behavior_change(&current_behavior_id, behavior_id, layer_index);
        Ok(true)
    }

    pub(super) fn sync_active_layer_auto_name(&mut self, behavior_id: &str) -> bool {
        self.sync_layer_auto_name(self.active_layer_index, behavior_id)
    }

    pub(super) fn sync_layer_auto_name(&mut self, layer_index: usize, behavior_id: &str) -> bool {
        if !self
            .layer_auto_names
            .get(layer_index)
            .copied()
            .unwrap_or(true)
        {
            return false;
        }
        let Some(name) = self.layer_names.get_mut(layer_index) else {
            return false;
        };
        if name == behavior_id {
            return false;
        }
        *name = behavior_id.into();
        true
    }

    pub(super) fn refresh_active_mapping_config(&mut self) {
        let mapping = self.mapping_config_for_layer(self.active_layer_index);
        self.engine.set_mapping_config(mapping.clone());
        self.mapping_config = mapping;
    }

    pub(super) fn refresh_active_interpretation_profile(&mut self) {
        self.interpretation_profile =
            self.interpretation_profile_for_layer(self.active_layer_index);
    }
}
