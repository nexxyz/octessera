use super::{derive_instrument_name, NativeRunner};

impl NativeRunner {
    pub(super) fn commit_behavior_structural_draft(&mut self) -> Result<(), String> {
        let Some(behavior_id) = self.menu.selected_behavior().map(|value| value.to_string()) else {
            return Ok(());
        };
        let current_part_behavior_id = self
            .part_behavior_ids
            .get(self.active_part_index)
            .cloned()
            .unwrap_or_else(|| self.behavior.id().into());
        let behavior_changed = behavior_id.as_str() != self.behavior.id();
        let part_behavior_changed = behavior_id != current_part_behavior_id;
        if !behavior_changed && !part_behavior_changed {
            if self.sync_active_part_auto_name(&behavior_id) {
                self.menu.rebuild(self.menu_config());
                self.mark_fast_autosave_dirty();
            }
            return Ok(());
        }
        let previous_behavior_id = current_part_behavior_id;
        self.behavior_configs
            .insert(self.behavior.id().to_string(), self.behavior_config.clone());
        if let Some(config) = self.part_behavior_configs.get_mut(self.active_part_index) {
            *config = self.behavior_config.clone();
        }
        let behavior = platform_core::get_native_behavior(&behavior_id)
            .ok_or_else(|| format!("unsupported native behavior `{behavior_id}`"))?;
        self.behavior_config = self
            .part_behavior_configs
            .get(self.active_part_index)
            .filter(|config| !config.is_null())
            .cloned()
            .or_else(|| self.behavior_configs.get(&behavior_id).cloned())
            .unwrap_or(serde_json::Value::Null);
        self.behavior_configs
            .insert(behavior_id.clone(), self.behavior_config.clone());
        if let Some(config) = self.part_behavior_configs.get_mut(self.active_part_index) {
            *config = self.behavior_config.clone();
        }
        if let Some(part_behavior_id) = self.part_behavior_ids.get_mut(self.active_part_index) {
            *part_behavior_id = behavior_id.clone();
        }
        self.sync_active_part_auto_name(&behavior_id);
        self.remap_bindings_for_behavior_change(
            &previous_behavior_id,
            &behavior_id,
            self.active_part_index,
        );
        if behavior_changed {
            self.engine = Self::build_engine(
                behavior,
                self.behavior_config.clone(),
                self.interpretation_profile.clone(),
                self.mapping_config.clone(),
                self.global_sound.clone(),
                self.note_behaviors.clone(),
                self.active_part_index,
            )?;
            self.behavior = behavior;
        }
        self.menu.rebuild(self.menu_config());
        self.mark_fast_autosave_dirty();
        Ok(())
    }

    pub(super) fn commit_instrument_type_structural_draft(&mut self, index: usize) {
        let Some(kind) = self
            .menu
            .value_for_key(&format!("instruments.{index}.type"))
        else {
            return;
        };
        let Some(instrument) = self.instruments.get_mut(index) else {
            return;
        };
        if instrument.kind == kind {
            return;
        }
        instrument.kind = kind;
        if instrument.auto_name {
            instrument.name = derive_instrument_name(index, &instrument.kind);
        }
        self.audio_config_revision = self.audio_config_revision.wrapping_add(1);
        self.menu.rebuild(self.menu_config());
        self.mark_fast_autosave_dirty();
    }

    pub(super) fn commit_instrument_route_structural_draft(&mut self, index: usize) {
        let Some(route) = self
            .menu
            .value_for_key(&format!("instruments.{index}.mixer.route"))
        else {
            return;
        };
        let Some(instrument) = self.instruments.get_mut(index) else {
            return;
        };
        if instrument.route == route {
            return;
        }
        instrument.route = route;
        self.audio_config_revision = self.audio_config_revision.wrapping_add(1);
        self.mark_fast_autosave_dirty();
    }
}
