use super::{velocity_curve_from_id, NativeRunner, Value};

impl NativeRunner {
    pub(super) fn apply_menu_state(&mut self) -> Result<(), String> {
        self.clear_deferred_menu_apply();
        let current_key = self.menu.current_key().map(str::to_string);
        let mut config_changed = false;
        let mut audio_config_changed = false;
        let (dance_mode_changed, global_config_changed, global_audio_config_changed) =
            self.apply_global_runtime_menu_state();
        let dance_fx_changed = self.apply_dance_fx_menu_state();
        config_changed |= global_config_changed || dance_fx_changed;
        audio_config_changed |= global_audio_config_changed;
        config_changed |= self.apply_param_mod_invert_menu_state();
        let part_changed = self.apply_part_menu_state();
        let instrument_changed = self.apply_instrument_menu_state();
        let sense_changed = self.apply_sense_menu_state();
        let fx_changed = self.apply_fx_menu_state();
        config_changed |= part_changed || instrument_changed || sense_changed || fx_changed;
        audio_config_changed |=
            instrument_changed && current_key_requires_audio_config(&current_key);
        audio_config_changed |= fx_changed && current_key_requires_audio_config(&current_key);
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
        config_changed |= behavior_changed;
        if behavior_changed {
            self.menu.rebuild(self.menu_config());
        }
        config_changed |= self.apply_behavior_config_menu_state()?;
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
        if audio_config_changed {
            self.audio_config_revision = self.audio_config_revision.wrapping_add(1);
        }
        if config_changed {
            self.config_dirty = true;
            self.force_autosave_payload_due();
        }
        Ok(())
    }

    fn apply_global_runtime_menu_state(&mut self) -> (bool, bool, bool) {
        let mut dance_mode_changed = false;
        let mut config_changed = false;
        let mut audio_config_changed = false;
        if let Some(sync_source) = self.menu.selected_sync_source() {
            config_changed |= self.sync_source != sync_source;
            self.sync_source = sync_source;
        }
        if let Some(step_pulses) = self.menu.selected_algorithm_step_pulses() {
            config_changed |= self.algorithm_step_pulses != step_pulses;
            self.algorithm_step_pulses = step_pulses;
            if let Some(part_step) = self
                .part_algorithm_step_pulses
                .get_mut(self.active_part_index)
            {
                config_changed |= *part_step != step_pulses;
                *part_step = step_pulses;
            }
        }
        if let Some(master_volume) = self.menu.selected_master_volume() {
            config_changed |= self.ui.master_volume != master_volume;
            self.ui.master_volume = master_volume;
        }
        if let Some(draft_name) = self.menu.value_for_key("system.draftName") {
            config_changed |= self.preset_draft_name != draft_name;
            self.preset_draft_name = draft_name;
        }
        config_changed |= self.apply_midi_menu_flags();
        if let Some(dance_mode) = self.menu.selected_dance_mode() {
            let changed = self.dance_mode != dance_mode;
            self.dance_mode = dance_mode.clone();
            dance_mode_changed = changed;
            config_changed |= changed;
            if changed && self.menu.is_in_dance_root_group() {
                self.active_dance_mode = dance_mode;
            }
        }
        config_changed |= self.apply_xy_menu_state();
        let (ui_sound_changed, sound_audio_config_changed) =
            self.apply_ui_sound_transport_menu_state();
        config_changed |= ui_sound_changed;
        audio_config_changed |= sound_audio_config_changed;
        (dance_mode_changed, config_changed, audio_config_changed)
    }

    fn apply_midi_menu_flags(&mut self) -> bool {
        let mut changed = false;
        if let Some(midi_enabled) = self
            .menu
            .value_for_key("midiEnabled")
            .map(|value| value == "true")
        {
            changed |= self.midi_enabled != midi_enabled;
            self.midi_enabled = midi_enabled;
        }
        if let Some(clock_out_enabled) = self
            .menu
            .value_for_key("midi.clockOutEnabled")
            .map(|value| value == "true")
        {
            changed |= self.midi_clock_out_enabled != clock_out_enabled;
            self.midi_clock_out_enabled = clock_out_enabled;
        }
        if let Some(clock_in_enabled) = self
            .menu
            .value_for_key("midi.clockInEnabled")
            .map(|value| value == "true")
        {
            changed |= self.midi_clock_in_enabled != clock_in_enabled;
            self.midi_clock_in_enabled = clock_in_enabled;
        }
        if let Some(respond_to_start_stop) = self
            .menu
            .value_for_key("midi.respondToStartStop")
            .map(|value| value == "true")
        {
            changed |= self.midi_respond_to_start_stop != respond_to_start_stop;
            self.midi_respond_to_start_stop = respond_to_start_stop;
        }
        changed
    }

    fn apply_xy_menu_state(&mut self) -> bool {
        let mut changed = false;
        if let Some(xy_release) = self.menu.value_for_key("dance.xy.release") {
            changed |= self.xy_release != xy_release;
            self.xy_release = xy_release;
        }
        if let Some(invert_x) = self.menu.value_for_key("dance.xy.invertX") {
            let invert_x = invert_x == "true";
            changed |= self.xy_invert_x != invert_x;
            self.xy_invert_x = invert_x;
        }
        if let Some(invert_y) = self.menu.value_for_key("dance.xy.invertY") {
            let invert_y = invert_y == "true";
            changed |= self.xy_invert_y != invert_y;
            self.xy_invert_y = invert_y;
        }
        changed
    }

    fn apply_ui_sound_transport_menu_state(&mut self) -> (bool, bool) {
        let mut changed = self.apply_display_menu_state();
        changed |= self.apply_transport_menu_state();
        let (sound_changed, sound_audio_config_changed) = self.apply_sound_menu_state();
        changed |= sound_changed;
        changed |= self.apply_ui_behavior_menu_state();
        (changed, sound_audio_config_changed)
    }

    fn apply_display_menu_state(&mut self) -> bool {
        let mut changed = false;
        if let Some(display_brightness) = self.menu.selected_display_brightness() {
            changed |= self.ui.display_brightness != display_brightness;
            self.ui.display_brightness = display_brightness;
        }
        if let Some(button_brightness) = self.menu.selected_button_brightness() {
            changed |= self.ui.button_brightness != button_brightness;
            self.ui.button_brightness = button_brightness;
        }
        if let Some(grid_brightness) = self.menu.number_for_key("gridBrightness") {
            let grid_brightness = grid_brightness.clamp(10, 100) as u8;
            changed |= self.ui.grid_brightness != grid_brightness;
            self.ui.grid_brightness = grid_brightness;
        }
        if let Some(numeric_display_mode) = self.menu.value_for_key("numericDisplayMode") {
            changed |= self.ui.numeric_display_mode != numeric_display_mode;
            self.ui.numeric_display_mode = numeric_display_mode;
        }
        if let Some(screen_sleep_seconds) = self.menu.number_for_key("screenSleepSeconds") {
            let screen_sleep_seconds = screen_sleep_seconds.clamp(0, 600) as u16;
            changed |= self.ui.screen_sleep_seconds != screen_sleep_seconds;
            self.ui.screen_sleep_seconds = screen_sleep_seconds;
        }
        changed
    }

    fn apply_transport_menu_state(&mut self) -> bool {
        let mut changed = false;
        if let Some(bpm) = self.menu.number_for_key("transport.bpm") {
            let bpm = f64::from(bpm.clamp(40, 240));
            changed |= (self.bpm - bpm).abs() > f64::EPSILON;
            self.bpm = bpm;
        }
        changed
    }

    fn apply_sound_menu_state(&mut self) -> (bool, bool) {
        let mut changed = false;
        let mut audio_config_changed = false;
        if let Some(note_length_ms) = self.menu.number_for_key("sound.noteLengthMs") {
            let note_length_ms = note_length_ms.clamp(30, 2000) as u32;
            changed |= self.global_sound.note_length_ms != note_length_ms;
            self.global_sound.note_length_ms = note_length_ms;
        }
        if let Some(velocity_scale_pct) = self.menu.number_for_key("sound.velocityScalePct") {
            let velocity_scale_pct = velocity_scale_pct.clamp(0, 200) as u16;
            changed |= self.global_sound.velocity_scale_pct != velocity_scale_pct;
            self.global_sound.velocity_scale_pct = velocity_scale_pct;
        }
        if let Some(velocity_curve) = self.menu.value_for_key("sound.velocityCurve") {
            let velocity_curve = velocity_curve_from_id(&velocity_curve);
            changed |= self.global_sound.velocity_curve != velocity_curve;
            self.global_sound.velocity_curve = velocity_curve;
        }
        if let Some(voice_stealing_mode) = self.menu.value_for_key("sound.voiceStealingMode") {
            if let Some(mode) = super::normalize_voice_stealing_mode(&voice_stealing_mode) {
                let mode = mode.into();
                let voice_changed = self.voice_stealing_mode != mode;
                changed |= voice_changed;
                audio_config_changed |= voice_changed;
                self.voice_stealing_mode = mode;
            }
        }
        (changed, audio_config_changed)
    }

    fn apply_ui_behavior_menu_state(&mut self) -> bool {
        let mut changed = false;
        if let Some(ghost_cells) = self.menu.value_for_key("ghostCells") {
            let ghost_cells = ghost_cells == "true";
            changed |= self.ui.ghost_cells != ghost_cells;
            self.ui.ghost_cells = ghost_cells;
        }
        if let Some(input_events_while_paused) = self.menu.value_for_key("inputEventsWhilePaused") {
            let input_events_while_paused = input_events_while_paused == "true";
            changed |= self.input_events_while_paused != input_events_while_paused;
            self.input_events_while_paused = input_events_while_paused;
        }
        if let Some(aux_auto_map_enabled) = self.menu.value_for_key("auxAutoMapEnabled") {
            let aux_auto_map_enabled = aux_auto_map_enabled == "true";
            changed |= self.aux_auto_map_enabled != aux_auto_map_enabled;
            self.aux_auto_map_enabled = aux_auto_map_enabled;
        }
        changed
    }

    fn apply_selected_behavior_menu_state(&mut self) -> Result<bool, String> {
        let Some(behavior_id) = self.menu.selected_behavior().map(|value| value.to_string()) else {
            return Ok(false);
        };
        let current_part_behavior_id = self
            .part_behavior_ids
            .get(self.active_part_index)
            .cloned()
            .unwrap_or_else(|| self.behavior.id().into());
        let behavior_changed = behavior_id.as_str() != self.behavior.id();
        let part_behavior_changed = behavior_id != current_part_behavior_id;
        if !behavior_changed && !part_behavior_changed {
            return Ok(self.sync_active_part_auto_name(&behavior_id));
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
            .unwrap_or(Value::Null);
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
            self.rebuild_engine(behavior)?;
        }
        Ok(true)
    }

    fn sync_active_part_auto_name(&mut self, behavior_id: &str) -> bool {
        if !self
            .part_auto_names
            .get(self.active_part_index)
            .copied()
            .unwrap_or(true)
        {
            return false;
        }
        let Some(name) = self.part_names.get_mut(self.active_part_index) else {
            return false;
        };
        if name == behavior_id {
            return false;
        }
        *name = behavior_id.into();
        true
    }

    fn apply_behavior_config_menu_state(&mut self) -> Result<bool, String> {
        let next_behavior_config = self.behavior_config_from_menu()?;
        if next_behavior_config == self.behavior_config {
            return Ok(false);
        }
        self.behavior_config = next_behavior_config;
        if let Some(config) = self.part_behavior_configs.get_mut(self.active_part_index) {
            *config = self.behavior_config.clone();
        }
        self.behavior_configs
            .insert(self.behavior.id().to_string(), self.behavior_config.clone());
        self.rebuild_engine(self.behavior)?;
        Ok(true)
    }

    fn apply_param_mod_invert_menu_state(&mut self) -> bool {
        let mut changed = false;
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
                            changed = true;
                        }
                    }
                }
            }
        }
        changed
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
        if matches!(suffix, "name" | "autoName" | "panPos") {
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
