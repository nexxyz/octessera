use super::NativeRunner;

impl NativeRunner {
    #[cfg(test)]
    pub(super) fn apply_menu_state(&mut self) -> Result<(), String> {
        self.clear_deferred_menu_apply();
        let current_key = self.menu.current_key().map(str::to_string);
        if current_key.as_deref().is_some_and(|key| {
            key.ends_with(".params.timeNote")
                || key.ends_with(".params.timeMode")
                || key.ends_with(".params.timeMs")
                || key.ends_with(".rangeMin")
                || key.ends_with(".rangeMax")
        }) {
            return self.apply_or_schedule_menu_key(current_key.as_deref().unwrap());
        }
        let mut config_changed = false;
        let mut audio_config_changed = false;
        let (_sparks_mode_changed, global_config_changed, global_audio_config_changed) =
            self.apply_global_runtime_menu_state();
        let sparks_fx_changed = self.apply_sparks_fx_menu_state();
        config_changed |= global_config_changed || sparks_fx_changed;
        audio_config_changed |= global_audio_config_changed;
        config_changed |= self.apply_param_mod_invert_menu_state();
        let layer_changed = self.apply_layer_menu_state();
        let instrument_changed = self.apply_instrument_menu_state();
        let pulses_changed = self.apply_pulses_menu_state();
        let fx_changed = self.apply_fx_menu_state();
        config_changed |= layer_changed || instrument_changed || pulses_changed || fx_changed;
        audio_config_changed |=
            instrument_changed && current_key_requires_audio_config(&current_key);
        audio_config_changed |= fx_changed && current_key_requires_audio_config(&current_key);
        self.sync_engine_runtime_config();
        if audio_config_changed || current_key_requires_menu_materialization(&current_key) {
            self.menu.rebuild(self.menu_config());
        }
        if pulses_changed {
            self.refresh_active_interpretation_profile();
            self.engine
                .set_interpretation_profile(self.interpretation_profile.clone());
        }
        let behavior_changed = self.apply_selected_behavior_menu_state()?;
        config_changed |= behavior_changed;
        let behavior_config_changed = self.apply_behavior_config_menu_state()?;
        config_changed |= behavior_config_changed;
        if behavior_changed || behavior_config_changed {
            self.clear_link_arp_state_for_layer(self.active_layer_index);
        }
        self.refresh_active_mapping_config();
        self.refresh_active_interpretation_profile();
        self.engine
            .set_interpretation_profile(self.interpretation_profile.clone());
        let next_auto_save_default = self
            .menu
            .value_for_key("autoSaveDefault")
            .map(|value| value == "true")
            .unwrap_or(self.auto_save_default);
        if self.auto_save_default != next_auto_save_default {
            self.auto_save_default = next_auto_save_default;
            config_changed = true;
        }
        let next_rolling_backups = self
            .menu
            .value_for_key("rollingBackups")
            .map(|value| value == "true")
            .unwrap_or(self.rolling_backups);
        if self.rolling_backups != next_rolling_backups {
            self.rolling_backups = next_rolling_backups;
            config_changed = true;
        }
        if audio_config_changed {
            self.audio_config_revision = self.audio_config_revision.saturating_add(1);
        }
        if config_changed {
            self.mark_config_dirty();
            self.force_autosave_payload_due();
        }
        Ok(())
    }

    #[cfg(test)]
    fn apply_selected_behavior_menu_state(&mut self) -> Result<bool, String> {
        let Some(behavior_id) = self.menu.selected_behavior().map(|value| value.to_string()) else {
            return Ok(false);
        };
        self.apply_behavior_selection(&behavior_id)
    }

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

    #[cfg(test)]
    pub(super) fn apply_behavior_config_menu_state(&mut self) -> Result<bool, String> {
        let next_behavior_config = self.behavior_config_from_menu()?;
        if next_behavior_config == self.behavior_config {
            return Ok(false);
        }
        let behavior_id = self.behavior.id().to_string();
        self.replace_layer_engine_with_config(
            self.active_layer_index,
            self.behavior,
            next_behavior_config.clone(),
            None,
        )?;
        self.set_layer_behavior_config(self.active_layer_index, &behavior_id, next_behavior_config);
        Ok(true)
    }

    #[cfg(test)]
    fn apply_param_mod_invert_menu_state(&mut self) -> bool {
        let mut changed = false;
        for layer_index in 0..self.param_mods.len() {
            for axis in ["x", "y"] {
                for slot in 0..2 {
                    let key = format!("layers.{layer_index}.paramMods.{axis}.{slot}.invert");
                    let Some(value) = self.menu.value_for_key(&key) else {
                        continue;
                    };
                    let invert = value == "true";
                    let target = if axis == "x" {
                        self.param_mods[layer_index].x.get_mut(slot)
                    } else {
                        self.param_mods[layer_index].y.get_mut(slot)
                    };
                    if let Some(Some(binding)) = target {
                        if binding.invert != invert {
                            binding.invert = invert;
                            changed = true;
                        }
                    }
                }
            }
        }
        changed
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

#[cfg(test)]
fn current_key_requires_audio_config(current_key: &Option<String>) -> bool {
    let Some(key) = current_key.as_deref() else {
        return true;
    };
    if key == "masterVolume" {
        return false;
    }
    if key == "sound.voiceStealingMode" {
        return true;
    }
    if let Some(rest) = key.strip_prefix("instruments.") {
        let Some((_, suffix)) = rest.split_once('.') else {
            return true;
        };
        return !matches!(
            suffix,
            "name"
                | "autoName"
                | "midi.enabled"
                | "midi.channel"
                | "midi.velocity"
                | "midi.durationMs"
                | "mixer.volume"
                | "mixer.panPos"
                | "synth.amp.gainPct"
                | "synth.filter.cutoffHz"
                | "synth.filter.resonance"
                | "sample.tuneSemis"
                | "sample.amp.gainPct"
                | "sample.amp.velocitySensitivityPct"
        );
    }
    if let Some(rest) = key.strip_prefix("mixer.buses.") {
        let Some((_, suffix)) = rest.split_once('.') else {
            return true;
        };
        if matches!(suffix, "name" | "autoName" | "panPos" | "volume") {
            return false;
        }
        return suffix.ends_with(".type");
    }
    if let Some(rest) = key.strip_prefix("mixer.master.slots.") {
        let Some((_, suffix)) = rest.split_once('.') else {
            return true;
        };
        return suffix == "type";
    }
    false
}

#[cfg(test)]
fn current_key_requires_menu_materialization(current_key: &Option<String>) -> bool {
    let Some(key) = current_key.as_deref() else {
        return false;
    };
    if let Some(rest) = key.strip_prefix("instruments.") {
        let Some((_, suffix)) = rest.split_once('.') else {
            return true;
        };
        return suffix == "kind" || suffix.ends_with(".type");
    }
    if let Some(rest) = key.strip_prefix("mixer.buses.") {
        let Some((_, suffix)) = rest.split_once('.') else {
            return true;
        };
        return suffix.ends_with(".type");
    }
    if let Some(rest) = key.strip_prefix("mixer.master.slots.") {
        let Some((_, suffix)) = rest.split_once('.') else {
            return true;
        };
        return suffix == "type";
    }
    if let Some(rest) = key.strip_prefix("layers.") {
        let Some((_, suffix)) = rest.split_once('.') else {
            return true;
        };
        return suffix == "pulses.scanMode";
    }
    false
}
