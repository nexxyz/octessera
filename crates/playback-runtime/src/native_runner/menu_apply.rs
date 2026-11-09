use super::NativeRunner;

impl NativeRunner {
    pub(super) fn apply_behavior_selection(&mut self, behavior_id: &str) -> Result<bool, String> {
        let current_layer_behavior_id = self
            .layers
            .get(self.active_layer_index)
            .map(|layer| layer.behavior_id.clone())
            .unwrap_or_else(|| self.active_behavior().id().into());
        let behavior_changed = behavior_id != self.active_behavior().id();
        let layer_behavior_changed = behavior_id != current_layer_behavior_id;
        if !behavior_changed && !layer_behavior_changed {
            return Ok(self.sync_active_layer_auto_name(behavior_id));
        }
        let previous_behavior_id = current_layer_behavior_id;
        let previous_config = self.active_layer().behavior_config.clone();
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
        if let Some(layer) = self.layers.get_mut(self.active_layer_index) {
            layer.behavior_id = behavior_id.to_string();
        }
        self.sync_active_layer_auto_name(behavior_id);
        self.remap_bindings_for_behavior_change(behavior_id, self.active_layer_index);
        self.set_layer_behavior_config(self.active_layer_index, behavior_id, next_config);
        Ok(true)
    }

    pub(super) fn apply_layer_behavior_selection(
        &mut self,
        layer_index: usize,
        behavior_id: &str,
    ) -> Result<bool, String> {
        let current_behavior_id = self
            .layers
            .get(layer_index)
            .map(|layer| layer.behavior_id.clone())
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
        if let Some(layer) = self.layers.get_mut(layer_index) {
            layer.behavior_id = behavior_id.to_string();
        }
        self.set_layer_behavior_config(layer_index, behavior_id, next_config);
        self.sync_layer_auto_name(layer_index, behavior_id);
        self.remap_bindings_for_behavior_change(behavior_id, layer_index);
        Ok(true)
    }

    pub(super) fn sync_active_layer_auto_name(&mut self, behavior_id: &str) -> bool {
        self.sync_layer_auto_name(self.active_layer_index, behavior_id)
    }

    pub(super) fn sync_layer_auto_name(&mut self, layer_index: usize, behavior_id: &str) -> bool {
        let Some(layer) = self.layers.get_mut(layer_index) else {
            return false;
        };
        layer.auto_name
            && super::menu_apply_fast::value_changed(
                &mut layer.name,
                super::behavior_menu::behavior_display_label(behavior_id),
            )
    }

    pub(super) fn refresh_active_mapping_config(&mut self) {
        let mapping = self.mapping_config_for_layer(self.active_layer_index);
        self.active_engine_mut().set_mapping_config(mapping);
    }

    pub(super) fn refresh_active_interpretation_profile(&mut self) {
        let profile = self.interpretation_profile_for_layer(self.active_layer_index);
        self.active_engine_mut().set_interpretation_profile(profile);
    }
}
