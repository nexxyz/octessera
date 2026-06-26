use super::{velocity_curve_from_id, NativeRunner, Value};

impl NativeRunner {
    pub(super) fn apply_menu_state(&mut self) -> Result<(), String> {
        self.clear_deferred_menu_apply();
        let before_payload = self.config_payload();
        let dance_mode_changed = self.apply_global_runtime_menu_state();
        let dance_fx_changed = self.apply_dance_fx_menu_state();
        self.apply_param_mod_invert_menu_state();
        let part_changed = self.apply_part_menu_state();
        let instrument_changed = self.apply_instrument_menu_state();
        let sense_changed = self.apply_sense_menu_state();
        let fx_changed = self.apply_fx_menu_state();
        self.sync_engine_runtime_config();
        if part_changed
            || instrument_changed
            || sense_changed
            || fx_changed
            || dance_fx_changed
            || dance_mode_changed
        {
            self.menu.rebuild(self.menu_config());
        }
        if sense_changed {
            self.refresh_active_interpretation_profile();
            self.engine
                .set_interpretation_profile(self.interpretation_profile.clone());
        }
        let behavior_changed = self.apply_selected_behavior_menu_state()?;
        if behavior_changed {
            self.menu.rebuild(self.menu_config());
        }
        self.apply_behavior_config_menu_state()?;
        self.refresh_active_mapping_config();
        self.refresh_active_interpretation_profile();
        self.engine
            .set_interpretation_profile(self.interpretation_profile.clone());
        self.auto_save_default = self
            .menu
            .value_for_key("autoSaveDefault")
            .map(|value| value == "true")
            .unwrap_or(self.auto_save_default);
        let after_payload = self.config_payload();
        if audio_config_changed(&before_payload, &after_payload) {
            self.audio_config_revision = self.audio_config_revision.wrapping_add(1);
        }
        if after_payload != before_payload {
            self.config_dirty = true;
            self.force_autosave_payload_due();
        }
        Ok(())
    }

    fn apply_global_runtime_menu_state(&mut self) -> bool {
        let mut dance_mode_changed = false;
        if let Some(sync_source) = self.menu.selected_sync_source() {
            self.sync_source = sync_source;
        }
        if let Some(step_pulses) = self.menu.selected_algorithm_step_pulses() {
            self.algorithm_step_pulses = step_pulses;
            if let Some(part_step) = self
                .part_algorithm_step_pulses
                .get_mut(self.active_part_index)
            {
                *part_step = step_pulses;
            }
        }
        if let Some(master_volume) = self.menu.selected_master_volume() {
            self.ui.master_volume = master_volume;
        }
        if let Some(draft_name) = self.menu.value_for_key("system.draftName") {
            self.preset_draft_name = draft_name;
        }
        self.apply_midi_menu_flags();
        if let Some(dance_mode) = self.menu.selected_dance_mode() {
            let changed = self.dance_mode != dance_mode;
            self.dance_mode = dance_mode.clone();
            dance_mode_changed = changed;
            if changed && self.menu.state.stack.first() == Some(&3) {
                self.active_dance_mode = dance_mode;
            }
        }
        self.apply_xy_menu_state();
        self.apply_ui_sound_transport_menu_state();
        dance_mode_changed
    }

    fn apply_midi_menu_flags(&mut self) {
        if let Some(midi_enabled) = self
            .menu
            .value_for_key("midiEnabled")
            .map(|value| value == "true")
        {
            self.midi_enabled = midi_enabled;
        }
        if let Some(clock_out_enabled) = self
            .menu
            .value_for_key("midi.clockOutEnabled")
            .map(|value| value == "true")
        {
            self.midi_clock_out_enabled = clock_out_enabled;
        }
        if let Some(clock_in_enabled) = self
            .menu
            .value_for_key("midi.clockInEnabled")
            .map(|value| value == "true")
        {
            self.midi_clock_in_enabled = clock_in_enabled;
        }
        if let Some(respond_to_start_stop) = self
            .menu
            .value_for_key("midi.respondToStartStop")
            .map(|value| value == "true")
        {
            self.midi_respond_to_start_stop = respond_to_start_stop;
        }
    }

    fn apply_xy_menu_state(&mut self) {
        if let Some(xy_release) = self.menu.value_for_key("dance.xy.release") {
            self.xy_release = xy_release;
        }
        if let Some(invert_x) = self.menu.value_for_key("dance.xy.invertX") {
            self.xy_invert_x = invert_x == "true";
        }
        if let Some(invert_y) = self.menu.value_for_key("dance.xy.invertY") {
            self.xy_invert_y = invert_y == "true";
        }
    }

    fn apply_ui_sound_transport_menu_state(&mut self) {
        self.apply_display_menu_state();
        self.apply_transport_menu_state();
        self.apply_sound_menu_state();
        self.apply_ui_behavior_menu_state();
    }

    fn apply_display_menu_state(&mut self) {
        if let Some(display_brightness) = self.menu.selected_display_brightness() {
            self.ui.display_brightness = display_brightness;
        }
        if let Some(button_brightness) = self.menu.selected_button_brightness() {
            self.ui.button_brightness = button_brightness;
        }
        if let Some(grid_brightness) = self.menu.number_for_key("gridBrightness") {
            self.ui.grid_brightness = grid_brightness.clamp(10, 100) as u8;
        }
        if let Some(numeric_display_mode) = self.menu.value_for_key("numericDisplayMode") {
            self.ui.numeric_display_mode = numeric_display_mode;
        }
        if let Some(screen_sleep_seconds) = self.menu.number_for_key("screenSleepSeconds") {
            self.ui.screen_sleep_seconds = screen_sleep_seconds.clamp(0, 600) as u16;
        }
    }

    fn apply_transport_menu_state(&mut self) {
        if let Some(bpm) = self.menu.number_for_key("transport.bpm") {
            self.bpm = f64::from(bpm.clamp(40, 240));
        }
    }

    fn apply_sound_menu_state(&mut self) {
        if let Some(note_length_ms) = self.menu.number_for_key("sound.noteLengthMs") {
            self.global_sound.note_length_ms = note_length_ms.clamp(30, 2000) as u32;
        }
        if let Some(velocity_scale_pct) = self.menu.number_for_key("sound.velocityScalePct") {
            self.global_sound.velocity_scale_pct = velocity_scale_pct.clamp(0, 200) as u16;
        }
        if let Some(velocity_curve) = self.menu.value_for_key("sound.velocityCurve") {
            self.global_sound.velocity_curve = velocity_curve_from_id(&velocity_curve);
        }
        if let Some(voice_stealing_mode) = self.menu.value_for_key("sound.voiceStealingMode") {
            if let Some(mode) = super::normalize_voice_stealing_mode(&voice_stealing_mode) {
                self.voice_stealing_mode = mode.into();
            }
        }
    }

    fn apply_ui_behavior_menu_state(&mut self) {
        if let Some(ghost_cells) = self.menu.value_for_key("ghostCells") {
            self.ui.ghost_cells = ghost_cells == "true";
        }
        if let Some(input_events_while_paused) = self.menu.value_for_key("inputEventsWhilePaused") {
            self.input_events_while_paused = input_events_while_paused == "true";
        }
        if let Some(aux_auto_map_enabled) = self.menu.value_for_key("auxAutoMapEnabled") {
            self.aux_auto_map_enabled = aux_auto_map_enabled == "true";
        }
    }

    fn apply_selected_behavior_menu_state(&mut self) -> Result<bool, String> {
        let Some(behavior_id) = self.menu.selected_behavior().map(|value| value.to_string()) else {
            return Ok(false);
        };
        if behavior_id.as_str() == self.behavior.id() {
            return Ok(false);
        }
        let previous_behavior_id = self.behavior.id().to_string();
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
            .unwrap_or(Value::Null);
        self.behavior_configs
            .insert(behavior_id.clone(), self.behavior_config.clone());
        if let Some(config) = self.part_behavior_configs.get_mut(self.active_part_index) {
            *config = self.behavior_config.clone();
        }
        if let Some(part_behavior_id) = self.part_behavior_ids.get_mut(self.active_part_index) {
            *part_behavior_id = behavior_id.clone();
        }
        if self
            .part_auto_names
            .get(self.active_part_index)
            .copied()
            .unwrap_or(true)
        {
            if let Some(name) = self.part_names.get_mut(self.active_part_index) {
                *name = behavior_id.clone();
            }
        }
        self.remap_bindings_for_behavior_change(
            &previous_behavior_id,
            &behavior_id,
            self.active_part_index,
        );
        self.rebuild_engine(behavior)?;
        Ok(true)
    }

    fn apply_behavior_config_menu_state(&mut self) -> Result<(), String> {
        let next_behavior_config = self.behavior_config_from_menu()?;
        if next_behavior_config == self.behavior_config {
            return Ok(());
        }
        self.behavior_config = next_behavior_config;
        if let Some(config) = self.part_behavior_configs.get_mut(self.active_part_index) {
            *config = self.behavior_config.clone();
        }
        self.behavior_configs
            .insert(self.behavior.id().to_string(), self.behavior_config.clone());
        self.rebuild_engine(self.behavior)
    }

    fn apply_param_mod_invert_menu_state(&mut self) {
        for part_index in 0..self.param_mods.len() {
            for axis in ["x", "y"] {
                for slot in 0..2 {
                    let key = format!("parts.{part_index}.paramMods.{axis}.{slot}.invert");
                    let Some(value) = self.menu.value_for_key(&key) else {
                        continue;
                    };
                    let invert = value == "true";
                    let target = if axis == "x" {
                        self.param_mods[part_index].x.get_mut(slot)
                    } else {
                        self.param_mods[part_index].y.get_mut(slot)
                    };
                    if let Some(Some(binding)) = target {
                        if binding.invert != invert {
                            binding.invert = invert;
                            self.config_dirty = true;
                        }
                    }
                }
            }
        }
    }

    pub(super) fn refresh_active_mapping_config(&mut self) {
        let mapping = self.mapping_config_for_part(self.active_part_index);
        self.engine.set_mapping_config(mapping.clone());
        self.mapping_config = mapping;
    }

    pub(super) fn refresh_active_interpretation_profile(&mut self) {
        self.interpretation_profile = self.interpretation_profile_for_part(self.active_part_index);
    }

    fn apply_part_menu_state(&mut self) -> bool {
        let mut changed = false;
        for index in 0..self.part_auto_names.len() {
            let before_name = self.part_names.get(index).cloned().unwrap_or_default();
            let Some(auto_name) = self
                .menu
                .value_for_key(&format!("parts.{index}.autoName"))
                .map(|value| value == "true")
            else {
                continue;
            };
            if self.part_auto_names[index] != auto_name {
                self.part_auto_names[index] = auto_name;
                if auto_name {
                    let behavior_id = self
                        .part_behavior_ids
                        .get(index)
                        .cloned()
                        .unwrap_or_else(|| self.behavior.id().into());
                    if let Some(name) = self.part_names.get_mut(index) {
                        *name = behavior_id;
                    }
                }
                changed = true;
            }
            let name_key = format!("parts.{index}.name");
            if self.menu.current_key() == Some(name_key.as_str()) {
                if let Some(name) = self.menu.value_for_key(&name_key) {
                    if name != before_name {
                        if let Some(target) = self.part_names.get_mut(index) {
                            *target = name;
                        }
                        if let Some(auto_name) = self.part_auto_names.get_mut(index) {
                            *auto_name = false;
                        }
                        changed = true;
                    }
                }
            }
            if self.part_auto_names[index] {
                let behavior_id = self
                    .part_behavior_ids
                    .get(index)
                    .cloned()
                    .unwrap_or_else(|| self.behavior.id().into());
                if self.part_names.get(index) != Some(&behavior_id) {
                    if let Some(target) = self.part_names.get_mut(index) {
                        *target = behavior_id;
                    }
                    changed = true;
                }
            }
        }
        changed
    }
}

fn audio_config_changed(before: &Value, after: &Value) -> bool {
    let before = before.get("runtimeConfig").unwrap_or(before);
    let after = after.get("runtimeConfig").unwrap_or(after);
    ["instruments", "mixer", "masterVolume"]
        .into_iter()
        .any(|key| before.get(key) != after.get(key))
}
